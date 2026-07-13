//! Reusable per-render scratch buffers (P2-SCRATCH).
//!
//! The hot compose helpers — `compose::field::draw_bare_with_shadow`,
//! `raster::backdrop_blur`, and the Glass seat coverage — allocated fresh Vecs / a
//! `Raster` on EVERY render (warm AND cold, so the interactive slider-drag path paid a
//! heap churn per frame, not just first paint). `RenderScratch` owns those buffers and
//! reuses them. It is threaded down the render path EXACTLY like `&mut MaskCache`: one
//! instance per `RenderSession`, one per rayon worker in `batch::render_icons_par`
//! (`map_init`), and a transient for the free `render_tile*`. It is NEVER shared
//! mutably across threads.
//!
//! ## BYTE-PARITY invariant (the reuse trap)
//! A freshly `vec![0.0; n]` / `Raster::new` buffer is ZERO-initialized; a reused buffer
//! holds STALE data. A reused buffer is byte-identical to a fresh one ONLY IF every
//! element ever READ this render is first WRITTEN this render (full overwrite), OR the
//! buffer is explicitly zeroed to the fresh-alloc state. Each buffer below documents
//! which case it is and how the reset restores it. The four-way M6 cert (corpus + size
//! sweep, scalar==fast × native/wasm, 0 diff bytes) is the proof this holds.

use crate::js_math::clamp_byte;
use crate::raster::{box_blur_into, Raster};

/// Scratch for `compose::field::draw_bare_with_shadow`.
pub(crate) struct ShadowScratch {
    /// The centred-artwork layer. `draw_centred` writes only the centred content box,
    /// so the BORDER pixels are never written and today rely on the fresh-alloc zero
    /// (`draw_bare_with_shadow` then reads `layer.data[i*4+3]` for the WHOLE layer, and
    /// `composite_over` reads it all again). → ZEROED on every `prepare`, byte-identical
    /// to `Raster::new(size, size)`.
    pub(crate) layer: Raster,
    /// Coverage field. FULLY overwritten by the `alpha.iter_mut()` loop before any read,
    /// so only its length matters on reuse (contents are irrelevant).
    pub(crate) alpha: Vec<f32>,
    /// Separable-blur transpose scratch. FULLY overwritten by every `box_blur_in_place`
    /// horizontal pass before its vertical read, so only its length matters on reuse.
    pub(crate) tmp: Vec<f32>,
}

impl ShadowScratch {
    fn new() -> Self {
        Self { layer: Raster::new(0, 0), alpha: Vec::new(), tmp: Vec::new() }
    }

    /// Size the buffers to `size²` and restore the exact fresh-alloc state the parity
    /// contract needs: `layer` zeroed (== `Raster::new`), `alpha`/`tmp` length `n`
    /// (their contents are fully overwritten before any read).
    pub(crate) fn prepare(&mut self, size: usize) {
        let n = size * size;
        self.layer.width = size;
        self.layer.height = size;
        // clear + resize(0) leaves every one of the n*4 bytes freshly zeroed, matching
        // Raster::new(size, size) exactly — the border pixels draw_centred never writes.
        self.layer.data.clear();
        self.layer.data.resize(n * 4, 0);
        self.alpha.resize(n, 0.0);
        self.tmp.resize(n, 0.0);
    }
}

/// Scratch for `raster::backdrop_blur` (the frosted Glass-seat backdrop).
pub(crate) struct BlurScratch {
    /// Source colour planes — each FULLY overwritten from `src.data` before its blur.
    chans: [Vec<f64>; 4],
    /// Separable-blur transpose scratch, shared across the four channel blurs. FULLY
    /// overwritten by each `box_blur_into` horizontal pass before its vertical read.
    tmp: Vec<f64>,
    /// Blurred colour planes — each FULLY overwritten by `box_blur_into`'s vertical pass.
    blurred: [Vec<f64>; 4],
    /// Output raster — every one of its `size²·4` bytes is written from `blurred`, so a
    /// reused buffer needs no zeroing.
    out: Raster,
}

