//! 快捷方式标识 — 1:1 port of the frozen `marks.ts`. Marks are stateless; the
//! tile composer owns the z-order (behind siblings, over overlays). Trait +
//! shared helpers + the arrow glyph live here; the seven styles in `styles.rs`.

mod styles;

use std::sync::{Arc, RwLock};

use crate::analysis::ContentBounds;
use crate::config::{IconShape, MarkStyle};
use crate::filters::chamfer_distance;
use crate::js_math::{clamp_u8_int, js_round, js_trunc};
use crate::mask_cache::{MaskCache, MaskKey};
use crate::raster::{
    dist_to_segment, from_rgb_int, in_triangle, paint, shape_mask, smooth_step01, Raster, Rgba,
};
use crate::render_scratch::RenderScratch;
use crate::sampling::draw_scaled;

/// 品牌珊瑚 — accent used when the user has not chosen a mark colour.
pub const MARK_ACCENT: u32 = 0xff6f5e;
/// Mark adaptivity crossover (distinct from the 0.66 ink threshold).
pub const ADAPTIVE_THRESHOLD: f64 = 0.58;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Behind,
    Over,
}

pub struct MarkContext {
    pub size: usize,
    pub shape: IconShape,
    pub luminance: f64,
    pub mark_color: Option<u32>,
    /// The full-tile silhouette coverage. Shared read-only from the session mask
    /// cache (`Arc<[f64]>`) — marks only read it, so no per-cell copy (M6 Phase 1).
    pub tile_alpha: Arc<[f64]>,
}

/// A shortcut mark (marks.ts `Mark`). Defaults mirror the TS `base`.
pub trait Mark {
    fn placement(&self) -> Placement;
    fn card_inset(&self, _ctx: &MarkContext) -> usize {
        0
    }
    fn carves_card(&self) -> bool {
        false
    }
    fn carve_card(&self, _card_mask: &mut [f64], _ctx: &MarkContext) {}
    /// `scratch` carries the reusable render buffers (Glass frost/backdrop + seat); it is
    /// threaded alongside `masks` so a mark that allocates hot scratch can reuse it.
    fn render(&self, target: &mut Raster, card_mask: &[f64], ctx: &MarkContext, masks: &mut MaskCache, scratch: &mut RenderScratch);
}

pub(crate) fn is_light_tile(ctx: &MarkContext) -> bool {
    ctx.luminance > ADAPTIVE_THRESHOLD
}

pub(crate) fn mark_rgb(ctx: &MarkContext) -> Rgba {
    from_rgb_int(ctx.mark_color.unwrap_or(MARK_ACCENT))
}

/// A scaled/offset stamp of the mark geometry (marks.ts `stampMask`).
///
/// The `shape != None` branch is a pure geometry function (`shape_mask`), so it is
/// shared through the session mask cache — each offset variant is its own `MaskKey`,
/// and repeated Shadow renders of the same shape/size/offset collapse to one compute
/// (M6 Phase 1, review P3-1). The `None` branch reads `ctx.tile_alpha` (the per-cell
/// composed silhouette — NOT a geometry-only value), so it must NOT be cached by the
/// geometry key; it is recomputed every call.
pub(crate) fn stamp_mask(ctx: &MarkContext, mask_size: usize, off_x: f64, off_y: f64, masks: &mut MaskCache) -> Arc<[f64]> {
    // A mark whose inscribed geometry collapses to zero (tiny tiles: `size - 2*pad == 0`)
    // stamps nothing — guard both the shape_mask shapeSize assert and the None-shape
    // `size / mask_size` divide. Never fires at the 256² master (insets are a few px).
    if mask_size == 0 {
        return Arc::from(vec![0.0; ctx.size * ctx.size]);
    }
    if ctx.shape != IconShape::None {
        return masks.get_or_compute(MaskKey::new(ctx.shape, ctx.size, mask_size, off_x, off_y), || {
            shape_mask(ctx.shape, ctx.size, mask_size, off_x, off_y)
        });
    }
    let size = ctx.size;
    let mut out = vec![0.0f64; size * size];
    let scale = size as f64 / mask_size as f64;
    for y in 0..size {
        for x in 0..size {
            let sx = js_round((x as f64 - off_x) * scale) as i64;
            let sy = js_round((y as f64 - off_y) * scale) as i64;
            if sx >= 0 && sy >= 0 && sx < size as i64 && sy < size as i64 {
                out[y * size + x] = ctx.tile_alpha[sy as usize * size + sx as usize];
            }
        }
    }
    Arc::from(out)
}

