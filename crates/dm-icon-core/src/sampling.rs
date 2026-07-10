//! Resampling — 1:1 port of the frozen `sampling.ts` (TileRenderer.DrawScaled +
//! IconResampler.cs). Downscales are TRUE area averages in linear light with
//! premultiplied alpha; upscales are 4×4 supersampled premultiplied bilinear.
//! All accumulation is f64 in strict y-then-x order (no reassociation).

use crate::analysis::{bounds_h, bounds_w, ContentBounds};
use crate::color::{srgb_decode, srgb_encode};
use crate::js_math::{js_round, js_trunc};
use crate::raster::{over_at, Raster};

/// Premultiplied-alpha bilinear sample at fractional source coords
/// (sampling.ts `sampleBilinearAt`). Returns the `Math.round`ed 0-255 channel
/// values exactly as the TS tuple does — callers re-decode through the LUT.
pub fn sample_bilinear_at(src: &Raster, fx: f64, fy: f64) -> (u8, u8, u8, u8) {
    let fx = fx.max(0.0).min((src.width - 1) as f64);
    let fy = fy.max(0.0).min((src.height - 1) as f64);
    let x0 = fx.floor() as usize;
    let y0 = fy.floor() as usize;
    let x1 = (x0 + 1).min(src.width - 1);
    let y1 = (y0 + 1).min(src.height - 1);
    let tx = fx - x0 as f64;
    let ty = fy - y0 as f64;
    let d = &src.data;
    let i00 = (y0 * src.width + x0) * 4;
    let i10 = (y0 * src.width + x1) * 4;
    let i01 = (y1 * src.width + x0) * 4;
    let i11 = (y1 * src.width + x1) * 4;
    let w00 = (1.0 - tx) * (1.0 - ty);
    let w10 = tx * (1.0 - ty);
    let w01 = (1.0 - tx) * ty;
    let w11 = tx * ty;

    let a = w00 * d[i00 + 3] as f64
        + w10 * d[i10 + 3] as f64
        + w01 * d[i01 + 3] as f64
        + w11 * d[i11 + 3] as f64;
    if a <= 0.0 {
        return (0, 0, 0, 0);
    }
    let ch = |o: usize| -> f64 {
        (w00 * d[i00 + o] as f64 * d[i00 + 3] as f64
            + w10 * d[i10 + o] as f64 * d[i10 + 3] as f64
            + w01 * d[i01 + o] as f64 * d[i01 + 3] as f64
            + w11 * d[i11 + o] as f64 * d[i11 + 3] as f64)
            / a
    };
    (
        js_round(ch(0)) as u8,
        js_round(ch(1)) as u8,
        js_round(ch(2)) as u8,
        js_round(a) as u8,
    )
}

