//! Artwork analysis — 1:1 port of the frozen `analysis.ts` (itself a port of the
//! C# IconBackgroundAnalyzer/IconSilhouetteClassifier). Understands what the real
//! icon already looks like so styling helps instead of hurts. Pure functions; the
//! TS side memoizes per raster (parity-neutral — a RenderSession cache belongs to
//! the caller). Split by concern to hold the 500-line cap.

mod background;
mod dominant;
mod shape_match;

pub use background::{corners_symmetric, try_detect_background, try_detect_background_with_bounds};
pub use dominant::{dominant_color, DominantColour};
pub use shape_match::{foreground_bounds, max_scale_inside, matches_shape};

use crate::color::perceived_lightness;
use crate::raster::{Raster, Rgba};
use crate::shapes::IconShape;

/// Mirror of the memoized `analysis.foregroundBounds(c)`: derive the plate +
/// own background exactly as the TS object does (tolerance 48). None when the
/// icon has no detectable background.
pub fn foreground_auto(c: &Raster) -> Option<ContentBounds> {
    foreground_from(c, find_content_bounds(c), try_detect_background(c))
}

/// `foreground_auto` given the source's already-computed content `bounds` and detected
/// `background` — the exact-input variant the shared analysis bundle feeds so neither
/// `find_content_bounds` nor `try_detect_background` is recomputed. BYTE-IDENTICAL to
/// `foreground_auto(c)` when `bounds == find_content_bounds(c)` and
/// `background == try_detect_background(c)`: the `?` short-circuits on a None background
/// exactly as `try_detect_background(c)?` does, then the same `foreground_bounds(_, _, _, 48)`.
pub fn foreground_from(
    c: &Raster,
    bounds: ContentBounds,
    background: Option<Rgba>,
) -> Option<ContentBounds> {
    let bg = background?;
    foreground_bounds(c, bounds, bg, 48)
}

/// Mirror of the memoized `analysis.maxScaleInside(c, shape)`.
pub fn max_scale_auto(c: &Raster, shape: IconShape) -> f64 {
    max_scale_inside(c, find_content_bounds(c), shape)
}

/// analysis.ts `ContentBounds` (left/top inclusive, right/bottom exclusive).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContentBounds {
    pub left: usize,
    pub top: usize,
    pub right: usize,
    pub bottom: usize,
}

pub fn bounds_w(b: ContentBounds) -> usize {
    b.right - b.left
}

pub fn bounds_h(b: ContentBounds) -> usize {
    b.bottom - b.top
}

pub(crate) const SOLID_ALPHA: u8 = 128;

pub(crate) fn alpha_at(c: &Raster, x: usize, y: usize) -> u8 {
    c.data[(y * c.width + x) * 4 + 3]
}

pub(crate) fn pixel_at(c: &Raster, x: usize, y: usize) -> Rgba {
    let i4 = (y * c.width + x) * 4;
    Rgba { r: c.data[i4], g: c.data[i4 + 1], b: c.data[i4 + 2], a: c.data[i4 + 3] }
}

/// Manhattan RGB distance (analysis.ts `colorDistance`).
pub fn color_distance(a: Rgba, b: Rgba) -> i32 {
    (a.r as i32 - b.r as i32).abs()
        + (a.g as i32 - b.g as i32).abs()
        + (a.b as i32 - b.b as i32).abs()
}

/// >10% of the 1px border see-through → the icon floats (analysis.ts
/// `hasTransparentEdges`). The top/bottom edges walk the width, the left/right
/// edges walk the height — the frozen TS assumed a square and read past the row
/// on non-square rasters; this is the corrected, dimension-safe form (byte-
/// identical for the square 256² masters the whole pipeline normalises to).
pub fn has_transparent_edges(c: &Raster) -> bool {
    if c.width == 0 || c.height == 0 {
        return false;
    }
    let last_x = c.width - 1;
    let last_y = c.height - 1;
    let mut transparent = 0i32;
    let mut total = 0i32;
    for x in 0..c.width {
        if alpha_at(c, x, 0) < 245 {
            transparent += 1;
        }
        if alpha_at(c, x, last_y) < 245 {
            transparent += 1;
        }
        total += 2;
    }
    for y in 0..c.height {
        if alpha_at(c, 0, y) < 245 {
            transparent += 1;
        }
        if alpha_at(c, last_x, y) < 245 {
            transparent += 1;
        }
        total += 2;
    }
    transparent > total / 10
}

