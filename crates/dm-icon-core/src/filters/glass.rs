//! 玻璃 (liquid glass) — 1:1 port of the frozen `filters.ts` glass finish:
//! translucent slab body, frosted subject, fresnel/specular, rim refraction, and
//! a grounding halo just outside the slab. Colour-aware for Mono (tinted ramp).

use super::{chamfer_distance, smooth_step_range};
use crate::color::pale_tone;
use crate::js_math::{clamp_byte, clamp_u8_int, js_round};
use crate::mono::{mono_ramp, stretched_lightness};
use crate::raster::Raster;

const PLATE_BODY_ALPHA: f64 = 0.44;
const PLATE_FRESNEL_ALPHA: f64 = 0.16;
const GLYPH_ALPHA: f64 = 0.94;

/// Bit-exact cache of the Glass rim falloff `libm::exp(-(dist/3.0)/falloff)`, keyed by the
/// INTEGER chamfer distance. On a HIT it returns the identical bits a fresh `exp` would —
/// the cache replaces the transcendental, never the value; a MISS runs precisely the
/// unchanged expression `let d = dist/3.0; libm::exp(-d/falloff)` on the ACTUAL f64 `dist`.
///
/// Subtlety (the byte-safety hinge): Glass calls this for every pixel it considers opaque
/// (`alpha != 0`), but `chamfer_distance` seeds pixels with `alpha < 32` as TRANSPARENT and
/// leaves them at the sentinel `−1.0` (not a real distance). So the semi-transparent
/// anti-aliased band carries `dist == −1.0` here — negative, and it maps to a DISTINCT edge
/// value from `dist == 0`. Those are passed through to the direct expression and NEVER
/// keyed. Only the real distances — nonnegative integers (sums of 3.0/4.0 + min), which
/// round-trip `dist as usize as f64 == dist` exactly — are Vec-cached. `cache` is
/// per-`glass`/per-size scratch (size, hence `falloff`, is fixed within a call); `NaN` marks
/// an unfilled slot (the expression is finite for every finite input, so NaN is never a real
/// cached value).
#[inline]
fn glass_edge(cache: &mut Vec<f64>, dist: f64, falloff: f64) -> f64 {
    // The `−1.0` chamfer seed of the semi-transparent edge band is not a real distance:
    // compute it directly (byte-identical to the unchanged expression) and never key it.
    if dist < 0.0 {
        let d = dist / 3.0;
        return libm::exp(-d / falloff);
    }
    debug_assert!(dist.is_finite() && dist.fract() == 0.0, "chamfer distance not a nonneg integer: {dist}");
    let key = dist as usize;
    debug_assert_eq!(key as f64, dist, "chamfer distance does not round-trip its key");
    if key >= cache.len() {
        cache.resize(key + 1, f64::NAN);
    }
    let cached = cache[key];
    if cached.is_nan() {
        let d = dist / 3.0;
        let v = libm::exp(-d / falloff);
        cache[key] = v;
        v
    } else {
        cached
    }
}