/// Draw src[bounds] scaled to (dstW×dstH) at (dstX,dstY), composited OVER the
/// square content raster (sampling.ts `drawScaled` dispatch: area average when
/// both axes shrink, 4×4 supersample otherwise).
#[allow(clippy::too_many_arguments)]
pub fn draw_scaled(
    src: &Raster,
    b: ContentBounds,
    content: &mut Raster,
    size: usize,
    dst_x: i32,
    dst_y: i32,
    dst_w: usize,
    dst_h: usize,
) {
    if bounds_w(b) >= dst_w && bounds_h(b) >= dst_h {
        draw_area_averaged(src, b, content, size, dst_x, dst_y, dst_w, dst_h);
    } else {
        draw_supersampled(src, b, content, size, dst_x, dst_y, dst_w, dst_h);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_area_averaged(
    src: &Raster,
    b: ContentBounds,
    content: &mut Raster,
    size: usize,
    dst_x: i32,
    dst_y: i32,
    dst_w: usize,
    dst_h: usize,
) {
    let bw = bounds_w(b) as f64;
    let bh = bounds_h(b) as f64;
    let scale_x = bw / dst_w as f64;
    let scale_y = bh / dst_h as f64;
    let sd = &src.data;
    for yy in 0..dst_h {
        let ty = dst_y + yy as i32;
        if ty < 0 || ty >= size as i32 {
            continue;
        }
        let top = b.top as f64 + yy as f64 * scale_y;
        let bottom = b.top as f64 + (yy as f64 + 1.0) * scale_y;
        let y0 = js_trunc(top).max(0.0) as usize;
        let y1 = (bottom.ceil() as usize).min(src.height);
        for xx in 0..dst_w {
            let tx = dst_x + xx as i32;
            if tx < 0 || tx >= size as i32 {
                continue;
            }
            let left = b.left as f64 + xx as f64 * scale_x;
            let right = b.left as f64 + (xx as f64 + 1.0) * scale_x;
            let x0 = js_trunc(left).max(0.0) as usize;
            let x1 = (right.ceil() as usize).min(src.width);

            // Linear-light, alpha-premultiplied accumulation (strict order).
            let mut r = 0.0f64;
            let mut g = 0.0f64;
            let mut bl = 0.0f64;
            let mut a_sum = 0.0f64;
            let mut area = 0.0f64;
            for y in y0..y1 {
                let hy = ((y + 1) as f64).min(bottom) - (y as f64).max(top);
                if hy <= 0.0 {
                    continue;
                }
                for x in x0..x1 {
                    let wx = ((x + 1) as f64).min(right) - (x as f64).max(left);
                    if wx <= 0.0 {
                        continue;
                    }
                    let w = wx * hy;
                    let i4 = (y * src.width + x) * 4;
                    let af = (sd[i4 + 3] as f64 / 255.0) * w;
                    r += srgb_decode(sd[i4]) * af;
                    g += srgb_decode(sd[i4 + 1]) * af;
                    bl += srgb_decode(sd[i4 + 2]) * af;
                    a_sum += sd[i4 + 3] as f64 * w;
                    area += w;
                }
            }
            if area <= 0.0 || a_sum <= 0.0 {
                continue;
            }
            let weight = a_sum / 255.0;
            let out_a = js_round(a_sum / area).clamp(0.0, 255.0);
            if out_a == 0.0 {
                continue;
            }
            over_at(
                &mut content.data,
                (ty as usize * size + tx as usize) * 4,
                srgb_encode(r / weight),
                srgb_encode(g / weight),
                srgb_encode(bl / weight),
                out_a as u8,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_supersampled(
    src: &Raster,
    b: ContentBounds,
    content: &mut Raster,
    size: usize,
    dst_x: i32,
    dst_y: i32,
    dst_w: usize,
    dst_h: usize,
) {
    const SUB: usize = 4;
    let sub_step = 1.0 / SUB as f64;
    let bw = bounds_w(b) as f64;
    let bh = bounds_h(b) as f64;
    for yy in 0..dst_h {
        let ty = dst_y + yy as i32;
        if ty < 0 || ty >= size as i32 {
            continue;
        }
        for xx in 0..dst_w {
            let tx = dst_x + xx as i32;
            if tx < 0 || tx >= size as i32 {
                continue;
            }
            let mut r = 0.0f64;
            let mut g = 0.0f64;
            let mut bl = 0.0f64;
            let mut a = 0.0f64;
            for sy2 in 0..SUB {
                for sx2 in 0..SUB {
                    let sx = b.left as f64
                        + ((xx as f64 + (sx2 as f64 + 0.5) * sub_step) / dst_w as f64) * bw;
                    let sy = b.top as f64
                        + ((yy as f64 + (sy2 as f64 + 0.5) * sub_step) / dst_h as f64) * bh;
                    let (pr, pg, pb, pa) = sample_bilinear_at(src, sx - 0.5, sy - 0.5);
                    r += srgb_decode(pr) * pa as f64;
                    g += srgb_decode(pg) * pa as f64;
                    bl += srgb_decode(pb) * pa as f64;
                    a += pa as f64;
                }
            }
            if a <= 0.0 {
                continue;
            }
            over_at(
                &mut content.data,
                (ty as usize * size + tx as usize) * 4,
                srgb_encode(r / a),
                srgb_encode(g / a),
                srgb_encode(bl / a),
                js_round(a / (SUB * SUB) as f64) as u8,
            );
        }
    }
}

/// Bilinear sample in [0,1] UV space (sampling.ts `sampleBilinear`).
pub fn sample_bilinear(src: &Raster, u: f64, v: f64) -> (u8, u8, u8, u8) {
    sample_bilinear_at(src, u * src.width as f64 - 0.5, v * src.height as f64 - 0.5)
}

/// Linear-light premultiplied box average to target² (sampling.ts `downscale`).
/// A no-op copy when the target is not smaller than the source.
pub fn downscale(src: &Raster, target: usize) -> Raster {
    if target >= src.width {
        return src.clone();
    }
    let mut dst = Raster::new(target, target);
    let sw = src.width;
    let sh = src.height;
    let sd = &src.data;
    let scale_x = sw as f64 / target as f64;
    let scale_y = sh as f64 / target as f64;
    for dy in 0..target {
        let top = dy as f64 * scale_y;
        let bottom = (dy as f64 + 1.0) * scale_y;
        let y0 = js_trunc(top) as usize;
        let y1 = (bottom.ceil() as usize).min(sh);
        for dx in 0..target {
            let left = dx as f64 * scale_x;
            let right = (dx as f64 + 1.0) * scale_x;
            let x0 = js_trunc(left) as usize;
            let x1 = (right.ceil() as usize).min(sw);
            let mut rp = 0.0f64;
            let mut gp = 0.0f64;
            let mut bp = 0.0f64;
            let mut a_sum = 0.0f64;
            let mut area_sum = 0.0f64;
            for y in y0..y1 {
                let hy = ((y + 1) as f64).min(bottom) - (y as f64).max(top);
                if hy <= 0.0 {
                    continue;
                }
                for x in x0..x1 {
                    let wx = ((x + 1) as f64).min(right) - (x as f64).max(left);
                    if wx <= 0.0 {
                        continue;
                    }
                    let area = wx * hy;
                    let i4 = (y * sw + x) * 4;
                    let af = (sd[i4 + 3] as f64 / 255.0) * area;
                    rp += srgb_decode(sd[i4]) * af;
                    gp += srgb_decode(sd[i4 + 1]) * af;
                    bp += srgb_decode(sd[i4 + 2]) * af;
                    a_sum += sd[i4 + 3] as f64 * area;
                    area_sum += area;
                }
            }
            let o4 = (dy * target + dx) * 4;
            if area_sum <= 0.0 || a_sum <= 0.0 {
                continue;
            }
            let weight = a_sum / 255.0;
            dst.data[o4] = srgb_encode(rp / weight);
            dst.data[o4 + 1] = srgb_encode(gp / weight);
            dst.data[o4 + 2] = srgb_encode(bp / weight);
            dst.data[o4 + 3] = js_round(a_sum / area_sum).clamp(0.0, 255.0) as u8;
        }
    }
    dst
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker(w: usize, h: usize) -> Raster {
        let mut r = Raster::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let i4 = (y * w + x) * 4;
                let v = if (x + y) % 2 == 0 { 255 } else { 0 };
                r.data[i4] = v;
                r.data[i4 + 1] = v;
                r.data[i4 + 2] = v;
                r.data[i4 + 3] = 255;
            }
        }
        r
    }

    #[test]
    fn bilinear_at_integer_coords_is_the_pixel() {
        let src = checker(4, 4);
        assert_eq!(sample_bilinear_at(&src, 0.0, 0.0), (255, 255, 255, 255));
        assert_eq!(sample_bilinear_at(&src, 1.0, 0.0), (0, 0, 0, 255));
    }

    #[test]
    fn area_average_of_uniform_is_uniform() {
        let mut src = Raster::new(8, 8);
        for i in 0..64 {
            src.data[i * 4] = 200;
            src.data[i * 4 + 1] = 100;
            src.data[i * 4 + 2] = 50;
            src.data[i * 4 + 3] = 255;
        }
        let mut dst = Raster::new(4, 4);
        let b = ContentBounds { left: 0, top: 0, right: 8, bottom: 8 };
        draw_scaled(&src, b, &mut dst, 4, 0, 0, 4, 4);
        for i in 0..16 {
            assert_eq!(&dst.data[i * 4..i * 4 + 4], &[200, 100, 50, 255]);
        }
    }

    #[test]
    fn upscale_takes_the_supersampled_lane_and_fills() {
        let src = checker(2, 2);
        let mut dst = Raster::new(8, 8);
        let b = ContentBounds { left: 0, top: 0, right: 2, bottom: 2 };
        draw_scaled(&src, b, &mut dst, 8, 0, 0, 8, 8);
        assert!(dst.data.chunks(4).all(|p| p[3] == 255));
        // corners keep their source tone (255 at TL, 0 at TR after bilinear clamping)
        assert_eq!(dst.data[3], 255);
    }

    #[test]
    fn downscale_short_circuits_and_averages_uniform() {
        let mut src = Raster::new(8, 8);
        for i in 0..64 {
            src.data[i * 4] = 200;
            src.data[i * 4 + 1] = 100;
            src.data[i * 4 + 2] = 50;
            src.data[i * 4 + 3] = 255;
        }
        assert_eq!(downscale(&src, 8).data, src.data); // target >= src → clone
        let d = downscale(&src, 4);
        assert!(d.data.chunks(4).all(|p| p == [200, 100, 50, 255]));
    }

    #[test]
    fn sample_bilinear_uv_hits_pixel_centres() {
        let src = checker(4, 4);
        // u=0.125 maps to fx = 0.5-0.5 = 0.0 → the (0,0) pixel.
        assert_eq!(sample_bilinear(&src, 0.125, 0.125), (255, 255, 255, 255));
    }

    // ---- properties + degenerate inputs ----

    fn solid(w: usize, h: usize, c: [u8; 4]) -> Raster {
        let mut r = Raster::new(w, h);
        for p in r.data.chunks_exact_mut(4) {
            p.copy_from_slice(&c);
        }
        r
    }

    #[test]
    fn downscale_of_a_uniform_field_is_that_colour_at_every_target() {
        let src = solid(64, 64, [173, 92, 210, 255]);
        for target in [1usize, 2, 7, 16, 31, 48, 63] {
            let d = downscale(&src, target);
            assert_eq!((d.width, d.height), (target, target));
            assert!(
                d.data.chunks_exact(4).all(|p| p == [173, 92, 210, 255]),
                "target {target} not uniform"
            );
        }
    }

    #[test]
    fn supersample_upscale_of_a_uniform_field_stays_uniform() {
        // draw_scaled takes the supersample lane when the destination is larger; a
        // constant source must produce a constant tile (bilinear of constants).
        let src = solid(3, 3, [40, 160, 90, 255]);
        let mut dst = Raster::new(24, 24);
        let b = ContentBounds { left: 0, top: 0, right: 3, bottom: 3 };
        draw_scaled(&src, b, &mut dst, 24, 0, 0, 24, 24);
        assert!(dst.data.chunks_exact(4).all(|p| p == [40, 160, 90, 255]));
    }

    #[test]
    fn downscale_of_a_fully_transparent_field_stays_transparent() {
        let d = downscale(&Raster::new(32, 32), 8); // all-zero source
        assert!(d.data.chunks_exact(4).all(|p| p == [0, 0, 0, 0]));
    }

    #[test]
    fn downscale_1x1_and_target_ge_source_clone() {
        let src = solid(1, 1, [5, 6, 7, 255]);
        assert_eq!(downscale(&src, 1).data, src.data); // 1 >= 1 → clone
        let bigger = solid(8, 8, [9, 9, 9, 255]);
        assert_eq!(downscale(&bigger, 16).data, bigger.data); // target > width → clone
    }

    #[test]
    fn bilinear_clamps_out_of_range_coords_to_the_edge() {
        let src = checker(4, 4); // (0,0) white, (3,0) ... alternating
        // Far out-of-range coords clamp into the source, never panic / index oob.
        let tl = sample_bilinear_at(&src, -100.0, -100.0);
        let br = sample_bilinear_at(&src, 1e6, 1e6);
        assert_eq!(tl, (255, 255, 255, 255)); // clamps to (0,0)
        assert_eq!(br.3, 255); // opaque corner, no panic
    }
}
