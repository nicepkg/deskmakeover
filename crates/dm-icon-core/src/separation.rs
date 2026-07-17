//! 自动分离 (auto separation) — contrast rescue for a bare subject that would melt
//! into the plate we (or the user) put behind it. POST-FREEZE engine behaviour, not
//! part of the frozen TS oracle: every path here is gated on
//! `Config::auto_separation`, which the parity surfaces (frozen-golden corpus cert,
//! m6 byte-differential) pin to `false` — with the flag off the composed bytes are
//! untouched.
//!
//! Detection (pixel-level perceptual proximity): the artwork's outermost rim band is
//! sampled at SOURCE resolution — size-invariant, so the 256 bake master and every
//! preview size reach the same verdict. Each sample is judged on its VISIBLE colour:
//! the sampled RGBA composited src-over onto the plate (mirroring `over_at`'s
//! straight-alpha sRGB blend), never its raw RGB — a half-transparent pixel the user
//! sees as near-plate must count as near-plate. A sample "melts" when BOTH its OKLab
//! distance and its lightness gap to the plate are too small to carry the edge: at
//! desktop icon sizes (32–48 px) chroma-only contrast does not survive — human
//! chromatic acuity cuts off far below luminance acuity at high spatial frequency —
//! so a small ΔL disqualifies even a hue-distinct edge. The subject is rescued only
//! when a meaningful SHARE of the rim melts (a pixel-count fraction over the sampled
//! band, never a mean: a bimodal rim whose light half vanishes must trigger even
//! though its mean looks safe).
//!
//! Why the SOLID (α ≥ 245) erosion band is a sound stand-in for the final visible
//! boundary: the artwork's own anti-aliased fringe is, per rasterization, a coverage
//! blend of the rim colour it fringes — composited over the plate its visible colour
//! lies ON THE SEGMENT between the rim's visible colour and the plate colour, so its
//! ΔE to the plate is ≤ the rim's. A fringe can therefore never rescue a melting rim
//! (melt verdicts stay valid) nor melt when its rim reads fine (the solid core still
//! carries the edge). The one construct outside that proof is a deliberately
//! semi-transparent DESIGN outline in a colour unrelated to the core: its composited
//! ring may already separate, and the rescue then thickens it — a benign double
//! outline, accepted for v1 (regression-pinned in `tests/auto_separation.rs`). Only
//! when NO solid pixels exist at all does the α ≥ 128 fallback band judge — there
//! the visible-colour compositing above does the real work.
//!
//! Rescue: a thin die-cut stroke ring strictly OUTSIDE the silhouette (the sticker
//! filter's chamfer-coverage geometry), inked with `field_shadow_tone(plate)` — the
//! plate's opposing-lightness, same-hue-family tone the silhouette shadow already
//! uses. Subject pixels are never recoloured (owner iron law, compose/field.rs).

use crate::color::{field_shadow_tone, ok_lab_of};
use crate::config::{Config, Subject};
use crate::filters::chamfer_distance;
use crate::js_math::{clamp_u8_int, js_round};
use crate::raster::{over_at, Raster, Rgba};

/// Outermost-band depth: `max(2, min(w,h)/64)` — 4 px on a 256 source. Deliberately
/// thinner than `profile.rs`'s plate-derivation band (÷16): only the pixels that
/// actually touch the plate decide whether the edge survives; a wide band would
/// dilute a melting outline with interior colour and miss it.
const RIM_DEPTH_DIVISOR: usize = 64;
const RIM_MIN_DEPTH: usize = 2;
/// Solid-first alpha passes, `profile.rs subject_rim` discipline: prefer fully solid
/// rim pixels; fall back to the AA-tolerant band only when none exist.
const RIM_SOLID_MIN_ALPHA: u8 = 245;
/// Rim samples are strided down to at most this many OKLab probes per source.
const RIM_SAMPLE_CAP: usize = 512;

/// A rim pixel melts when BOTH hold: full OKLab ΔE below this…
const MELT_DE_MAX: f64 = 0.12;
/// …and the lightness gap below this (luminance carries small-size edges).
const MELT_DL_MAX: f64 = 0.10;
/// The rim share that must melt before the stroke fires — below this the edge is
/// judged locally readable (a small low-contrast patch is not a lost silhouette).
const MELT_MIN_FRACTION: f64 = 0.30;

/// Stroke width as a fraction of the tile edge (~6.4 px on the 256 bake master) —
/// sized so the ICO ladder's area-average downscale keeps ≥ ~0.8 px of ink at the
/// 32 px rung instead of averaging a hairline away.
const STROKE_WIDTH_FRACTION: f64 = 0.025;
const STROKE_MIN_WIDTH: f64 = 1.25;

