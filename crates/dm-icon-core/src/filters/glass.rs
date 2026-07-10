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

            let d = dist[i] / 3.0;
            let edge = libm::exp(-d / falloff);

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