impl BlurScratch {
    pub(crate) fn new() -> Self {
        Self {
            chans: Default::default(),
            tmp: Vec::new(),
            blurred: Default::default(),
            out: Raster::new(0, 0),
        }
    }

    /// Byte-identical to the free `raster::backdrop_blur`, reusing owned buffers instead
    /// of allocating four channel arrays + four blurred arrays + an output raster per
    /// call. Returns a borrow of the internal output raster (valid until the next call).
    pub(crate) fn backdrop_blur(&mut self, src: &Raster, radius: i32) -> &Raster {
        if radius < 1 {
            // matches `src.clone()`.
            self.out.width = src.width;
            self.out.height = src.height;
            self.out.data.clear();
            self.out.data.extend_from_slice(&src.data);
            return &self.out;
        }
        let size = src.width;
        let n = size * size;
        for c in 0..4 {
            let ch = &mut self.chans[c];
            ch.resize(n, 0.0);
            for (i, v) in ch.iter_mut().enumerate() {
                *v = src.data[i * 4 + c] as f64;
            }
        }
        self.tmp.resize(n, 0.0);
        for c in 0..4 {
            self.blurred[c].resize(n, 0.0);
            box_blur_into(&mut self.blurred[c], &self.chans[c], &mut self.tmp, size, radius);
        }
        self.out.width = size;
        self.out.height = size;
        self.out.data.resize(n * 4, 0);
        for i in 0..n {
            let i4 = i * 4;
            self.out.data[i4] = clamp_byte(self.blurred[0][i]);
            self.out.data[i4 + 1] = clamp_byte(self.blurred[1][i]);
            self.out.data[i4 + 2] = clamp_byte(self.blurred[2][i]);
            self.out.data[i4 + 3] = clamp_byte(self.blurred[3][i]);
        }
        &self.out
    }
}

/// The reusable buffer set threaded through one render (alongside `&mut MaskCache`).
pub struct RenderScratch {
    pub(crate) shadow: ShadowScratch,
    pub(crate) blur: BlurScratch,
    /// Glass seat coverage. Written ONLY inside the seat's bounding box (and read only
    /// there + by the arrow glyph, a strict sub-box), so its un-written margin relies on
    /// the fresh `vec![0.0; n]`. → ZEROED to length `n` on each Glass render via
    /// [`RenderScratch::reset_seat_cov`], byte-identical to the fresh alloc.
    pub(crate) seat_cov: Vec<f64>,
}

impl RenderScratch {
    pub fn new() -> Self {
        Self { shadow: ShadowScratch::new(), blur: BlurScratch::new(), seat_cov: Vec::new() }
    }

    /// Zero-fill `seat_cov` to `n` — byte-identical to a fresh `vec![0.0; n]`.
    pub(crate) fn reset_seat_cov(&mut self, n: usize) {
        self.seat_cov.clear();
        self.seat_cov.resize(n, 0.0);
    }
}

impl Default for RenderScratch {
    fn default() -> Self {
        Self::new()
    }
}

// The reuse-vs-fresh byte-identity cert (P2-SCRATCH mandatory verification #3). Pins the
// "stale data doesn't leak" property: a tile rendered through a scratch already POLLUTED
// by prior different renders must be byte-identical to one rendered through a FRESH
// scratch AND to the free `render_tile` oracle. A wrongly-skipped `layer` zero or
// `seat_cov` reset diverges here.
#[cfg(test)]
mod reuse_cert {
    use super::RenderScratch;
    use crate::compose::{render_tile, render_tile_cached, ComposeDiagnostics, RenderOpts};
    use crate::config::{Band, Config, Distinction, FilterStyle, MarkStyle, MonoStyle, PlateFallback, Subject};
    use crate::mask_cache::MaskCache;
    use crate::raster::Raster;
    use crate::shapes::IconShape;

