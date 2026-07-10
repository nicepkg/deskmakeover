//! 贴纸 (die-cut sticker) — 1:1 port of the frozen `filters.ts` sticker: shrink
//! the tile, then a white die-cut border + soft outer shadow from the chamfer
//! distance outside the shrunk silhouette.

use super::chamfer_distance;
use crate::js_math::{clamp_u8_int, js_round};
use crate::raster::Raster;
use crate::sampling::downscale;

pub fn sticker(tile: &mut Raster, size: usize) {
    let border = 3.0_f64.max(size as f64 * 0.05);
    let shadow = 2.0_f64.max(size as f64 * 0.016);
    let inset = (border + shadow + 1.0).ceil() as usize;

    // A tile smaller than the die-cut margins has no room for the sticker; mirror
    // the frozen TS silent no-op (a negative canvas size drew nothing) instead of
    // underflowing `size - 2 * inset`.
    let target = match size.checked_sub(2 * inset) {
        Some(t) if t > 0 => t,
        _ => {
            tile.data.fill(0);
            return;
        }
    };
    let clone = tile.clone();
    let shrunk = downscale(&clone, target);
    tile.data.fill(0);
    for y in 0..target {
        for x in 0..target {
            let s4 = (y * target + x) * 4;
            let d4 = ((y + inset) * size + x + inset) * 4;
            tile.data[d4] = shrunk.data[s4];
            tile.data[d4 + 1] = shrunk.data[s4 + 1];
            tile.data[d4 + 2] = shrunk.data[s4 + 2];
            tile.data[d4 + 3] = shrunk.data[s4 + 3];
        }
    }

    let dist = chamfer_distance(tile, size, false);
    let td = &mut tile.data;
    for i in 0..dist.len() {
        if dist[i] < 0.0 {
            continue;
        }
        let d = dist[i] / 3.0;
        let i4 = i * 4;
        if d <= border {
            let coverage = (border + 0.75 - d).clamp(0.0, 1.0);
            td[i4] = 253;
            td[i4 + 1] = 253;
            td[i4 + 2] = 251;
            td[i4 + 3] = clamp_u8_int(js_round(coverage * 255.0));
        } else if d <= border + shadow {
            let fade_t = 1.0 - (d - border) / shadow;
            td[i4] = 20;
            td[i4 + 1] = 22;
            td[i4 + 2] = 26;
            td[i4 + 3] = clamp_u8_int(js_round(46.0 * fade_t * fade_t));
        }
    }
}