/// Tight bounding box of pixels with alpha > 24; the whole canvas if fully empty
/// (analysis.ts `findContentBounds`).
pub fn find_content_bounds(c: &Raster) -> ContentBounds {
    let mut min_x = c.width;
    let mut min_y = c.height;
    let mut max_x: isize = -1;
    let mut max_y: isize = -1;
    for y in 0..c.height {
        for x in 0..c.width {
            if c.data[(y * c.width + x) * 4 + 3] > 24 {
                if x < min_x {
                    min_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if x as isize > max_x {
                    max_x = x as isize;
                }
                if y as isize > max_y {
                    max_y = y as isize;
                }
            }
        }
    }
    if max_x < min_x as isize || max_y < min_y as isize {
        ContentBounds { left: 0, top: 0, right: c.width, bottom: c.height }
    } else {
        ContentBounds {
            left: min_x,
            top: min_y,
            right: (max_x + 1) as usize,
            bottom: (max_y + 1) as usize,
        }
    }
}

/// Tight bbox of solid (A ≥ 128) pixels; None when the canvas has none
/// (analysis.ts `solidBounds`).
pub fn solid_bounds(c: &Raster) -> Option<ContentBounds> {
    let mut min_x = c.width;
    let mut min_y = c.height;
    let mut max_x: isize = -1;
    let mut max_y: isize = -1;
    for y in 0..c.height {
        for x in 0..c.width {
            if c.data[(y * c.width + x) * 4 + 3] >= SOLID_ALPHA {
                if x < min_x {
                    min_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if x as isize > max_x {
                    max_x = x as isize;
                }
                if y as isize > max_y {
                    max_y = y as isize;
                }
            }
        }
    }
    if max_x < min_x as isize {
        None
    } else {
        Some(ContentBounds {
            left: min_x,
            top: min_y,
            right: (max_x + 1) as usize,
            bottom: (max_y + 1) as usize,
        })
    }
}

/// Mean perceived lightness over solid (A≥128) pixels (analysis.ts
/// `visibleLightnessStats`; the `.mean` field only).
pub fn visible_lightness_mean(c: &Raster) -> f64 {
    let d = &c.data;
    let n = c.width * c.height;
    let mut sum = 0.0f64;
    let mut visible = 0u32;
    for i in 0..n {
        let i4 = i * 4;
        if d[i4 + 3] < SOLID_ALPHA {
            continue;
        }
        sum += perceived_lightness(d[i4], d[i4 + 1], d[i4 + 2]);
        visible += 1;
    }
    if visible == 0 {
        0.5
    } else {
        sum / visible as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dot(w: usize, h: usize, x: usize, y: usize, a: u8) -> Raster {
        let mut r = Raster::new(w, h);
        r.data[(y * w + x) * 4 + 3] = a;
        r
    }

    #[test]
    fn content_bounds_of_a_dot() {
        let b = find_content_bounds(&dot(8, 8, 4, 3, 255));
        assert_eq!(b, ContentBounds { left: 4, top: 3, right: 5, bottom: 4 });
    }

    #[test]
    fn alpha_24_is_invisible() {
        let b = find_content_bounds(&dot(4, 4, 0, 0, 24));
        assert_eq!(b, ContentBounds { left: 0, top: 0, right: 4, bottom: 4 });
    }

    #[test]
    fn solid_bounds_threshold() {
        assert_eq!(solid_bounds(&dot(4, 4, 1, 1, 127)), None);
        assert_eq!(
            solid_bounds(&dot(4, 4, 1, 1, 128)),
            Some(ContentBounds { left: 1, top: 1, right: 2, bottom: 2 })
        );
    }

    #[test]
    fn transparent_edges_detects_floating_art() {
        // Fully opaque border → not floating.
        let mut solidr = Raster::new(8, 8);
        for i in 0..solidr.data.len() {
            if i % 4 == 3 {
                solidr.data[i] = 255;
            }
        }
        assert!(!has_transparent_edges(&solidr));
        // Empty canvas → all edges transparent → floating.
        assert!(has_transparent_edges(&Raster::new(8, 8)));
    }

    #[test]
    fn color_distance_is_manhattan() {
        let a = Rgba { r: 10, g: 20, b: 30, a: 255 };
        let b = Rgba { r: 12, g: 17, b: 30, a: 0 };
        assert_eq!(color_distance(a, b), 2 + 3 + 0);
    }
}