    /// A centred opaque blob on a transparent field → the transparent-edge bare-shadow
    /// lane (`draw_bare_with_shadow`) under Original + a derived plate. `margin` controls
    /// the content coverage so a polluter can write a strictly larger region than the
    /// target overwrites — exactly the case a missing `layer` zero would leak.
    fn blob(n: usize, margin: usize, r: u8, g: u8, b: u8) -> Raster {
        let mut raster = Raster::new(n, n);
        let (lo, hi) = (margin, n - margin);
        for y in lo..hi {
            for x in lo..hi {
                let i = (y * n + x) * 4;
                raster.data[i..i + 4].copy_from_slice(&[r, g, b, 255]);
            }
        }
        raster
    }

    fn base(shape: IconShape) -> Config {
        Config {
            shape,
            subject: Subject::Original,
            tint: 0xff6f5e,
            mono_style: MonoStyle::Tonal,
            plate_band: Band::Vivid,
            shortcut_shape: None,
            distinction: Distinction::None,
            mark_style: MarkStyle::Glass,
            mark_color: None,
            filter: FilterStyle::None,
            plate_color: None,
            plate_fallback: PlateFallback::Derived,
        }
    }

    fn render_with(scratch: &mut RenderScratch, src: &Raster, cfg: &Config, is_shortcut: bool, size: usize) -> Vec<u8> {
        let mut mask = MaskCache::new();
        let mut diag = ComposeDiagnostics::default();
        render_tile_cached(src, cfg, is_shortcut, false, size, &RenderOpts::default(), &mut diag, None, &mut mask, scratch, None).data
    }

    fn free(src: &Raster, cfg: &Config, is_shortcut: bool, size: usize) -> Vec<u8> {
        let mut diag = ComposeDiagnostics::default();
        render_tile(src, cfg, is_shortcut, false, size, &RenderOpts::default(), &mut diag).data
    }

    #[test]
    fn scratch_reuse_matches_fresh_and_free() {
        // Shadow-lane target (draw_bare_with_shadow) and a Glass-mark target
        // (draw_bare_with_shadow tile + backdrop_blur + seat_cov).
        let shadow_src = blob(96, 24, 210, 70, 40);
        let shadow_cfg = base(IconShape::Circle);
        let glass_src = blob(96, 24, 40, 90, 210);
        let glass_cfg = Config { distinction: Distinction::Mark, mark_style: MarkStyle::Glass, ..base(IconShape::Apple) };

        // A single scratch, POLLUTED across both lanes at different sizes/shapes/marks —
        // larger content coverage (margin 8) than the 96px targets (margin 24), so the
        // layer border + seat region carry stale non-zero data going in.
        let mut dirty = RenderScratch::new();
        let fold200 = Config { distinction: Distinction::Mark, mark_style: MarkStyle::Fold, ..base(IconShape::Diamond) };
        let glass96 = Config { distinction: Distinction::Mark, mark_style: MarkStyle::Glass, ..base(IconShape::Circle) };
        let _ = render_with(&mut dirty, &blob(160, 12, 250, 250, 30), &fold200, true, 200);
        let _ = render_with(&mut dirty, &blob(96, 8, 10, 10, 10), &glass96, true, 96);

        // Each target through the (ever more) polluted scratch must equal a fresh scratch
        // AND the free render_tile — at the same size, a bigger size, and 255 (sweep max).
        for (src, cfg, shortcut, size) in [
            (&shadow_src, &shadow_cfg, false, 96usize),
            (&glass_src, &glass_cfg, true, 96),
            (&shadow_src, &shadow_cfg, false, 128),
            (&glass_src, &glass_cfg, true, 255),
        ] {
            let via_dirty = render_with(&mut dirty, src, cfg, shortcut, size);
            let mut fresh = RenderScratch::new();
            let via_fresh = render_with(&mut fresh, src, cfg, shortcut, size);
            let via_free = free(src, cfg, shortcut, size);
            assert_eq!(via_dirty, via_fresh, "reused scratch diverged from a fresh scratch (stale buffer leak) at size {size}");
            assert_eq!(via_dirty, via_free, "reused scratch diverged from the free render_tile oracle at size {size}");
        }
    }
}
