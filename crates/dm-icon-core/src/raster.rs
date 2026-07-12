//! Pure raster primitives — 1:1 port of the frozen `raster.ts` (itself a port
//! of the C# `RasterOps.cs`). Straight-alpha RGBA, row-major; alpha/coverage
//! fields are `f64` (TS `Float64Array`), never `f32`.

use crate::js_math::{clamp01, clamp_byte, clamp_u8_int, js_round};
use crate::shapes::{shape_contains, IconShape};

/// A straight-alpha RGBA bitmap (row-major, 4 bytes per pixel).
#[derive(Clone, Debug)]
pub struct Raster {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

impl Raster {
    /// `makeRaster`.
    pub fn new(width: usize, height: usize) -> Self {
        Raster { width, height, data: vec![0; width * height * 4] }
    }
}

/// An opaque-or-translucent colour in 0-255 channels (straight alpha).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

pub const WHITE: Rgba = Rgba { r: 255, g: 255, b: 255, a: 255 };

/// raster.ts `fromRgbInt` — unpack `0xRRGGBB` to an opaque colour.
pub fn from_rgb_int(rgb: u32) -> Rgba {
    Rgba { r: ((rgb >> 16) & 0xff) as u8, g: ((rgb >> 8) & 0xff) as u8, b: (rgb & 0xff) as u8, a: 255 }
}

/// raster.ts `rgbaOf` — `0xRRGGBB` + [0,1] alpha → straight-alpha colour.
pub fn rgba_of(rgb: u32, alpha: f64) -> Rgba {
    Rgba {
        r: ((rgb >> 16) & 0xff) as u8,
        g: ((rgb >> 8) & 0xff) as u8,
        b: (rgb & 0xff) as u8,
        a: clamp_u8_int(js_round(clamp01(alpha) * 255.0)),
    }
}

/// raster.ts `hexToInt` — '#RRGGBB' → packed `0xRRGGBB`. The frozen inputs are
/// always clean 6-hex strings (`parseInt(...,16)` there); non-hex → 0.
pub fn hex_to_int(hex: &str) -> u32 {
    u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0) & 0x00ff_ffff
}

/// Straight-alpha Porter-Duff "over" written INTO the buffer at byte offset
/// `i4`. Result RGB normalised by the output alpha (raster.ts `overAt`).
/// Early-out ORDER is part of the contract: `a == 0` → no-op; `a == 255` or
/// empty destination → direct store; otherwise blend in f64 + `js_round`.
pub fn over_at(dst: &mut [u8], i4: usize, r: u8, g: u8, b: u8, a: u8) {
    if a == 0 {
        return;
    }
    let ba = dst[i4 + 3];
    if a == 255 || ba == 0 {
        dst[i4] = r;
        dst[i4 + 1] = g;
        dst[i4 + 2] = b;
        dst[i4 + 3] = a;
        return;
    }
    let ta = a as f64 / 255.0;
    let bf = (ba as f64 / 255.0) * (1.0 - ta);
    let out_a = ta + bf;
    let inv = 1.0 / out_a;
    dst[i4] = clamp_u8_int(js_round((r as f64 * ta + dst[i4] as f64 * bf) * inv));
    dst[i4 + 1] = clamp_u8_int(js_round((g as f64 * ta + dst[i4 + 1] as f64 * bf) * inv));
    dst[i4 + 2] = clamp_u8_int(js_round((b as f64 * ta + dst[i4 + 2] as f64 * bf) * inv));
    dst[i4 + 3] = clamp_u8_int(js_round(out_a * 255.0));
}

/// `RasterOps.Paint` — composite a translucent colour over pixel `i`, gated by
/// coverage (raster.ts `paint`).
pub fn paint(target: &mut Raster, i: usize, colour: Rgba, coverage: f64) {
    if coverage <= 0.0 || colour.a == 0 {
        return;
    }
    let cov = if coverage > 1.0 { 1.0 } else { coverage };
    over_at(
        &mut target.data,
        i * 4,
        colour.r,
        colour.g,
        colour.b,
        clamp_u8_int(js_round(colour.a as f64 * cov)),
    );
}

