//! Background detection (analysis.ts): canvas-edge ring, shape ring, and the
//! corner-symmetry discriminator that rejects dog-eared document pages.

use super::{
    alpha_at, bounds_h, bounds_w, color_distance, find_content_bounds, pixel_at, ContentBounds,
};
use crate::raster::{Raster, Rgba};

/// The icon's own background colour, or None for a bare logo (analysis.ts
/// `tryDetectBackground`).
pub fn try_detect_background(c: &Raster) -> Option<Rgba> {
    try_canvas_background(c).or_else(|| try_shape_background(c))
}

fn try_canvas_background(c: &Raster) -> Option<Rgba> {
    let inner_inset = 4.max(c.width / 32);
    let outer = try_uniform_rect_ring(c, 0, 18)?;
    let inner = try_uniform_rect_ring(c, inner_inset, 18)?;
    if color_distance(outer, inner) > 18 {
        None
    } else {
        Some(outer)
    }
}

fn try_shape_background(c: &Raster) -> Option<Rgba> {
    let bounds = find_content_bounds(c);
    if opaque_coverage(c, bounds) < 0.62 {
        return None;
    }
    let min_dim = bounds_w(bounds).min(bounds_h(bounds));
    if min_dim < c.width / 3 {
        return None;
    }
    // Owner law ②: only corner-symmetric silhouettes own a board.
    if !corners_symmetric(c, bounds, min_dim) {
        return None;
    }

    // Only the outermost ring decides; probe several insets (a 1px highlight
    // border fattens under upscaling — first uniform depth wins).
    let offsets = [
        2.max(min_dim / 96),
        5.max(min_dim / 48),
        9.max(min_dim / 24),
    ];
    for offset in offsets {
        if offset * 2 + 2 >= min_dim {
            break;
        }
        if let Some(ring) = try_uniform_shape_ring(c, bounds, offset, 24) {
            return Some(ring);
        }
    }
    None
}

/// Diagonal corner insets to the first fully-solid pixel; a fold/notch on one
/// corner breaks the four-way symmetry (analysis.ts `cornersSymmetric`).
pub fn corners_symmetric(c: &Raster, b: ContentBounds, min_dim: usize) -> bool {
    let solid_at = |x: isize, y: isize| -> bool {
        if x < 0 || y < 0 || x >= c.width as isize || y >= c.height as isize {
            false
        } else {
            alpha_at(c, x as usize, y as usize) >= 245
        }
    };
    let walk = |sx: usize, sy: usize, dx: isize, dy: isize| -> i64 {
        let limit = min_dim.div_ceil(2) as i64;
        for k in 0..limit {
            if solid_at(sx as isize + dx * k as isize, sy as isize + dy * k as isize) {
                return k;
            }
        }
        limit
    };
    let insets = [
        walk(b.left, b.top, 1, 1),
        walk(b.right - 1, b.top, -1, 1),
        walk(b.left, b.bottom - 1, 1, -1),
        walk(b.right - 1, b.bottom - 1, -1, -1),
    ];
    let spread = insets.iter().max().unwrap() - insets.iter().min().unwrap();
    spread <= (2.max(min_dim / 24)) as i64
}

fn opaque_coverage(c: &Raster, b: ContentBounds) -> f64 {
    let total = (bounds_w(b) * bounds_h(b)).max(1);
    let mut opaque = 0usize;
    for y in b.top..b.bottom {
        for x in b.left..b.right {
            if c.data[(y * c.width + x) * 4 + 3] > 24 {
                opaque += 1;
            }
        }
    }
    opaque as f64 / total as f64
}

/// Averaging accumulator over a probed ring (analysis.ts `RingAccumulator`).
struct RingAccumulator {
    samples: Vec<Rgba>,
    total: usize,
    opaque: usize,
    sum_r: f64,
    sum_g: f64,
    sum_b: f64,
}