pub fn glass(tile: &mut Raster, size: usize, hue: Option<u32>) {
    let t = stretched_lightness(tile);

    let (plate_r, plate_g, plate_b): (f64, f64, f64);
    let (glyph_r, glyph_g, glyph_b): (f64, f64, f64);
    match hue {
        Some(h) => {
            let plate = pale_tone(h);
            plate_r = plate.r as f64;
            plate_g = plate.g as f64;
            plate_b = plate.b as f64;
            let pale = mono_ramp(0.985, h);
            glyph_r = (pale.r as u32 + 16).min(255) as f64;
            glyph_g = (pale.g as u32 + 16).min(255) as f64;
            glyph_b = (pale.b as u32 + 16).min(255) as f64;
        }
        None => {
            plate_r = 238.0;
            plate_g = 243.0;
            plate_b = 248.0;
            glyph_r = 252.0;
            glyph_g = 253.0;
            glyph_b = 255.0;
        }
    }

    let dist = chamfer_distance(tile, size, true);
    let outside = chamfer_distance(tile, size, false);
    let falloff = size as f64 * 0.05;
    let warp_px = size as f64 * 0.024;
    let halo_px = 2.0_f64.max(size as f64 * 0.014);

    let src = tile.clone();
    let sd = &src.data;
    let mut subject = vec![0.0f64; size * size];

    let dist_at = |x: isize, y: isize, fallback: f64| -> f64 {
        if x < 0 || y < 0 || x >= size as isize || y >= size as isize {
            return 0.0;
        }
        let v = dist[y as usize * size + x as usize];
        if v < 0.0 {
            fallback
        } else {
            v
        }
    };

    // Per-size scratch: the rim falloff is a pure function of the integer chamfer
    // distance, and there are far fewer distinct distances than opaque pixels. Not shared
    // across rayon workers (each `glass` call owns it).
    let mut edge_cache: Vec<f64> = Vec::new();

    let td = &mut tile.data;
    for y in 0..size {
        for x in 0..size {
            let i = y * size + x;
            let i4 = i * 4;
            if sd[i4 + 3] == 0 {
                // The grounding halo just outside the slab.
                let od = outside[i] / 3.0;
                if od >= 0.0 && od <= halo_px {
                    let fade_t = 1.0 - od / halo_px;
                    td[i4] = 12;
                    td[i4 + 1] = 14;
                    td[i4 + 2] = 18;
                    td[i4 + 3] = clamp_u8_int(js_round(34.0 * fade_t * fade_t));
                }
                continue;
            }

            let edge = glass_edge(&mut edge_cache, dist[i], falloff);

            let (xi, yi) = (x as isize, y as isize);
            let gx = dist_at(xi - 1, yi, dist[i]) - dist_at(xi + 1, yi, dist[i]);
            let gy = dist_at(xi, yi - 1, dist[i]) - dist_at(xi, yi + 1, dist[i]);
            let glen = libm::sqrt(gx * gx + gy * gy);
            let mut nx = 0.0;
            let mut ny = 0.0;
            if glen > 1e-6 {
                nx = gx / glen;
                ny = gy / glen;
            }

            // Refraction: content near the rim samples slightly inward.
            let warp = libm::pow(edge, 1.5) * warp_px;
            let sx = (js_round(x as f64 - nx * warp) as i64).clamp(0, size as i64 - 1) as usize;
            let sy = (js_round(y as f64 - ny * warp) as i64).clamp(0, size as i64 - 1) as usize;
            let si = sy * size + sx;
            let dense = 1.0 - if sd[si * 4 + 3] == 0 { t[i] } else { t[si] };

            subject[i] = smooth_step_range(0.48, 0.78, dense);

            let mut alpha = PLATE_BODY_ALPHA + PLATE_FRESNEL_ALPHA * edge + 0.06 * (1.0 - y as f64 / size as f64);
            let mut r = plate_r;
            let mut g = plate_g;
            let mut b = plate_b;

            let lx = -0.7071;
            let ly = -0.7071;
            let facing = nx * lx + ny * ly;
            if facing > 0.0 {
                let specular = edge * edge * facing * facing;
                r += (255.0 - r) * specular;
                g += (255.0 - g) * specular;
                b += (255.0 - b) * specular;
                alpha += specular * 0.24;
            } else {
                let shade = edge * facing * facing * 0.4;
                r *= 1.0 - 0.22 * shade;
                g *= 1.0 - 0.22 * shade;
                b *= 1.0 - 0.22 * shade;
            }

            let m = subject[i];
            r += (glyph_r - r) * m;
            g += (glyph_g - g) * m;
            b += (glyph_b - b) * m;
            alpha += (GLYPH_ALPHA - alpha) * m;

            alpha = alpha.min(0.96);
            td[i4] = clamp_byte(r);
            td[i4 + 1] = clamp_byte(g);
            td[i4 + 2] = clamp_byte(b);
            td[i4 + 3] = clamp_u8_int(js_round(alpha * sd[i4 + 3] as f64));
        }
    }

    // Soft drop shadow under the glyph (light from the top-left).
    let off = 1.max(js_round(size as f64 * 0.008) as usize);
    for y in 0..size {
        for x in 0..size {
            let i = y * size + x;
            let i4 = i * 4;
            if td[i4 + 3] == 0 || subject[i] > 0.15 {
                continue;
            }
            let sx2 = x.saturating_sub(off).min(size - 1);
            let sy2 = y.saturating_sub(off).min(size - 1);
            let shadow = subject[sy2 * size + sx2] * 0.3;
            if shadow > 0.01 {
                td[i4] = clamp_u8_int(js_round(td[i4] as f64 * (1.0 - shadow)));
                td[i4 + 1] = clamp_u8_int(js_round(td[i4 + 1] as f64 * (1.0 - shadow)));
                td[i4 + 2] = clamp_u8_int(js_round(td[i4 + 2] as f64 * (1.0 - shadow)));
                td[i4 + 3] = (td[i4 + 3] as i64 + js_round(shadow * 70.0) as i64).min(255) as u8;
            }
        }
    }
}

