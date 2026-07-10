//! Artwork analysis — 1:1 port of the frozen `analysis.ts` (slice subset:
//! content bounds only). Background detection, silhouette classification and
//! the profile port land at M5. The TS side memoizes per raster; pure
//! functions, so caching is parity-neutral and stays with the caller.

use crate::raster::Raster;

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

/// Tight bounding box of pixels with alpha > 24; the whole canvas if fully
/// empty (analysis.ts `findContentBounds`).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_of_a_centre_dot() {
        let mut r = Raster::new(8, 8);
        r.data[(3 * 8 + 4) * 4 + 3] = 255;
        let b = find_content_bounds(&r);
        assert_eq!(b, ContentBounds { left: 4, top: 3, right: 5, bottom: 4 });
    }

    #[test]
    fn alpha_24_is_still_invisible() {
        let mut r = Raster::new(4, 4);
        r.data[3] = 24; // exactly at the threshold → excluded
        let b = find_content_bounds(&r);
        assert_eq!(b, ContentBounds { left: 0, top: 0, right: 4, bottom: 4 }); // empty → full canvas
    }
}
