//! Background detection (analysis.ts): canvas-edge ring, shape ring, and the
//! corner-symmetry discriminator that rejects dog-eared document pages.

use super::{
    alpha_at, bounds_h, bounds_w, color_distance, find_content_bounds, pixel_at, ContentBounds,
};
use crate::raster::{Raster, Rgba};

/// The icon's own background colour, or None for a bare logo (analysis.ts
/// `tryDetectBackground`).
pub fn try_detect_background(c: &Raster) -> Option<Rgba> {
    try_detect_background_with_bounds(c, find_content_bounds(c))
}

/// `try_detect_background` given the source's already-computed content `bounds` — the
/// exact-input variant the shared analysis bundle feeds so `find_content_bounds` is
/// not recomputed inside the shape-ring probe. BYTE-IDENTICAL to `try_detect_background(c)`:
/// the ONLY difference is `bounds` is passed in instead of recomputed by
/// `try_shape_background`, and the caller guarantees `bounds == find_content_bounds(c)`.
/// The canvas-ring pass is unchanged, so control flow and every accumulation match.
pub fn try_detect_background_with_bounds(c: &Raster, bounds: ContentBounds) -> Option<Rgba> {
    try_canvas_background(c).or_else(|| try_shape_background_with_bounds(c, bounds))
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

fn try_shape_background_with_bounds(c: &Raster, bounds: ContentBounds) -> Option<Rgba> {
    if opaque_coverage(c, bounds) < 0.62 {
        return None;
    }
    let min_dim = bounds_w(bounds).min(bounds_h(bounds));
    // FLOAT division to match the frozen oracle (audit F7 / ADR-0019): integer `c.width / 3` moves
    // the boundary (e.g. 256/3 = 85 in Rust vs 85.333 in JS), so an 85px board on a 256px canvas
    // would diverge — Rust proceeding while the oracle returns none.
    if (min_dim as f64) < c.width as f64 / 3.0 {
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
    // A ring inset past EITHER axis has no pixels (and `dim - 1 - inset` would underflow on a tiny
    // fully-opaque source, e.g. a 3×3 board). The frozen TS used `width` for BOTH axes, so a
    // non-square raster read past the row and PANICS in `pixel_at` (which does no bounds check) — audit
    // F7 / codex R2 C-3: `RenderSession` guards this by dropping non-square, but `SourceFacts::compute`,
    // the free renderer, and `batch::IconJob` all analyze directly. This is the dimension-safe form,
    // BYTE-IDENTICAL for the square 256² masters the pipeline normalises to (same pixel multiset — only
    // the accumulation order changes, and `RingAccumulator::resolve` is order-independent).
    if inset + 1 >= c.width || inset + 1 >= c.height {
        return None;
    }
    let min = inset;
    let max_x = c.width - 1 - inset;
    let max_y = c.height - 1 - inset;
    if max_x <= min || max_y <= min {
        return None;
    }
    let mut acc = RingAccumulator::new();
    // Top + bottom edges walk x; left + right edges walk y (corners double-counted, exactly as before).
    for x in min..=max_x {
        acc.add(pixel_at(c, x, min));
        acc.add(pixel_at(c, x, max_y));
    }
    for y in min..=max_y {
        acc.add(pixel_at(c, min, y));
        acc.add(pixel_at(c, max_x, y));
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A solid-colour raster of arbitrary dimensions (fully opaque).
    fn solid(width: usize, height: usize, r: u8, g: u8, b: u8) -> Raster {
        let mut data = vec![0u8; width * height * 4];
        for px in data.chunks_exact_mut(4) {
            px.copy_from_slice(&[r, g, b, 255]);
        }
        Raster { width, height, data }
    }

    #[test]
    fn non_square_rasters_do_not_panic_in_the_rect_ring() {
        // codex R2 C-3: the rect ring used `width` for BOTH axes, so `pixel_at` (no bounds check)
        // indexed past the buffer on a non-square raster and PANICKED. RenderSession drops non-square,
        // but SourceFacts::compute / the free renderer / batch analyze directly. A wide AND a tall
        // raster must both analyze without panicking.
        for (w, h) in [(256usize, 128usize), (128, 256), (200, 50), (50, 200), (3, 3)] {
            let _ = try_detect_background(&solid(w, h, 40, 120, 200));
        }
    }

    #[test]
    fn a_uniform_square_still_detects_its_background() {
        // Square parity untouched: a solid 256² board is a background.
        assert_eq!(
            try_detect_background(&solid(256, 256, 40, 120, 200)),
            Some(Rgba { r: 40, g: 120, b: 200, a: 255 })
        );
    }
}