#[cfg(test)]
mod edge_cache_cert {
    use super::*;
    use crate::filters::chamfer_distance;

    /// The cached rim falloff must be BYTE-EXACT to the unchanged expression
    /// `let d = dist/3.0; libm::exp(-d/falloff)`, on both a cold miss and a warm hit, and
    /// for the `−1.0` seed of the semi-transparent band (the passthrough that must NOT be
    /// keyed). Compared via `to_bits`. This pins Win 2's invariant independently of the cert.
    #[test]
    fn cached_edge_is_bit_identical_to_direct_exp() {
        for &size in &[16usize, 64, 128, 256] {
            let falloff = size as f64 * 0.05;
            let mut cache: Vec<f64> = Vec::new();
            // `−1.0` (the anti-aliased-band seed) interleaved with real distances; out of
            // order + repeats so a key is served as a miss then a hit.
            for &dist in &[-1.0f64, 0.0, 3.0, 4.0, -1.0, 7.0, 12.0, 4.0, 0.0, 40.0, 7.0, 999.0, 40.0] {
                let direct = {
                    let d = dist / 3.0;
                    libm::exp(-d / falloff)
                };
                let cached = glass_edge(&mut cache, dist, falloff);
                assert_eq!(cached.to_bits(), direct.to_bits(), "size {size} dist {dist}: cached edge != direct exp");
            }
        }
    }

    /// A tile with an anti-aliased edge band (alpha 1..31) reproduces the exact hazard: those
    /// pixels are glass-opaque (`alpha != 0`) yet chamfer-transparent (`alpha < 32`), so they
    /// carry `dist == −1.0`, while alpha ≥ 32 pixels carry nonnegative integer distances that
    /// round-trip their key. `glass_edge` on EVERY real `dist` must equal the direct
    /// expression — the end-to-end proof the negative passthrough + integer key are byte-safe.
    #[test]
    fn cached_edge_matches_direct_on_real_chamfer_including_aa_band() {
        for &size in &[16usize, 33, 64] {
            let mut tile = Raster::new(size, size);
            for y in 0..size {
                for x in 0..size {
                    let i4 = (y * size + x) * 4;
                    let core = x >= size / 4 && x < 3 * size / 4 && y >= size / 4 && y < 3 * size / 4;
                    let band = x >= size / 4 - 1 && x < 3 * size / 4 + 1 && y >= size / 4 - 1 && y < 3 * size / 4 + 1;
                    tile.data[i4 + 3] = if core { 255 } else if band { 16 } else { 0 };
                }
            }
            let dist = chamfer_distance(&tile, size, true);
            let falloff = size as f64 * 0.05;
            let mut cache: Vec<f64> = Vec::new();
            let mut saw_negative = false;
            for (i, &v) in dist.iter().enumerate() {
                if tile.data[i * 4 + 3] == 0 {
                    continue; // glass skips fully-transparent pixels (never reaches glass_edge)
                }
                if v < 0.0 {
                    saw_negative = true; // the AA band's −1.0 seed
                }
                let direct = {
                    let d = v / 3.0;
                    libm::exp(-d / falloff)
                };
                assert_eq!(glass_edge(&mut cache, v, falloff).to_bits(), direct.to_bits(), "size {size} dist {v}");
            }
            assert!(saw_negative, "fixture did not exercise the −1.0 semi-transparent-band seed");
        }
    }
}