/// Outside-distance (px) from a coverage field's silhouette (marks.ts `outsideDistance`).
pub(crate) fn outside_distance(field: &[f64], size: usize) -> Vec<f64> {
    let mut probe = Raster::new(size, size);
    for (i, &v) in field.iter().enumerate() {
        probe.data[i * 4 + 3] = if v >= 0.5 { 255 } else { 0 };
    }
    chamfer_distance(&probe, size, false)
}

pub(crate) fn lerp_rgba(a: Rgba, b: Rgba, t: f64) -> Rgba {
    Rgba {
        r: clamp_u8_int(js_round(a.r as f64 + (b.r as f64 - a.r as f64) * t)),
        g: clamp_u8_int(js_round(a.g as f64 + (b.g as f64 - a.g as f64) * t)),
        b: clamp_u8_int(js_round(a.b as f64 + (b.b as f64 - a.b as f64) * t)),
        a: clamp_u8_int(js_round(a.a as f64 + (b.a as f64 - a.a as f64) * t)),
    }
}

/// Straight-alpha "over" for two colours (marks.ts `overRgba`, Glass frost).
pub(crate) fn over_rgba(top: Rgba, bottom: Rgba) -> Rgba {
    if top.a == 0 {
        return bottom;
    }
    if top.a == 255 {
        return top;
    }
    let ta = top.a as f64 / 255.0;
    let ba = bottom.a as f64 / 255.0;
    let out_a = ta + ba * (1.0 - ta);
    if out_a <= 0.0 {
        return Rgba { r: 0, g: 0, b: 0, a: 0 };
    }
    let inv = 1.0 / out_a;
    Rgba {
        r: clamp_u8_int(js_round((top.r as f64 * ta + bottom.r as f64 * ba * (1.0 - ta)) * inv)),
        g: clamp_u8_int(js_round((top.g as f64 * ta + bottom.g as f64 * ba * (1.0 - ta)) * inv)),
        b: clamp_u8_int(js_round((top.b as f64 * ta + bottom.b as f64 * ba * (1.0 - ta)) * inv)),
        a: clamp_u8_int(js_round(out_a * 255.0)),
    }
}

// ---- ArrowGlyph + classic arrow ----

/// The one NE "↗" arrow glyph (marks.ts `drawArrowGlyph`).
#[allow(clippy::too_many_arguments)]
pub fn draw_arrow_glyph(
    target: &mut Raster,
    size: usize,
    cx: f64,
    cy: f64,
    reach: f64,
    ink: Rgba,
    clip: &[f64],
) {
    if reach <= 0.0 {
        return;
    }
    let (tail_u, tail_v) = (-0.44, 0.44);
    let (neck_u, neck_v) = (0.12, -0.12);
    let (tip_u, tip_v) = (0.48, -0.48);
    let shaft_half = 0.135;
    let head_half = 0.28;
    let perp = 0.70710678;
    let head_ax = neck_u + perp * head_half;
    let head_ay = neck_v + perp * head_half;
    let head_bx = neck_u - perp * head_half;
    let head_by = neck_v - perp * head_half;
    let soft = 1.3 / reach;
    let r = libm::ceil(reach) + 2.0;

    let mut y = js_trunc(cy - r) as i64;
    while (y as f64) <= cy + r {
        if y >= 0 && (y as usize) < size {
            let mut x = js_trunc(cx - r) as i64;
            while (x as f64) <= cx + r {
                if x >= 0 && (x as usize) < size {
                    let i = y as usize * size + x as usize;
                    let clip_cov = clip[i];
                    if clip_cov > 0.0 {
                        let u = (x as f64 + 0.5 - cx) / reach;
                        let v = (y as f64 + 0.5 - cy) / reach;
                        let d_shaft = dist_to_segment(u, v, tail_u, tail_v, neck_u, neck_v);
                        let cov = if in_triangle(u, v, tip_u, tip_v, head_ax, head_ay, head_bx, head_by) {
                            1.0
                        } else {
                            smooth_step01((shaft_half - d_shaft) / soft)
                        };
                        paint(target, i, ink, cov * clip_cov);
                    }
                }
                x += 1;
            }
        }
        y += 1;
    }
}

const ARROW_PLATE: Rgba = Rgba { r: 244, g: 244, b: 241, a: 245 };
const ARROW_GLYPH: Rgba = Rgba { r: 46, g: 50, b: 56, a: 255 };

// The GENUINE Win11 shortcut-arrow badge (owner-extracted). Mirrors the TS
// module global `nativeArrow`; the runner sets it once at boot. This is a
// process-global (not thread-local): every render thread in the M6 worker pool
// must see the same raster the runner installed. A thread-local silently drops
// off-thread renders to the DRAWN fallback, which breaks native↔wasm byte-parity.
// Read-mostly (one clone per shortcut render, writes only at boot), so RwLock
// over Mutex — worker threads read concurrently.
static NATIVE_ARROW: RwLock<Option<Raster>> = RwLock::new(None);

