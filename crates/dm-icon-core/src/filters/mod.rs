//! 滤镜 — 1:1 port of the frozen `filters.ts`. Algorithmic finishes over the
//! COMPOSED, shape-clipped tile; marks draw after and adapt to the result.
//! Luminance-structure driven, never colour driven. Split by finish.

mod glass;
mod pixel;
mod sticker;

use crate::color::{srgb_decode, srgb_encode};
use crate::config::{FilterStyle, Subject};
use crate::js_math::clamp_byte;
use crate::raster::Raster;

/// Apply a finish over the composed tile (filters.ts `applyFilter`).
pub fn apply_filter(tile: &mut Raster, size: usize, filter: FilterStyle, subject: Subject, tint: u32) {
    // 玻璃 alone is colour-aware: Mono tiles keep their tinted ramp on the slab.
    let hue = if subject == Subject::Mono { Some(tint) } else { None };
    match filter {
        FilterStyle::Gloss => gloss(tile, size),
        FilterStyle::Glass => glass::glass(tile, size, hue),
        FilterStyle::Pixel => pixel::pixelate(tile, size),
        FilterStyle::Sticker => sticker::sticker(tile, size),
        FilterStyle::None => {}
    }
}

/// filters.ts `smoothStepRange`.
pub(crate) fn smooth_step_range(lo: f64, hi: f64, v: f64) -> f64 {
    let t = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ---- 光泽 (glossy sheen) ----

const GLOSS_SHEEN_TOP: f64 = 0.34;
const GLOSS_SHEEN_EDGE: f64 = 0.12;
const GLOSS_DEPTH: f64 = 0.07;

fn gloss(tile: &mut Raster, size: usize) {
    // A 1×1 tile has no gradient axis: `y / (size - 1)` is `0 / 0 = NaN`, which
    // propagates through the sheen math to `clamp_byte(NaN)` and silently blacks
    // out the single opaque pixel. Nothing to shade at that size — leave it. (ICON-2)
    if size <= 1 {
        return;
    }
    let d = &mut tile.data;
    for y in 0..size {
        let v = y as f64 / (size - 1) as f64;
        for x in 0..size {
            let i4 = (y * size + x) * 4;
            if d[i4 + 3] == 0 {
                continue;
            }
            let u = x as f64 / (size - 1) as f64;
            let boundary = 0.42 - 0.105 * (1.0 - (2.0 * u - 1.0) * (2.0 * u - 1.0));
            if v < boundary {
                let fade = smooth_step_range(0.0, 0.05, boundary - v);
                let a = (GLOSS_SHEEN_EDGE + (GLOSS_SHEEN_TOP - GLOSS_SHEEN_EDGE) * (1.0 - v / boundary)) * fade;
                for c in 0..3 {
                    let lin = srgb_decode(d[i4 + c]);
                    d[i4 + c] = srgb_encode(lin + (1.0 - lin) * a);
                }
            } else {
                let depth = smooth_step_range(boundary, 1.0, v) * GLOSS_DEPTH;
                d[i4] = clamp_byte(d[i4] as f64 * (1.0 - depth));
                d[i4 + 1] = clamp_byte(d[i4 + 1] as f64 * (1.0 - depth));
                d[i4 + 2] = clamp_byte(d[i4 + 2] as f64 * (1.0 - depth));
            }
        }
    }
}

/// Two-pass 3-4 chamfer distance transform (filters.ts `chamferDistance`).
/// inside: distance from opaque pixels to the nearest transparency (−1 on
/// transparent); outside: the reverse. Units are chamfer weights (÷3 ≈ px).
pub(crate) fn chamfer_distance(tile: &Raster, size: usize, inside: bool) -> Vec<f64> {
    let inf = f64::MAX / 4.0;
    let d = &tile.data;
    let mut dist = vec![0.0f64; size * size];
    for (i, slot) in dist.iter_mut().enumerate() {
        let opaque = d[i * 4 + 3] >= 32;
        *slot = if opaque == inside { inf } else { -1.0 };
    }

    let cost = |dist: &[f64], x: isize, y: isize, w: f64| -> f64 {
        if x < 0 || y < 0 || x >= size as isize || y >= size as isize {
            return if inside { w } else { inf };
        }
        let v = dist[y as usize * size + x as usize];
        if v < 0.0 {
            w
        } else {
            v + w
        }
    };

    for y in 0..size {
        for x in 0..size {
            let i = y * size + x;
            if dist[i] < 0.0 {
                continue;
            }
            let (xi, yi) = (x as isize, y as isize);
            let mut best = dist[i];
            best = best.min(cost(&dist, xi - 1, yi, 3.0));
            best = best.min(cost(&dist, xi, yi - 1, 3.0));
            best = best.min(cost(&dist, xi - 1, yi - 1, 4.0));
            best = best.min(cost(&dist, xi + 1, yi - 1, 4.0));
            dist[i] = best;
        }
    }
    for y in (0..size).rev() {
        for x in (0..size).rev() {
            let i = y * size + x;
            if dist[i] < 0.0 {
                continue;
            }
            let (xi, yi) = (x as isize, y as isize);
            let mut best = dist[i];
            best = best.min(cost(&dist, xi + 1, yi, 3.0));
            best = best.min(cost(&dist, xi, yi + 1, 3.0));
            best = best.min(cost(&dist, xi - 1, yi + 1, 4.0));
            best = best.min(cost(&dist, xi + 1, yi + 1, 4.0));
            dist[i] = best;
        }
    }
    dist
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raster::Raster;

    fn centred_square(size: usize, lo: usize, hi: usize) -> Raster {
        let mut tile = Raster::new(size, size);
        for y in lo..hi {
            for x in lo..hi {
                tile.data[(y * size + x) * 4 + 3] = 255;
            }
        }
        tile
    }

    #[test]
    fn chamfer_inside_and_outside_are_polar_and_grow_away_from_the_edge() {
        let size = 16;
        let tile = centred_square(size, 4, 12);

        // inside: transparency is -1; opaque distance grows toward the centre.
        let inside = chamfer_distance(&tile, size, true);
        assert_eq!(inside[0], -1.0, "a transparent corner is -1 for the inside transform");
        let centre = inside[8 * size + 8];
        let edge = inside[4 * size + 8]; // the square's top edge row
        assert!(edge >= 0.0 && centre > edge, "inside distance deepens toward the centre");

        // outside: opaque is -1; transparency carries the distance.
        let outside = chamfer_distance(&tile, size, false);
        assert_eq!(outside[8 * size + 8], -1.0, "an opaque pixel is -1 for the outside transform");
        assert!(outside[0] > 0.0, "a far transparent corner has a positive outside distance");
    }

    #[test]
    fn gloss_leaves_a_1x1_opaque_tile_its_true_colour() {
        // ICON-2: at size==1 the sheen gradient is `0 / (1-1) = NaN`, which used to
        // propagate to `clamp_byte(NaN)` and black out the single opaque pixel.
        let mut tile = Raster::new(1, 1);
        tile.data.copy_from_slice(&[10, 20, 30, 255]);
        gloss(&mut tile, 1);
        assert_eq!(tile.data, [10, 20, 30, 255], "size==1 gloss must not corrupt the pixel to black");
    }
}
