//! 单色 Material-style tonal duotone — 1:1 port of the frozen `color.ts`
//! ramp/stretch/map/transform tail (kept out of `color.rs` for the 500-line cap).
//!
//! The tint ramp is a 256-entry LUT built from `mono_tone` (8 gamut-fit steps
//! each), memoized per tint. The TS oracle caches it in a module `Map`; here a
//! thread-local mirror keeps `transform_pixel_in_place`'s per-pixel `monoRamp`
//! from rebuilding the LUT. Pure function of the tint, so the cache is
//! parity-neutral.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::color::{gray_value, luminance, mono_tone, perceived_lightness};
use crate::config::Subject;
use crate::js_math::{clamp_byte, clamp_u8_int, js_round};
use crate::raster::{Raster, Rgba};

thread_local! {
    /// Per-tint tonal ramp (color.ts `rampCache`); pure, so parity-neutral. Grows
    /// monotonically but is bounded in practice (one entry per distinct tint a session
    /// uses); never evicted, per the frozen oracle.
    static RAMP_CACHE: RefCell<HashMap<u32, [u8; 768]>> = RefCell::new(HashMap::new());
}

/// color.ts `buildRamp` — the darkest→lightest tonal ramp of the tint's hue.
fn build_ramp(tint: u32) -> [u8; 768] {
    let mut lut = [0u8; 768];
    for i in 0..256 {
        let mut t = i as f64 / 255.0;
        let sep = t * t * (3.0 - 2.0 * t);
        t = 0.42 * t + 0.58 * sep;
        let lightness = 0.4 + (0.965 - 0.4) * t;
        let chroma_scale = 1.15 + (0.22 - 1.15) * t;
        let tone = mono_tone(lightness, chroma_scale, tint);
        lut[i * 3] = tone.r;
        lut[i * 3 + 1] = tone.g;
        lut[i * 3 + 2] = tone.b;
    }
    lut
}

/// The memoized ramp LUT for a tint (a cheap 768-byte copy out of the cache).
fn ramp_lut(tint: u32) -> [u8; 768] {
    RAMP_CACHE.with(|c| {
        if let Some(l) = c.borrow().get(&tint) {
            return *l;
        }
        let lut = build_ramp(tint);
        c.borrow_mut().insert(tint, lut);
        lut
    })
}

/// The mono tonal ramp: `t∈[0,1]` darkest→lightest of the tint's hue
/// (color.ts `monoRamp`).
pub fn mono_ramp(t: f64, tint: u32) -> Rgba {
    let lut = ramp_lut(tint);
    let i = (js_round(t * 255.0) as i64).clamp(0, 255) as usize;
    Rgba { r: lut[i * 3], g: lut[i * 3 + 1], b: lut[i * 3 + 2], a: 255 }
}

/// Per-pixel ADAPTIVE-stretched lightness `t∈[0,1]`: the tile's visible P5-P95
/// range remapped to full scale (color.ts `stretchedLightness`). Transparent
/// pixels stay 0.
pub fn stretched_lightness(tile: &Raster) -> Vec<f64> {
    let d = &tile.data;
    let n = tile.width * tile.height;
    let mut hist = [0u32; 256];
    let mut light = vec![0u8; n];
    let mut result = vec![0.0f64; n];
    let mut visible = 0u32;
    for i in 0..n {
        let i4 = i * 4;
        if d[i4 + 3] == 0 {
            continue;
        }
        let v = clamp_byte(perceived_lightness(d[i4], d[i4 + 1], d[i4 + 2]) * 255.0);
        light[i] = v;
        hist[v as usize] += 1;
        visible += 1;
    }
    if visible == 0 {
        return result;
    }

    let percentile = |p: f64| -> u8 {
        let target = visible as f64 * p;
        let mut cum = 0u32;
        for (v, &count) in hist.iter().enumerate() {
            cum += count;
            if cum as f64 >= target {
                return v as u8;
            }
        }
        255
    };
    let lo = percentile(0.05);
    let hi = percentile(0.95);
    let span = hi as i32 - lo as i32;
    let stretch = span >= 26;
    for i in 0..n {
        if d[i * 4 + 3] == 0 {
            continue;
        }
        result[i] = if stretch {
            ((light[i] as f64 - lo as f64) / span as f64).clamp(0.0, 1.0)
        } else {
            light[i] as f64 / 255.0
        };
    }
    result
}