impl RingAccumulator {
    fn new() -> Self {
        RingAccumulator { samples: Vec::new(), total: 0, opaque: 0, sum_r: 0.0, sum_g: 0.0, sum_b: 0.0 }
    }

    fn add(&mut self, color: Rgba) {
        self.total += 1;
        if color.a < 245 {
            return;
        }
        self.opaque += 1;
        self.samples.push(color);
        self.sum_r += color.r as f64;
        self.sum_g += color.g as f64;
        self.sum_b += color.b as f64;
    }

    fn resolve(&self, tolerance: i32, opaque_fraction: f64, close_fraction: f64) -> Option<Rgba> {
        if self.total == 0 || (self.opaque as f64) < (self.total as f64 * opaque_fraction).floor() {
            return None;
        }
        let avg = Rgba {
            r: (self.sum_r / self.opaque as f64).floor() as u8,
            g: (self.sum_g / self.opaque as f64).floor() as u8,
            b: (self.sum_b / self.opaque as f64).floor() as u8,
            a: 255,
        };
        let mut close = 0usize;
        for s in &self.samples {
            if color_distance(*s, avg) <= tolerance {
                close += 1;
            }
        }
        if close as f64 >= (self.samples.len() as f64 * close_fraction).floor() {
            Some(avg)
        } else {
            None
        }
    }
}

fn try_uniform_rect_ring(c: &Raster, inset: usize, tolerance: i32) -> Option<Rgba> {
    let min = inset;
    let max = c.width - 1 - inset;
    if max <= min {
        return None;
    }
    let mut acc = RingAccumulator::new();
    for i in min..=max {
        acc.add(pixel_at(c, i, min));
        acc.add(pixel_at(c, i, max));
        acc.add(pixel_at(c, min, i));
        acc.add(pixel_at(c, max, i));
    }
    acc.resolve(tolerance, 0.9, 0.95)
}

fn try_uniform_shape_ring(c: &Raster, b: ContentBounds, offset: usize, tolerance: i32) -> Option<Rgba> {
    let mut acc = RingAccumulator::new();
    for y in b.top..b.bottom {
        if let Some((lo, hi)) = opaque_row_span(c, y, b.left, b.right - 1) {
            if hi - lo > offset * 2 {
                acc.add(pixel_at(c, lo + offset, y));
                acc.add(pixel_at(c, hi - offset, y));
            }
        }
    }
    for x in b.left..b.right {
        if let Some((lo, hi)) = opaque_column_span(c, x, b.top, b.bottom - 1) {
            if hi - lo > offset * 2 {
                acc.add(pixel_at(c, x, lo + offset));
                acc.add(pixel_at(c, x, hi - offset));
            }
        }
    }
    if acc.total < 32 {
        None
    } else {
        acc.resolve(tolerance, 0.92, 0.92)
    }
}

fn opaque_row_span(c: &Raster, y: usize, min_x: usize, max_x: usize) -> Option<(usize, usize)> {
    let mut left: isize = -1;
    let mut right: isize = -1;
    for x in min_x..=max_x {
        if alpha_at(c, x, y) >= 245 {
            left = x as isize;
            break;
        }
    }
    for x in (min_x..=max_x).rev() {
        if alpha_at(c, x, y) >= 245 {
            right = x as isize;
            break;
        }
    }
    if left >= 0 && right >= left {
        Some((left as usize, right as usize))
    } else {
        None
    }
}

fn opaque_column_span(c: &Raster, x: usize, min_y: usize, max_y: usize) -> Option<(usize, usize)> {
    let mut top: isize = -1;
    let mut bottom: isize = -1;
    for y in min_y..=max_y {
        if alpha_at(c, x, y) >= 245 {
            top = y as isize;
            break;
        }
    }
    for y in (min_y..=max_y).rev() {
        if alpha_at(c, x, y) >= 245 {
            bottom = y as isize;
            break;
        }
    }
    if top >= 0 && bottom >= top {
        Some((top as usize, bottom as usize))
    } else {
        None
    }
}