/// `RasterOps.Mix` — CSS color-mix(in srgb) incl. alpha (raster.ts `mix`).
pub fn mix(a: Rgba, b: Rgba, pct: f64) -> Rgba {
    let q = 1.0 - pct;
    Rgba {
        r: clamp_byte(a.r as f64 * pct + b.r as f64 * q),
        g: clamp_byte(a.g as f64 * pct + b.g as f64 * q),
        b: clamp_byte(a.b as f64 * pct + b.b as f64 * q),
        a: clamp_byte(a.a as f64 * pct + b.a as f64 * q),
    }
}

/// `RasterOps.Fade` — keep rgb, scale alpha (raster.ts `fade`).
pub fn fade(c: Rgba, pct: f64) -> Rgba {
    Rgba { r: c.r, g: c.g, b: c.b, a: clamp_byte(c.a as f64 * pct) }
}

// ---- shape masks (RasterOps.ShapeMask) --------------------------------------

const EDGE_SUB_SAMPLES: usize = 16;

/// Coverage mask of a shape over a square buffer (raster.ts `buildShapeMask`).
/// Boundary pixels get 16×16 supersampling; interior/exterior classify from a
/// shared corner grid, with a pixel-CENTRE confirmation when all four corners
/// agree (a thin curve can slice a pixel whose corners agree). The TS side
/// memoizes per key — pure function, so the cache is parity-neutral and the
/// Rust port leaves caching to the caller.
pub fn shape_mask(
    shape: IconShape,
    buffer_size: usize,
    shape_size: usize,
    offset_x: f64,
    offset_y: f64,
) -> Vec<f64> {
    assert!(shape_size > 0, "shapeSize must be positive");
    let size = shape_size as f64;
    let ox = offset_x;
    let oy = offset_y;
    let mut mask = vec![0.0f64; buffer_size * buffer_size];
    let grid_w = buffer_size + 1;
    let mut grid = vec![0u8; grid_w * grid_w];
    for y in 0..=buffer_size {
        for x in 0..=buffer_size {
            grid[y * grid_w + x] =
                shape_contains(shape, x as f64 - ox, y as f64 - oy, size) as u8;
        }
    }

    let step = 1.0 / EDGE_SUB_SAMPLES as f64;
    for y in 0..buffer_size {
        for x in 0..buffer_size {
            let tl = grid[y * grid_w + x];
            let tr = grid[y * grid_w + x + 1];
            let bl = grid[(y + 1) * grid_w + x];
            let br = grid[(y + 1) * grid_w + x + 1];

            if tl == tr && tl == bl && tl == br {
                let centre =
                    shape_contains(shape, x as f64 + 0.5 - ox, y as f64 + 0.5 - oy, size) as u8;
                if centre == tl {
                    mask[y * buffer_size + x] = if tl != 0 { 1.0 } else { 0.0 };
                    continue;
                }
            }

            let mut inside = 0usize;
            for sy in 0..EDGE_SUB_SAMPLES {
                for sx in 0..EDGE_SUB_SAMPLES {
                    if shape_contains(
                        shape,
                        x as f64 + (sx as f64 + 0.5) * step - ox,
                        y as f64 + (sy as f64 + 0.5) * step - oy,
                        size,
                    ) {
                        inside += 1;
                    }
                }
            }
            mask[y * buffer_size + x] =
                inside as f64 / (EDGE_SUB_SAMPLES * EDGE_SUB_SAMPLES) as f64;
        }
    }
    mask
}