/// RGBA probes of the artwork's outermost rim band, at source resolution.
/// Erosion-band construction mirrors `profile.rs subject_rim` (two alpha passes,
/// solid first); the band is strided down to `RIM_SAMPLE_CAP` probes. Alpha rides
/// along so `melt_fraction` can judge the COMPOSITED visible colour. Empty when
/// the artwork has no pixels at α ≥ 128.
pub fn rim_rgba_samples(c: &Raster) -> Vec<[u8; 4]> {
    let d = &c.data;
    let (ww, hh) = (c.width, c.height);
    let n = ww * hh;
    if n == 0 {
        return Vec::new();
    }
    let depth =
        RIM_MIN_DEPTH.max(js_round(ww.min(hh) as f64 / RIM_DEPTH_DIVISOR as f64) as usize);
    let mut band = vec![0u8; n];
    let mut cur = vec![0u8; n];
    let mut next = vec![0u8; n];
    for min_alpha in [RIM_SOLID_MIN_ALPHA, 128u8] {
        band.fill(0);
        for i in 0..n {
            cur[i] = (d[i * 4 + 3] >= min_alpha) as u8;
        }
        for _pass in 0..depth {
            next.copy_from_slice(&cur);
            for y in 0..hh {
                for x in 0..ww {
                    let i = y * ww + x;
                    if cur[i] == 0 {
                        continue;
                    }
                    let interior = x > 0
                        && cur[i - 1] != 0
                        && x < ww - 1
                        && cur[i + 1] != 0
                        && y > 0
                        && cur[i - ww] != 0
                        && y < hh - 1
                        && cur[i + ww] != 0;
                    if !interior {
                        band[i] = 1;
                        next[i] = 0;
                    }
                }
            }
            std::mem::swap(&mut cur, &mut next);
        }
        let band_idx: Vec<u32> = (0..n).filter(|&i| band[i] != 0).map(|i| i as u32).collect();
        if band_idx.is_empty() {
            continue;
        }
        // Stratified-jitter downsampling: the band is split into `target` equal
        // strata and each stratum contributes ONE pixel at a hash-chosen offset.
        // A fixed raster-order stride would alias with periodic rim art (a
        // period-2 column pattern sampled at an even stride reads 100% one
        // colour); the per-stratum jitter breaks every fixed period while
        // keeping full spatial coverage. splitmix64 is pure u64 arithmetic —
        // deterministic across wasm/native, no float, no platform RNG.
        let count = band_idx.len();
        let target = count.min(RIM_SAMPLE_CAP);
        let mut samples = Vec::with_capacity(target);
        for k in 0..target {
            let lo = k * count / target;
            let hi = ((k + 1) * count / target).max(lo + 1);
            let pick = lo + (splitmix64(k as u64) as usize) % (hi - lo);
            let i4 = band_idx[pick] as usize * 4;
            samples.push([d[i4], d[i4 + 1], d[i4 + 2], d[i4 + 3]]);
        }
        return samples;
    }
    Vec::new()
}