/// marks.ts `setNativeArrowRaster`.
pub fn set_native_arrow_raster(raster: Option<Raster>) {
    // Self-heal a poisoned lock (unwrap_or_else → the inner value): a panic on any
    // one render thread must not cascade into every later shortcut render across the
    // whole worker fleet. (ICON-3)
    *NATIVE_ARROW.write().unwrap_or_else(|e| e.into_inner()) = raster;
}

/// A snapshot of the boot-installed native arrow badge, or None. Used by the output
/// cache to fold the arrow into the content key (shortcut renders depend on it).
pub fn native_arrow() -> Option<Raster> {
    NATIVE_ARROW.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// 经典箭头 — the real system badge when available, else the drawn fallback
/// (marks.ts `drawClassicArrow`).
pub fn draw_classic_arrow(target: &mut Raster, size: usize) {
    if let Some(arrow) = native_arrow() {
        let box_size = 14.max(js_round(size as f64 * 0.28) as usize);
        draw_scaled(
            &arrow,
            ContentBounds { left: 0, top: 0, right: arrow.width, bottom: arrow.height },
            target,
            size,
            1,
            size as i32 - 1 - box_size as i32,
            box_size,
            box_size,
        );
        return;
    }
    let asz = 14.0_f64.max(size as f64 * 0.28);
    let left = 1.0;
    let top = size as f64 - 1.0 - asz;
    let radius = 4.0_f64.min(asz / 2.0);

    let mut plate = vec![0.0f64; size * size];
    let mut y = js_trunc(top - 1.0) as i64;
    while (y as f64) <= top + asz + 1.0 {
        if y >= 0 && (y as usize) < size {
            let mut x = js_trunc(left - 1.0) as i64;
            while (x as f64) <= left + asz + 1.0 {
                if x >= 0 && (x as usize) < size {
                    let cov = rounded_rect_coverage(x as f64 + 0.5, y as f64 + 0.5, left, top, asz, radius);
                    if cov > 0.0 {
                        let i = y as usize * size + x as usize;
                        plate[i] = cov;
                        paint(target, i, ARROW_PLATE, cov);
                    }
                }
                x += 1;
            }
        }
        y += 1;
    }
    draw_arrow_glyph(target, size, left + asz / 2.0, top + asz / 2.0, asz * 0.34, ARROW_GLYPH, &plate);
}

fn rounded_rect_coverage(px: f64, py: f64, rx: f64, ry: f64, side: f64, radius: f64) -> f64 {
    let cx = rx + side / 2.0;
    let cy = ry + side / 2.0;
    let half = side / 2.0 - radius;
    let qx = ((px - cx).abs() - half).max(0.0);
    let qy = ((py - cy).abs() - half).max(0.0);
    let d = libm::sqrt(qx * qx + qy * qy) - radius;
    (0.5 - d).clamp(0.0, 1.0)
}

/// marks.ts `resolveMark`.
pub fn resolve_mark(style: MarkStyle) -> &'static dyn Mark {
    match style {
        MarkStyle::Glass => &styles::GLASS_MARK,
        MarkStyle::Shadow => &styles::SHADOW_MARK,
        MarkStyle::Halo => &styles::HALO_MARK,
        MarkStyle::Satin => &styles::SATIN_MARK,
        MarkStyle::Arc => &styles::ARC_MARK,
        MarkStyle::Fold => &styles::FOLD_MARK,
        MarkStyle::Ring => &styles::RING_MARK,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    /// A sentinel arrow badge — solid opaque magenta. The DRAWN fallback only
    /// paints ARROW_PLATE / ARROW_GLYPH, so any magenta pixel in the output is
    /// proof the native raster path (not the fallback) was taken.
    const SENTINEL: Rgba = Rgba { r: 255, g: 0, b: 255, a: 255 };

    fn sentinel_arrow() -> Raster {
        let mut r = Raster::new(32, 32);
        for p in r.data.chunks_exact_mut(4) {
            p.copy_from_slice(&[SENTINEL.r, SENTINEL.g, SENTINEL.b, SENTINEL.a]);
        }
        r
    }

    fn sentinel_pixels(target: &Raster) -> usize {
        target
            .data
            .chunks_exact(4)
            .filter(|p| p == &[SENTINEL.r, SENTINEL.g, SENTINEL.b, SENTINEL.a])
            .count()
    }

    fn render_arrow(size: usize) -> Raster {
        let mut t = Raster::new(size, size);
        draw_classic_arrow(&mut t, size);
        t
    }

    /// P2-7 regression: the native arrow raster is installed on one thread and
    /// must be visible to a *different* render thread. With the old
    /// `thread_local!` storage the spawned thread saw `None` and silently fell
    /// back to the DRAWN arrow, breaking native↔wasm byte-parity the moment the
    /// M6 worker pool renders off the setter's thread. This is the exact failure.
    ///
    /// Both phases live in one `#[test]` because they both mutate the process
    /// global `NATIVE_ARROW`; splitting them would let the harness run them
    /// concurrently and race the global. Sequential ownership = deterministic.
    #[test]
    fn native_arrow_visible_across_render_threads() {
        const SIZE: usize = 64;

        // --- Phase 1: native raster installed on this thread ---
        set_native_arrow_raster(Some(sentinel_arrow()));

        // Render on the setter's thread (baseline native render).
        let main_render = render_arrow(SIZE);
        // Render on a *spawned* thread — the case the thread_local silently broke.
        let off_render = thread::spawn(|| render_arrow(SIZE)).join().unwrap();

        // The off-thread render must have used the native raster, not the fallback.
        assert!(
            sentinel_pixels(&off_render) > 0,
            "off-thread render fell back to the DRAWN arrow — native raster not visible across threads"
        );
        // And it must be byte-identical to the setter-thread render: same raster,
        // same bytes, on every worker thread (the byte-parity worker-pool invariant).
        assert_eq!(
            off_render.data, main_render.data,
            "off-thread render diverged from the setter-thread render"
        );

        // --- Phase 2: negative control — fallback still reachable when unset ---
        // Guards against Phase 1 passing for the wrong reason (e.g. both paths
        // producing the sentinel). Also restores the global for other tests.
        set_native_arrow_raster(None);
        let fallback = render_arrow(SIZE);
        assert_eq!(
            sentinel_pixels(&fallback),
            0,
            "no native raster installed, yet the sentinel colour appeared"
        );
        assert!(
            fallback.data.chunks_exact(4).any(|p| p[3] > 0),
            "drawn fallback painted nothing"
        );
    }
}

#[cfg(all(test, feature = "fast"))]
mod stamp_cache_cert {
    use super::*;
    use crate::mask_cache::MaskCache;

    fn none_ctx(silhouette: f64, size: usize) -> MarkContext {
        MarkContext {
            size,
            shape: IconShape::None,
            luminance: 0.5,
            mark_color: None,
            tile_alpha: Arc::from(vec![silhouette; size * size]),
        }
    }

    /// P3-1 (Codex round-3 caveat): the corpus has no None+Shadow reuse, so the
    /// None-branch exclusion is otherwise untested. A None-shape stamp is a function
    /// of the per-cell `tile_alpha`, NOT geometry, so it must NEVER enter the geometry
    /// cache — otherwise a second source with the same stamp geometry would read the
    /// first's cached silhouette. Guards against a future edit caching the None branch.
    #[test]
    fn none_shape_stamp_never_enters_the_geometry_cache() {
        let size = 64;
        // Two "sources": identical stamp geometry, DIFFERENT silhouettes.
        let a = none_ctx(1.0, size);
        let b = none_ctx(0.0, size);
        let mut masks = MaskCache::new();
        let sa = stamp_mask(&a, 50, 3.0, 4.0, &mut masks);
        let sb = stamp_mask(&b, 50, 3.0, 4.0, &mut masks); // SAME geometry key as A
        // If the None branch were cached by geometry, sb would be A's silhouette.
        assert_ne!(
            sa.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            sb.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            "None-shape stamp B read A's cached silhouette — the None branch was wrongly cached"
        );
        assert_eq!((masks.misses, masks.hits, masks.len()), (0, 0, 0), "None-shape stamp must never touch the geometry cache");
    }

    /// P3-1 positive: a `shape != None` stamp IS pure geometry, so repeated identical
    /// stamps (common: many Shadow cells of the same shape/size) collapse to one
    /// compute + cache hits, and every hit is bit-identical to the first compute.
    #[test]
    fn shadow_stamp_shares_the_cache_on_repeat() {
        let ctx = MarkContext {
            size: 256,
            shape: IconShape::Circle,
            luminance: 0.5,
            mark_color: None,
            tile_alpha: Arc::from(vec![1.0; 256 * 256]),
        };
        let mut masks = MaskCache::new();
        let first = stamp_mask(&ctx, 224, 14.0, 16.0, &mut masks);
        for _ in 0..7 {
            let again = stamp_mask(&ctx, 224, 14.0, 16.0, &mut masks);
            assert_eq!(
                first.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                again.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                "cached stamp diverged from the first compute"
            );
        }
        assert_eq!((masks.misses, masks.hits, masks.len()), (1, 7, 1), "8 identical Shadow stamps → 1 compute + 7 hits");
    }
}
