//! Silhouette↔shape matching, foreground extraction, inscribe scale
//! (analysis.ts IconSilhouetteClassifier tail).

use super::{alpha_at, bounds_h, bounds_w, color_distance, pixel_at, solid_bounds, ContentBounds, SOLID_ALPHA};
use crate::raster::{Raster, Rgba};
use crate::shapes::{shape_contains, IconShape};

const MATCH_IOU: f64 = 0.985;

/// True when the icon's solid silhouette IS the target shape (IoU ≥ 0.985)
/// (analysis.ts `matchesShape`).
pub fn matches_shape(c: &Raster, shape: IconShape) -> bool {
    let b = match solid_bounds(c) {
        Some(b) => b,
        None => return false,
    };
    let third = c.width as f64 / 3.0;
    if (bounds_w(b) as f64) < third || (bounds_h(b) as f64) < third {
        return false;
    }
    let w = bounds_w(b);
    let h = bounds_h(b);
    if (w.min(h) as f64) / (w.max(h) as f64) < 0.95 {
        return false;
    }

    let s = w.max(h) as f64;
    let ox = b.left as f64 + (w as f64 - s) / 2.0;
    let oy = b.top as f64 + (h as f64 - s) / 2.0;
    let step = 1.max((s / 96.0).floor() as usize);
    let mut inter = 0i64;
    let mut union = 0i64;
    let mut y = b.top;
    while y < b.bottom {
        let mut x = b.left;
        while x < b.right {
            let solid = alpha_at(c, x, y) >= SOLID_ALPHA;
            let in_shape = shape_contains(shape, x as f64 + 0.5 - ox, y as f64 + 0.5 - oy, s);
            if solid && in_shape {
                inter += 1;
            }
            if solid || in_shape {
                union += 1;
            }
            x += step;
        }
        y += step;
    }
    union > 0 && (inter as f64 / union as f64) >= MATCH_IOU
}

/// The bounding box of the icon's own logo INSIDE its solid plate
/// (analysis.ts `foregroundBounds`; TS default tolerance 48).
pub fn foreground_bounds(
    c: &Raster,
    plate: ContentBounds,
    background: Rgba,
    tolerance: i32,
) -> Option<ContentBounds> {
    let mut min_x = plate.right as isize;
    let mut min_y = plate.bottom as isize;
    let mut max_x = plate.left as isize - 1;
    let mut max_y = plate.top as isize - 1;
    for y in plate.top..plate.bottom {
        for x in plate.left..plate.right {
            let p = pixel_at(c, x, y);
            if p.a > 24 && color_distance(p, background) > tolerance {
                if (x as isize) < min_x {
                    min_x = x as isize;
                }
                if (y as isize) < min_y {
                    min_y = y as isize;
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
    if max_x < min_x {
        return None;
    }
    let margin = 1.max(bounds_w(plate).min(bounds_h(plate)) / 48) as isize;
    Some(ContentBounds {
        left: (plate.left as isize).max(min_x - margin) as usize,
        top: (plate.top as isize).max(min_y - margin) as usize,
        right: (plate.right as isize).min(max_x + 1 + margin) as usize,
        bottom: (plate.bottom as isize).min(max_y + 1 + margin) as usize,
    })
}

/// Largest scale (fraction of the shape box) at which the solid silhouette fits
/// inside (analysis.ts `maxScaleInside`).
pub fn max_scale_inside(c: &Raster, b: ContentBounds, shape: IconShape) -> f64 {
    let mut boundary: Vec<(f64, f64)> = Vec::new();
    for y in b.top..b.bottom {
        for x in b.left..b.right {
            if alpha_at(c, x, y) < SOLID_ALPHA {
                continue;
            }
            let edge = x == 0
                || y == 0
                || x == c.width - 1
                || y == c.height - 1
                || alpha_at(c, x - 1, y) < SOLID_ALPHA
                || alpha_at(c, x + 1, y) < SOLID_ALPHA
                || alpha_at(c, x, y - 1) < SOLID_ALPHA
                || alpha_at(c, x, y + 1) < SOLID_ALPHA;
            if edge {
                boundary.push((x as f64 + 0.5, y as f64 + 0.5));
            }
        }
    }
    if boundary.is_empty() {
        return 1.0;
    }

    let cx = b.left as f64 + bounds_w(b) as f64 / 2.0;
    let cy = b.top as f64 + bounds_h(b) as f64 / 2.0;
    let half = bounds_w(b).max(bounds_h(b)) as f64 / 2.0;
    let fits_at = |scale: f64| -> bool {
        for &(x, y) in &boundary {
            let u = 1.0 + ((x - cx) / half) * scale;
            let v = 1.0 + ((y - cy) / half) * scale;
            if !shape_contains(shape, u, v, 2.0) {
                return false;
            }
        }
        true
    };

    let mut lo = 0.5;
    let mut hi = 1.0;
    for _ in 0..7 {
        let mid = (lo + hi) / 2.0;
        if fits_at(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}