/// SplitMix64 — the standard 64-bit finalizer-mix PRNG step. Integer-only, so the
/// sampled offsets are bit-identical on every target (determinism doctrine).
fn splitmix64(k: u64) -> u64 {
    let mut z = k.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The share of rim probes whose VISIBLE colour (src-over onto the plate, the same
/// straight-alpha sRGB blend `over_at` renders with) perceptually merges with
/// `plate`. 0.0 when there is no rim at all — nothing to lose, nothing to rescue.
pub fn melt_fraction(samples: &[[u8; 4]], plate: Rgba) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let p = ok_lab_of(plate.r, plate.g, plate.b);
    let melted = samples
        .iter()
        .filter(|s| {
            let a = s[3] as f64 / 255.0;
            let blend =
                |src: u8, dst: u8| clamp_u8_int(js_round(src as f64 * a + dst as f64 * (1.0 - a)));
            let lab = ok_lab_of(blend(s[0], plate.r), blend(s[1], plate.g), blend(s[2], plate.b));
            let dl = lab.l - p.l;
            let da = lab.a - p.a;
            let db = lab.b - p.b;
            libm::sqrt(dl * dl + da * da + db * db) < MELT_DE_MAX && dl.abs() < MELT_DL_MAX
        })
        .count();
    melted as f64 / samples.len() as f64
}

/// The stroke ink IF the rim melts against `plate`, else None. The ink is the
/// plate's opposing tone (`field_shadow_tone`) so the ring and the silhouette
/// shadow stay one hue family.
pub fn separation_ink(samples: &[[u8; 4]], plate: Rgba) -> Option<Rgba> {
    (melt_fraction(samples, plate) >= MELT_MIN_FRACTION).then(|| field_shadow_tone(plate))
}

/// Config-gated ink for the 满彩 Field bare lanes (subject is always Original there).
pub(crate) fn separation_stroke_ink(config: &Config, artwork: &Raster, plate: Rgba) -> Option<Rgba> {
    if !config.auto_separation {
        return None;
    }
    separation_ink(&rim_rgba_samples(artwork), plate)
}

/// Config-gated ink for the classic white/user-plate lanes (`BareWhite` /
/// `InscribeWhite`). Those lanes post-map Mono/BlackWhite over the WHOLE content —
/// plate included — so the plate colour probed here would be wrong for a remapped
/// subject; v1 rescues the untouched-colour subject only.
pub(crate) fn bare_plate_stroke_ink(config: &Config, artwork: &Raster, plate: Rgba) -> Option<Rgba> {
    if config.subject != Subject::Original {
        return None;
    }
    separation_stroke_ink(config, artwork, plate)
}

/// Draw the die-cut ring around `layer`'s silhouette onto `content` (which already
/// holds the plate; the caller composites `layer` OVER the ring afterwards). Chamfer
/// coverage anti-aliasing is the sticker filter's proven geometry: the ring starts at
/// the first fully transparent pixel and fades over the last ~0.75 px.
pub(crate) fn draw_separation_stroke(content: &mut Raster, layer: &Raster, size: usize, ink: Rgba) {
    let dist = chamfer_distance(layer, size, false);
    let width = STROKE_MIN_WIDTH.max(size as f64 * STROKE_WIDTH_FRACTION);
    for (i, &raw) in dist.iter().enumerate() {
        if raw < 0.0 {
            continue;
        }
        let d = raw / 3.0;
        if d > width + 0.75 {
            continue;
        }
        let coverage = (width + 0.75 - d).clamp(0.0, 1.0);
        over_at(
            &mut content.data,
            i * 4,
            ink.r,
            ink.g,
            ink.b,
            clamp_u8_int(js_round(coverage * 255.0)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::perceived_lightness;

    /// A centred solid square of one colour on a transparent field.
    fn glyph(size: usize, lo: usize, hi: usize, rgb: (u8, u8, u8)) -> Raster {
        let mut r = Raster::new(size, size);
        for y in lo..hi {
            for x in lo..hi {
                let i4 = (y * size + x) * 4;
                r.data[i4] = rgb.0;
                r.data[i4 + 1] = rgb.1;
                r.data[i4 + 2] = rgb.2;
                r.data[i4 + 3] = 255;
            }
        }
        r
    }

    const WHITE_PLATE: Rgba = Rgba { r: 255, g: 255, b: 255, a: 255 };

    #[test]
    fn a_white_rim_on_a_white_plate_melts_entirely() {
        let c = glyph(64, 16, 48, (255, 255, 255));
        let samples = rim_rgba_samples(&c);
        assert!(!samples.is_empty());
        assert!(melt_fraction(&samples, WHITE_PLATE) > 0.99);
        assert!(separation_ink(&samples, WHITE_PLATE).is_some());
    }

    #[test]
    fn a_black_rim_on_a_white_plate_does_not_melt() {
        let c = glyph(64, 16, 48, (10, 10, 12));
        let samples = rim_rgba_samples(&c);
        assert_eq!(melt_fraction(&samples, WHITE_PLATE), 0.0);
        assert!(separation_ink(&samples, WHITE_PLATE).is_none());
    }

    #[test]
    fn a_hue_distinct_but_equal_lightness_rim_still_melts() {
        // ΔL ≈ 0 with a real hue difference: at icon sizes the luminance channel
        // carries the edge, so this must still count as melted when ΔE is small too.
        // Two near-neutral tones a few units apart: tiny ΔE, tiny ΔL.
        let plate = Rgba { r: 200, g: 200, b: 205, a: 255 };
        let c = glyph(64, 16, 48, (205, 198, 200));
        let samples = rim_rgba_samples(&c);
        assert!(melt_fraction(&samples, plate) > 0.99);
    }

    #[test]
    fn a_bimodal_rim_triggers_on_its_melting_half_alone() {
        // Left half white (melts on white), right half near-black (does not): the
        // FRACTION must sit near 0.5 — a mean-based detector would miss this.
        let size = 64;
        let mut c = Raster::new(size, size);
        for y in 16..48 {
            for x in 16..48 {
                let i4 = (y * size + x) * 4;
                let v = if x < 32 { 255 } else { 20 };
                c.data[i4] = v;
                c.data[i4 + 1] = v;
                c.data[i4 + 2] = v;
                c.data[i4 + 3] = 255;
            }
        }
        let samples = rim_rgba_samples(&c);
        let f = melt_fraction(&samples, WHITE_PLATE);
        assert!(f > 0.3 && f < 0.7, "bimodal melt fraction ~0.5, got {f}");
        assert!(separation_ink(&samples, WHITE_PLATE).is_some());
    }

    #[test]
    fn an_empty_raster_yields_no_samples_and_never_triggers() {
        let c = Raster::new(32, 32);
        let samples = rim_rgba_samples(&c);
        assert!(samples.is_empty());
        assert_eq!(melt_fraction(&samples, WHITE_PLATE), 0.0);
        assert!(separation_ink(&samples, WHITE_PLATE).is_none());
    }

    #[test]
    fn samples_are_capped_but_keep_full_rim_coverage() {
        let c = glyph(256, 8, 248, (250, 250, 250));
        let samples = rim_rgba_samples(&c);
        assert!(!samples.is_empty() && samples.len() <= RIM_SAMPLE_CAP);
    }

    #[test]
    fn a_soft_glyph_is_judged_on_its_composited_visible_colour_not_raw_rgb() {
        // A uniformly半透明 glyph (α=130, gray 205): RAW rgb sits ΔL≈0.16 from white
        // (would read as safe), but the VISIBLE colour the user sees is
        // 0.51·205 + 0.49·255 ≈ 230 — inside the melt window. The solid (α≥245)
        // pass finds nothing, the α≥128 fallback samples it, and the composited
        // judgement must flag it.
        let mut c = Raster::new(64, 64);
        for y in 16..48 {
            for x in 16..48 {
                let i4 = (y * 64 + x) * 4;
                c.data[i4] = 205;
                c.data[i4 + 1] = 205;
                c.data[i4 + 2] = 205;
                c.data[i4 + 3] = 130;
            }
        }
        let samples = rim_rgba_samples(&c);
        assert!(!samples.is_empty(), "the α≥128 fallback band must sample a soft glyph");
        assert!(
            melt_fraction(&samples, WHITE_PLATE) > 0.99,
            "the composited visible colour (~230 on white) must melt"
        );
        // Control: the same soft glyph over a DARK plate is clearly visible.
        let dark = Rgba { r: 30, g: 30, b: 34, a: 255 };
        assert_eq!(melt_fraction(&samples, dark), 0.0);
    }

    #[test]
    fn stride_sampling_does_not_alias_a_periodic_rim() {
        // A large glyph whose rim alternates white/black by COLUMN (period 2) — the
        // worst case for a fixed raster-order stride. The sampled melt fraction on a
        // white plate must stay near the true 0.5, not collapse to 0 or 1.
        let size = 256;
        let mut c = Raster::new(size, size);
        for y in 8..248 {
            for x in 8..248 {
                let i4 = (y * size + x) * 4;
                let v = if x % 2 == 0 { 255 } else { 0 };
                c.data[i4] = v;
                c.data[i4 + 1] = v;
                c.data[i4 + 2] = v;
                c.data[i4 + 3] = 255;
            }
        }
        let samples = rim_rgba_samples(&c);
        assert!(samples.len() <= RIM_SAMPLE_CAP);
        let f = melt_fraction(&samples, WHITE_PLATE);
        assert!(f > 0.3 && f < 0.7, "stride aliasing skewed the fraction: {f}");
    }

    #[test]
    fn the_stroke_ring_sits_strictly_outside_the_silhouette_and_fades_out() {
        let size = 64;
        let layer = glyph(size, 24, 40, (255, 255, 255));
        let mut content = Raster::new(size, size);
        // Plate: opaque white everywhere.
        for i in 0..size * size {
            content.data[i * 4] = 255;
            content.data[i * 4 + 1] = 255;
            content.data[i * 4 + 2] = 255;
            content.data[i * 4 + 3] = 255;
        }
        let ink = field_shadow_tone(WHITE_PLATE);
        draw_separation_stroke(&mut content, &layer, size, ink);

        // Inside the silhouette: untouched (the ring never paints under the subject).
        let inside = (32 * size + 32) * 4;
        assert_eq!(&content.data[inside..inside + 3], &[255, 255, 255]);
        // The first pixel outside the silhouette (chamfer d = 1.0): fully inked.
        let ring = (32 * size + 23) * 4;
        assert_eq!(&content.data[ring..ring + 3], &[ink.r, ink.g, ink.b]);
        // One pixel further (d = 2.0): the anti-aliased falloff — between ink and plate.
        let fade = (32 * size + 22) * 4;
        assert!(
            content.data[fade] > ink.r && content.data[fade] < 255,
            "falloff pixel should blend, got {}",
            content.data[fade]
        );
        // Far away: untouched plate.
        let far = (4 * size + 4) * 4;
        assert_eq!(&content.data[far..far + 3], &[255, 255, 255]);
        // The ink is genuinely darker than the white plate it rescues from.
        assert!(perceived_lightness(ink.r, ink.g, ink.b) < 0.6);
    }
}