/// `RasterOps.ClipToMask` — multiply every pixel's alpha by the mask coverage
/// (raster.ts `clipToMask`; compose.ts `applyCoverage` is the same body).
/// Zero-coverage pixels are FULLY zeroed (RGB too) — compositeOver's alpha
/// guard later relies on that.
pub fn clip_to_mask(pixels: &mut Raster, mask: &[f64]) {
    let d = &mut pixels.data;
    for (i, &cov) in mask.iter().enumerate() {
        if cov >= 1.0 {
            continue;
        }
        let i4 = i * 4;
        if cov <= 0.0 {
            d[i4] = 0;
            d[i4 + 1] = 0;
            d[i4 + 2] = 0;
            d[i4 + 3] = 0;
        } else {
            d[i4 + 3] = clamp_u8_int(js_round(d[i4 + 3] as f64 * cov));
        }
    }
}

/// `RasterOps.BoxBlur` — separable box blur of an f64 alpha field
/// (raster.ts `boxBlur`; the marks/backdrop path). NOTE: the silhouette-shadow
/// blur is the DIFFERENT f32 `slice::box_blur_in_place` — do not conflate.
/// `radius < 1` returns the input unchanged (TS returns the same reference).
pub fn box_blur(src: &[f64], size: usize, radius: i32) -> Vec<f64> {
    if radius < 1 {
        return src.to_vec();
    }
    let w = (2 * radius + 1) as f64;
    let r = radius as isize;
    let n = size as isize;
    let clamp_i = |v: isize| -> usize {
        if v < 0 {
            0
        } else if v >= n {
            (n - 1) as usize
        } else {
            v as usize
        }
    };
    let mut tmp = vec![0.0f64; size * size];
    for y in 0..size {
        let mut sum = 0.0f64;
        for x in -r..=r {
            sum += src[y * size + clamp_i(x)];
        }
        for x in 0..size {
            tmp[y * size + x] = sum / w;
            let xi = x as isize;
            sum += src[y * size + clamp_i(xi + r + 1)] - src[y * size + clamp_i(xi - r)];
        }
    }
    let mut o = vec![0.0f64; size * size];
    for x in 0..size {
        let mut sum = 0.0f64;
        for y in -r..=r {
            sum += tmp[clamp_i(y) * size + x];
        }
        for y in 0..size {
            o[y * size + x] = sum / w;
            let yi = y as isize;
            sum += tmp[clamp_i(yi + r + 1) * size + x] - tmp[clamp_i(yi - r) * size + x];
        }
    }
    o
}

/// `RasterOps.Shift` — offset an alpha field by (dx, dy) (raster.ts `shift`).
pub fn shift(src: &[f64], size: usize, dx: i32, dy: i32) -> Vec<f64> {
    let mut o = vec![0.0f64; size * size];
    let n = size as i32;
    for y in 0..size {
        let sy = y as i32 - dy;
        if sy < 0 || sy >= n {
            continue;
        }
        for x in 0..size {
            let sx = x as i32 - dx;
            if sx >= 0 && sx < n {
                o[y * size + x] = src[sy as usize * size + sx as usize];
            }
        }
    }
    o
}

/// `RasterOps.BackdropBlur` — blurred copy of the buffer's colour (raster.ts
/// `backdropBlur`; the frosted Glass-seat backdrop). Assumes a square buffer.
pub fn backdrop_blur(src: &Raster, radius: i32) -> Raster {
    if radius < 1 {
        return src.clone();
    }
    let size = src.width;
    let n = size * size;
    let mut chans: [Vec<f64>; 4] =
        [vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]];
    for i in 0..n {
        let i4 = i * 4;
        chans[0][i] = src.data[i4] as f64;
        chans[1][i] = src.data[i4 + 1] as f64;
        chans[2][i] = src.data[i4 + 2] as f64;
        chans[3][i] = src.data[i4 + 3] as f64;
    }
    let blurred: Vec<Vec<f64>> = chans.iter().map(|c| box_blur(c, size, radius)).collect();
    let mut o = Raster::new(size, size);
    for i in 0..n {
        let i4 = i * 4;
        o.data[i4] = clamp_byte(blurred[0][i]);
        o.data[i4 + 1] = clamp_byte(blurred[1][i]);
        o.data[i4 + 2] = clamp_byte(blurred[2][i]);
        o.data[i4 + 3] = clamp_byte(blurred[3][i]);
    }
    o
}