/// Whole-tile adaptive 单色 mapping (color.ts `monoMapAdaptive`).
pub fn mono_map_adaptive(tile: &mut Raster, tint: u32) {
    let t = stretched_lightness(tile);
    let lut = ramp_lut(tint);
    let d = &mut tile.data;
    for (i, &ti) in t.iter().enumerate() {
        let i4 = i * 4;
        if d[i4 + 3] == 0 {
            continue;
        }
        let li = (js_round(ti * 255.0) as i64).clamp(0, 255) as usize;
        d[i4] = lut[li * 3];
        d[i4 + 1] = lut[li * 3 + 1];
        d[i4 + 2] = lut[li * 3 + 2];
    }
}

/// Per-pixel recolour for 黑白 / per-pixel 单色 (color.ts `transformPixelInPlace`).
/// `Original` is identity; `BlackWhite` is Rec.601 gray; `Mono` rides the ramp.
pub fn transform_pixel_in_place(d: &mut [u8], i4: usize, mode: Subject, tint: u32) {
    if mode == Subject::Original || d[i4 + 3] == 0 {
        return;
    }
    if mode == Subject::BlackWhite {
        let v = clamp_u8_int(gray_value(luminance(d[i4], d[i4 + 1], d[i4 + 2])));
        d[i4] = v;
        d[i4 + 1] = v;
        d[i4 + 2] = v;
        return;
    }
    let t = perceived_lightness(d[i4], d[i4 + 1], d[i4 + 2]);
    let toned = mono_ramp(t, tint);
    d[i4] = toned.r;
    d[i4 + 1] = toned.g;
    d[i4 + 2] = toned.b;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ramp_endpoints_track_lightness() {
        // A blue tint: t=0 is the dark end, t=1 the pale end.
        let dark = mono_ramp(0.0, 0x2266cc);
        let light = mono_ramp(1.0, 0x2266cc);
        let ld = perceived_lightness(dark.r, dark.g, dark.b);
        let ll = perceived_lightness(light.r, light.g, light.b);
        assert!(ll > ld, "ramp light end {ll} must exceed dark end {ld}");
        assert!(ll > 0.9, "pale end reads near-white, got {ll}");
    }

    #[test]
    fn ramp_is_cached_deterministic() {
        assert_eq!(mono_ramp(0.5, 0xff6f5e), mono_ramp(0.5, 0xff6f5e));
    }

    #[test]
    fn stretched_lightness_empty_is_zero() {
        let r = Raster::new(4, 4);
        assert!(stretched_lightness(&r).iter().all(|&v| v == 0.0));
    }

    #[test]
    fn stretched_lightness_flat_tile_passes_through() {
        // A uniform mid-gray tile has span 0 (< 26) → no stretch, raw l/255.
        let mut r = Raster::new(4, 4);
        for i in 0..16 {
            r.data[i * 4] = 128;
            r.data[i * 4 + 1] = 128;
            r.data[i * 4 + 2] = 128;
            r.data[i * 4 + 3] = 255;
        }
        let t = stretched_lightness(&r);
        let l = perceived_lightness(128, 128, 128);
        let expect = clamp_byte(l * 255.0) as f64 / 255.0;
        assert!(t.iter().all(|&v| (v - expect).abs() < 1e-12));
    }

    #[test]
    fn bw_transform_is_gray() {
        let mut d = vec![200u8, 40, 40, 255];
        transform_pixel_in_place(&mut d, 0, Subject::BlackWhite, 0);
        assert_eq!(d[0], d[1]);
        assert_eq!(d[1], d[2]);
        assert_eq!(d[3], 255);
    }

    #[test]
    fn original_transform_is_identity() {
        let mut d = vec![200u8, 40, 40, 255];
        let before = d.clone();
        transform_pixel_in_place(&mut d, 0, Subject::Original, 0xff6f5e);
        assert_eq!(d, before);
    }
}
