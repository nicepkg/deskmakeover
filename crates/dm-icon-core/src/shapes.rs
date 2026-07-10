//! Icon-shape geometry — 1:1 port of the frozen `shapes.ts`.
//!
//! Spike 4 implements the slice's shapes only (`Circle`, `None`); the polygon
//! family (Figma corner smoothing, authored cubics, Apple squircle) lands at
//! M5 in port order. The enum carries the FULL catalog so downstream config
//! types are already final.

/// The owner-curated shape catalog (bridge `IconShape`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IconShape {
    Apple,
    Circle,
    Samsung,
    None,
    Bookmark,
    Lemon,
    Tile,
    Teardrop,
    Diamond,
    Flower,
    Pebble,
    Folder,
}

/// True when the point (in the size×size box) is inside the shape
/// (shapes.ts `shapeContains`). Circle: normalized squared distance ≤ 1 —
/// pure f64 arithmetic, no transcendentals.
///
/// # Panics
/// On `size <= 0` (mirrors the TS `RangeError`) and on shapes the port has
/// not reached yet (M5).
pub fn shape_contains(shape: IconShape, x: f64, y: f64, size: f64) -> bool {
    assert!(size > 0.0, "size must be positive");
    match shape {
        IconShape::Circle => {
            let h = size / 2.0;
            let dx = (x - h) / h;
            let dy = (y - h) / h;
            dx * dx + dy * dy <= 1.0
        }
        IconShape::None => x >= 0.0 && y >= 0.0 && x <= size && y <= size,
        other => todo!("shape_contains({other:?}): polygon family ports at M5"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circle_membership() {
        assert!(shape_contains(IconShape::Circle, 128.0, 128.0, 256.0));
        assert!(shape_contains(IconShape::Circle, 128.0, 0.0, 256.0)); // top tangent point
        assert!(!shape_contains(IconShape::Circle, 0.0, 0.0, 256.0)); // corner
        assert!(!shape_contains(IconShape::Circle, 255.9, 255.9, 256.0));
    }

    #[test]
    fn none_is_the_full_box() {
        assert!(shape_contains(IconShape::None, 0.0, 0.0, 256.0));
        assert!(shape_contains(IconShape::None, 256.0, 256.0, 256.0));
        assert!(!shape_contains(IconShape::None, -0.1, 0.0, 256.0));
    }
}