/// raster.ts `smoothStep01`.
pub fn smooth_step01(u: f64) -> f64 {
    let u = clamp01(u);
    u * u * (3.0 - 2.0 * u)
}

/// Distance from point (px,py) to segment a→b (raster.ts `distToSegment`).
#[allow(clippy::too_many_arguments)]
pub fn dist_to_segment(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let vx = bx - ax;
    let vy = by - ay;
    let wx = px - ax;
    let wy = py - ay;
    let c1 = vx * wx + vy * wy;
    let c2 = vx * vx + vy * vy;
    let t = if c2 <= 0.0 { 0.0 } else { (c1 / c2).clamp(0.0, 1.0) };
    let qx = px - (ax + t * vx);
    let qy = py - (ay + t * vy);
    libm::sqrt(qx * qx + qy * qy)
}

/// Point-in-triangle test via consistent edge signs (raster.ts `inTriangle`).
#[allow(clippy::too_many_arguments)]
pub fn in_triangle(
    px: f64,
    py: f64,
    ax: f64,
    ay: f64,
    bx: f64,
    by: f64,
    cx: f64,
    cy: f64,
) -> bool {
    let sign = |x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64| {
        (x1 - x3) * (y2 - y3) - (x2 - x3) * (y1 - y3)
    };
    let d1 = sign(px, py, ax, ay, bx, by);
    let d2 = sign(px, py, bx, by, cx, cy);
    let d3 = sign(px, py, cx, cy, ax, ay);
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn over_at_direct_store_paths() {
        let mut d = vec![0u8; 8];
        over_at(&mut d, 0, 10, 20, 30, 0); // a == 0 → no-op
        assert_eq!(&d[0..4], &[0, 0, 0, 0]);
        over_at(&mut d, 0, 10, 20, 30, 128); // empty dst → direct store
        assert_eq!(&d[0..4], &[10, 20, 30, 128]);
        over_at(&mut d, 0, 1, 2, 3, 255); // opaque → direct store
        assert_eq!(&d[0..4], &[1, 2, 3, 255]);
    }

    #[test]
    fn over_at_blend_matches_ts_formula() {
        // 50% white over opaque black: ta=bf=…, JS gives 128/128/128/255.
        let mut d = vec![0u8, 0, 0, 255];
        over_at(&mut d, 0, 255, 255, 255, 128);
        assert_eq!(d, vec![128, 128, 128, 255]);
        // Translucent over translucent (normalised-RGB branch).
        let mut d = vec![100u8, 100, 100, 100];
        over_at(&mut d, 0, 200, 0, 50, 60);
        // ta=60/255, bf=(100/255)*(1-ta), outA=ta+bf — replicate in f64:
        let ta = 60.0 / 255.0;
        let bf = (100.0 / 255.0) * (1.0 - ta);
        let inv = 1.0 / (ta + bf);
        let exp = |src: f64, dst: f64| js_round((src * ta + dst * bf) * inv) as u8;
        let exp_a = js_round((ta + bf) * 255.0) as u8;
        assert_eq!(d, vec![exp(200.0, 100.0), exp(0.0, 100.0), exp(50.0, 100.0), exp_a]);
    }

    #[test]
    fn paint_gates_and_scales() {
        let mut t = Raster::new(1, 1);
        paint(&mut t, 0, WHITE, 0.0); // zero coverage → no-op
        assert_eq!(t.data[3], 0);
        paint(&mut t, 0, WHITE, 2.0); // coverage clamps to 1
        assert_eq!(t.data, vec![255, 255, 255, 255]);
        let mut t = Raster::new(1, 1);
        paint(&mut t, 0, Rgba { r: 10, g: 10, b: 10, a: 200 }, 0.5);
        assert_eq!(t.data[3], 100); // js_round(200*0.5)
    }

    #[test]
    fn circle_mask_interior_exterior_and_conservation() {
        let mask = shape_mask(IconShape::Circle, 64, 64, 0.0, 0.0);
        assert_eq!(mask[32 * 64 + 32], 1.0); // centre
        assert_eq!(mask[0], 0.0); // corner
        // every value in [0,1]
        assert!(mask.iter().all(|&v| (0.0..=1.0).contains(&v)));
        // boundary pixels exist and are fractional
        assert!(mask.iter().any(|&v| v > 0.0 && v < 1.0));
    }

    #[test]
    fn clip_to_mask_zeroes_rgb_outside() {
        let mut r = Raster::new(2, 1);
        r.data.copy_from_slice(&[9, 9, 9, 200, 9, 9, 9, 200]);
        clip_to_mask(&mut r, &[0.0, 0.5]);
        assert_eq!(r.data, vec![0, 0, 0, 0, 9, 9, 9, 100]);
    }

    #[test]
    fn box_blur_preserves_constant_field_and_short_circuits() {
        let src = vec![0.25f64; 16 * 16];
        let out = box_blur(&src, 16, 3);
        assert!(out.iter().all(|&v| (v - 0.25).abs() < 1e-12));
        let same = box_blur(&src, 16, 0);
        assert_eq!(same, src);
    }

    #[test]
    fn from_and_hex_int_round_trip() {
        assert_eq!(from_rgb_int(0xff6f5e), Rgba { r: 255, g: 111, b: 94, a: 255 });
        assert_eq!(hex_to_int("#FF6F5E"), 0xff6f5e);
        assert_eq!(hex_to_int("FFFFFF"), 0xffffff);
        assert_eq!(rgba_of(0x18181c, 0.45), Rgba { r: 24, g: 24, b: 28, a: 115 });
    }

    #[test]
    fn shift_moves_field_and_zeroes_edges() {
        // 3×3 field with a single 1.0 at (1,1); shift by (+1,+1) → (2,2).
        let mut src = vec![0.0f64; 9];
        src[1 * 3 + 1] = 1.0;
        let o = shift(&src, 3, 1, 1);
        assert_eq!(o[2 * 3 + 2], 1.0);
        assert_eq!(o[1 * 3 + 1], 0.0);
        // Shifting off-canvas leaves zeros.
        assert!(shift(&src, 3, 5, 0).iter().all(|&v| v == 0.0));
    }

    #[test]
    fn backdrop_blur_short_circuit_and_uniform() {
        let mut r = Raster::new(4, 4);
        for i in 0..16 {
            r.data[i * 4] = 40;
            r.data[i * 4 + 1] = 80;
            r.data[i * 4 + 2] = 120;
            r.data[i * 4 + 3] = 200;
        }
        assert_eq!(backdrop_blur(&r, 0).data, r.data); // radius<1 clones
        let b = backdrop_blur(&r, 1);
        assert!(b.data.chunks(4).all(|p| p == [40, 80, 120, 200]));
    }

    #[test]
    fn smooth_step01_endpoints_and_midpoint() {
        assert_eq!(smooth_step01(-1.0), 0.0);
        assert_eq!(smooth_step01(2.0), 1.0);
        assert_eq!(smooth_step01(0.5), 0.5);
    }

    #[test]
    fn dist_to_segment_projects_and_clamps() {
        // Segment (0,0)→(10,0): a point above the middle projects to distance 3.
        assert!((dist_to_segment(5.0, 3.0, 0.0, 0.0, 10.0, 0.0) - 3.0).abs() < 1e-12);
        // Past the end → distance to the endpoint.
        assert!((dist_to_segment(13.0, 0.0, 0.0, 0.0, 10.0, 0.0) - 3.0).abs() < 1e-12);
        // Degenerate segment → distance to the point.
        assert!((dist_to_segment(3.0, 4.0, 1.0, 1.0, 1.0, 1.0) - libm::sqrt(4.0 + 9.0)).abs() < 1e-12);
    }

    #[test]
    fn in_triangle_inside_and_outside() {
        assert!(in_triangle(1.0, 1.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0));
        assert!(!in_triangle(3.0, 3.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0));
    }
}
