//! 像素 (kawaii pixel-art) — 1:1 port of the frozen `filters.ts` pixelate.
//! Cell-grid linear-light average → candy palette → contour ring → block expand.
//! `candy`/`posterize` cast with `Math.trunc` (NOT clamped-array rounding).

use crate::color::{luminance, srgb_decode, srgb_encode};
use crate::js_math::{clamp_byte, clamp_u8_int, js_round};
use crate::raster::Raster;

const PIXEL_CELLS: usize = 24;
const OUTLINE_R: f64 = 24.0;
const OUTLINE_G: f64 = 24.0;
const OUTLINE_B: f64 = 30.0;

pub fn pixelate(tile: &mut Raster, size: usize) {
    let cell = size as f64 / PIXEL_CELLS as f64;
    let mut colors = vec![0u8; PIXEL_CELLS * PIXEL_CELLS * 3];
    let mut opaque = vec![0u8; PIXEL_CELLS * PIXEL_CELLS];

    // 1) Cell grid: linear-light box average + candy palette.
    for cy in 0..PIXEL_CELLS {
        let y0 = js_round(cy as f64 * cell) as usize;
        let y1 = (js_round((cy as f64 + 1.0) * cell) as usize).min(size);
        for cx in 0..PIXEL_CELLS {
            let x0 = js_round(cx as f64 * cell) as usize;
            let x1 = (js_round((cx as f64 + 1.0) * cell) as usize).min(size);
            let mut r = 0.0f64;
            let mut g = 0.0f64;
            let mut b = 0.0f64;
            let mut a = 0.0f64;
            let mut n = 0u32;
            for y in y0..y1 {
                for x in x0..x1 {
                    let i4 = (y * size + x) * 4;
                    let w = tile.data[i4 + 3] as f64 / 255.0;
                    r += srgb_decode(tile.data[i4]) * w;
                    g += srgb_decode(tile.data[i4 + 1]) * w;
                    b += srgb_decode(tile.data[i4 + 2]) * w;
                    a += w;
                    n += 1;
                }
            }
            let ci = cy * PIXEL_CELLS + cx;
            if n == 0 || (a / n as f64) < 0.5 {
                continue;
            }
            opaque[ci] = 1;
            let (cr, cg, cb) = candy(srgb_encode(r / a), srgb_encode(g / a), srgb_encode(b / a));
            colors[ci * 3] = cr;
            colors[ci * 3 + 1] = cg;
            colors[ci * 3 + 2] = cb;
        }
    }

    // 2) Contours: silhouette ring + darker side of strong internal edges.
    let mut outline = vec![0u8; opaque.len()];
    let neighbors: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
    for cy in 0..PIXEL_CELLS {
        for cx in 0..PIXEL_CELLS {
            let ci = cy * PIXEL_CELLS + cx;
            if opaque[ci] == 0 {
                continue;
            }
            let lum = luminance(colors[ci * 3], colors[ci * 3 + 1], colors[ci * 3 + 2]);
            for (dx, dy) in neighbors {
                let nx = cx as i32 + dx;
                let ny = cy as i32 + dy;
                if nx < 0
                    || ny < 0
                    || nx >= PIXEL_CELLS as i32
                    || ny >= PIXEL_CELLS as i32
                    || opaque[ny as usize * PIXEL_CELLS + nx as usize] == 0
                {
                    outline[ci] = 1;
                    break;
                }
                let ni = ny as usize * PIXEL_CELLS + nx as usize;
                let nl = luminance(colors[ni * 3], colors[ni * 3 + 1], colors[ni * 3 + 2]);
                if nl - lum > 0.3 {
                    outline[ci] = 1;
                    break;
                }
            }
        }
    }

    // 3) Paint: contour cells dark; the row under a top contour catches light.
    for cy in 0..PIXEL_CELLS {
        for cx in 0..PIXEL_CELLS {
            let ci = cy * PIXEL_CELLS + cx;
            if opaque[ci] == 0 {
                continue;
            }
            if outline[ci] != 0 {
                colors[ci * 3] = clamp_u8_int(OUTLINE_R * 0.75 + colors[ci * 3] as f64 * 0.25);
                colors[ci * 3 + 1] = clamp_u8_int(OUTLINE_G * 0.75 + colors[ci * 3 + 1] as f64 * 0.25);
                colors[ci * 3 + 2] = clamp_u8_int(OUTLINE_B * 0.75 + colors[ci * 3 + 2] as f64 * 0.25);
            } else if cy > 0 && outline[(cy - 1) * PIXEL_CELLS + cx] != 0 {
                colors[ci * 3] = clamp_u8_int(colors[ci * 3] as f64 + (255.0 - colors[ci * 3] as f64) * 0.22);
                colors[ci * 3 + 1] = clamp_u8_int(colors[ci * 3 + 1] as f64 + (255.0 - colors[ci * 3 + 1] as f64) * 0.22);
                colors[ci * 3 + 2] = clamp_u8_int(colors[ci * 3 + 2] as f64 + (255.0 - colors[ci * 3 + 2] as f64) * 0.22);
            }
        }
    }

    // 4) Expand cells back to pixels (nearest-neighbour blocks, hard alpha).
    for cy in 0..PIXEL_CELLS {
        let y0 = js_round(cy as f64 * cell) as usize;
        let y1 = (js_round((cy as f64 + 1.0) * cell) as usize).min(size);
        for cx in 0..PIXEL_CELLS {
            let x0 = js_round(cx as f64 * cell) as usize;
            let x1 = (js_round((cx as f64 + 1.0) * cell) as usize).min(size);
            let ci = cy * PIXEL_CELLS + cx;
            let on = opaque[ci] == 1;
            for y in y0..y1 {
                for x in x0..x1 {
                    let i4 = (y * size + x) * 4;
                    if on {
                        tile.data[i4] = colors[ci * 3];
                        tile.data[i4 + 1] = colors[ci * 3 + 1];
                        tile.data[i4 + 2] = colors[ci * 3 + 2];
                        tile.data[i4 + 3] = 255;
                    } else {
                        tile.data[i4] = 0;
                        tile.data[i4 + 1] = 0;
                        tile.data[i4 + 2] = 0;
                        tile.data[i4 + 3] = 0;
                    }
                }
            }
        }
    }
}

/// C# casts `(byte)` BEFORE Posterize — truncation, not rounding (filters.ts).
fn candy(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let m = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
    let br = (m + (r as f64 - m) * 1.3).clamp(0.0, 255.0).trunc();
    let bg = (m + (g as f64 - m) * 1.3).clamp(0.0, 255.0).trunc();
    let bb = (m + (b as f64 - m) * 1.3).clamp(0.0, 255.0).trunc();
    (posterize(br), posterize(bg), posterize(bb))
}

fn posterize(v: f64) -> u8 {
    let levels = 5.0;
    let q = js_round((v / 255.0) * (levels - 1.0)) / (levels - 1.0);
    clamp_byte(q * 255.0)
}
