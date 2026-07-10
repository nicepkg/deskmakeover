//! Icon-shape geometry — 1:1 port of the frozen `shapes.ts` (pixel geometry
//! only). The SVG path-string exports (`smoothShapePathD`, `curvedShapePathD`,
//! `applePathD`, `shapeOutline`) render panel chips, not pixels, and are not
//! exercised by the parity corpus — they are UI-layer and out of core scope
//! (docs/plans/2026-07-10-m5-icon-core.md §Scope). Only `shape_contains` +
//! the polygon builders it dispatches to are ported. All transcendentals route
//! through `libm`.

mod smooth;

use std::cell::RefCell;
use std::collections::HashMap;

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

pub(crate) type Pt = (f64, f64);

pub(crate) fn vsub(a: Pt, b: Pt) -> Pt {
    (a.0 - b.0, a.1 - b.1)
}
pub(crate) fn vadd(a: Pt, b: Pt) -> Pt {
    (a.0 + b.0, a.1 + b.1)
}
pub(crate) fn vscale(v: Pt, s: f64) -> Pt {
    (v.0 * s, v.1 * s)
}
pub(crate) fn vdot(a: Pt, b: Pt) -> f64 {
    a.0 * b.0 + a.1 * b.1
}
pub(crate) fn vnorm(v: Pt) -> Pt {
    let l = libm::hypot(v.0, v.1);
    if l < 1e-12 {
        (0.0, 0.0)
    } else {
        (v.0 / l, v.1 / l)
    }
}

/// Apple continuous-corner radius as a fraction of the tile size (iOS = 0.225).
pub const APPLE_CORNER_FACTOR: f64 = 0.225;

thread_local! {
    /// Per (shape, size-bits) flattened boundary polygon — the TS oracle memoizes
    /// the same way (`polyCache`); pure, so the cache is parity-neutral.
    static POLY_CACHE: RefCell<HashMap<(IconShape, u64), Vec<Pt>>> = RefCell::new(HashMap::new());
}

/// True when the point (in the size×size box) is inside the shape
/// (shapes.ts `shapeContains`).
///
/// # Panics
/// On `size <= 0` (mirrors the TS `RangeError`).
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
        _ => POLY_CACHE.with(|c| {
            let mut cache = c.borrow_mut();
            let poly = cache
                .entry((shape, size.to_bits()))
                .or_insert_with(|| build_polygon(shape, size));
            point_in_polygon(poly, x, y)
        }),
    }
}

fn build_polygon(shape: IconShape, size: f64) -> Vec<Pt> {
    match shape {
        IconShape::Apple => apple_polygon(size),
        IconShape::Tile
        | IconShape::Teardrop
        | IconShape::Bookmark
        | IconShape::Lemon
        | IconShape::Diamond
        | IconShape::Folder => smooth::smooth_polygon(shape, size),
        IconShape::Samsung | IconShape::Flower | IconShape::Pebble => {
            sample_path(&curved_def(shape), size)
        }
        IconShape::Circle | IconShape::None => {
            unreachable!("Circle/None are analytic in shape_contains")
        }
    }
}

fn point_in_polygon(poly: &[Pt], x: f64, y: f64) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if (yi > y) != (yj > y) && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

// ---- Authored cubic silhouettes (Samsung / Flower / Pebble) ------------------

enum Seg {
    #[allow(dead_code)] // no L/Q in the frozen data; kept for shape-def fidelity
    Line(Pt),
    #[allow(dead_code)]
    Quad(Pt, Pt),
    Cubic(Pt, Pt, Pt),
}

struct ShapeDef {
    start: Pt,
    segs: Vec<Seg>,
}

/// Steps per curved segment — chord error at 256 px is < 0.1 px.
const CURVE_STEPS: usize = 24;

fn curved_def(shape: IconShape) -> ShapeDef {
    match shape {
        IconShape::Samsung => ShapeDef {
            start: (50.0, 0.0),
            segs: vec![
                Seg::Cubic((10.0, 0.0), (0.0, 10.0), (0.0, 50.0)),
                Seg::Cubic((0.0, 90.0), (10.0, 100.0), (50.0, 100.0)),
                Seg::Cubic((90.0, 100.0), (100.0, 90.0), (100.0, 50.0)),
                Seg::Cubic((100.0, 10.0), (90.0, 0.0), (50.0, 0.0)),
            ],
        },
        IconShape::Flower => ShapeDef {
            start: (50.0, 0.0),
            segs: vec![
                Seg::Cubic((60.6, 0.0), (69.9, 5.3), (75.6, 13.5)),
                Seg::Cubic((78.56, 17.81), (82.29, 21.54), (86.6, 24.5)),
                Seg::Cubic((95.0, 30.27), (100.01, 39.81), (100.0, 50.0)),
                Seg::Cubic((100.0, 60.6), (94.7, 69.9), (86.5, 75.6)),
                Seg::Cubic((82.19, 78.56), (78.46, 82.29), (75.5, 86.6)),
                Seg::Cubic((69.73, 95.0), (60.19, 100.01), (50.0, 100.0)),
                Seg::Cubic((39.4, 100.0), (30.1, 94.7), (24.4, 86.5)),
                Seg::Cubic((21.44, 82.19), (17.71, 78.46), (13.4, 75.5)),
                Seg::Cubic((5.0, 69.73), (-0.01, 60.19), (0.0, 50.0)),
                Seg::Cubic((0.0, 39.4), (5.3, 30.1), (13.5, 24.4)),
                Seg::Cubic((17.81, 21.44), (21.54, 17.71), (24.5, 13.4)),
                Seg::Cubic((30.27, 5.0), (39.81, -0.01), (50.0, 0.0)),
            ],
        },
        IconShape::Pebble => ShapeDef {
            start: (55.0, 0.0),
            segs: vec![
                Seg::Cubic((25.0, 0.0), (0.0, 25.0), (0.0, 50.0)),
                Seg::Cubic((0.0, 78.0), (28.0, 100.0), (55.0, 100.0)),
                Seg::Cubic((85.0, 100.0), (100.0, 85.0), (100.0, 58.0)),
                Seg::Cubic((100.0, 30.0), (86.0, 0.0), (55.0, 0.0)),
            ],
        },
        _ => unreachable!("curved_def called on non-curved shape {shape:?}"),
    }
}

fn sample_cubic(pts: &mut Vec<Pt>, cur: Pt, c1: Pt, c2: Pt, to: Pt, s: f64) {
    for i in 1..=CURVE_STEPS {
        let t = i as f64 / CURVE_STEPS as f64;
        let u = 1.0 - t;
        let w0 = u * u * u;
        let w1 = 3.0 * u * u * t;
        let w2 = 3.0 * u * t * t;
        let w3 = t * t * t;
        pts.push((
            (w0 * cur.0 + w1 * c1.0 + w2 * c2.0 + w3 * to.0) * s,
            (w0 * cur.1 + w1 * c1.1 + w2 * c2.1 + w3 * to.1) * s,
        ));
    }
}

fn sample_path(def: &ShapeDef, size: f64) -> Vec<Pt> {
    let s = size / 100.0;
    let mut pts: Vec<Pt> = vec![(def.start.0 * s, def.start.1 * s)];
    let mut cur = def.start;
    for seg in &def.segs {
        match seg {
            Seg::Line(to) => {
                pts.push((to.0 * s, to.1 * s));
                cur = *to;
            }
            Seg::Quad(c, to) => {
                // Promote quadratic to cubic: c1 = p0 + ⅔(c−p0), c2 = p1 + ⅔(c−p1).
                let c1 = (cur.0 + (2.0 / 3.0) * (c.0 - cur.0), cur.1 + (2.0 / 3.0) * (c.1 - cur.1));
                let c2 = (to.0 + (2.0 / 3.0) * (c.0 - to.0), to.1 + (2.0 / 3.0) * (c.1 - to.1));
                sample_cubic(&mut pts, cur, c1, c2, *to, s);
                cur = *to;
            }
            Seg::Cubic(c1, c2, to) => {
                sample_cubic(&mut pts, cur, *c1, *c2, *to, s);
                cur = *to;
            }
        }
    }
    pts
}

// ---- Apple: the TRUE iOS continuous-corner squircle (three cubics/corner) ----

const APPLE_CORNER_STEPS: usize = 12;

#[allow(clippy::too_many_arguments)]
fn corner(
    pts: &mut Vec<Pt>,
    cur: Pt,
    a1: Pt,
    a2: Pt,
    a3: Pt,
    b1: Pt,
    b2: Pt,
    b3: Pt,
    c1: Pt,
    c2: Pt,
    c3: Pt,
) -> Pt {
    let cur = bezier(pts, cur, a1, a2, a3);
    let cur = bezier(pts, cur, b1, b2, b3);
    bezier(pts, cur, c1, c2, c3)
}

fn bezier(pts: &mut Vec<Pt>, p0: Pt, c1: Pt, c2: Pt, end: Pt) -> Pt {
    for i in 1..=APPLE_CORNER_STEPS {
        let t = i as f64 / APPLE_CORNER_STEPS as f64;
        let u = 1.0 - t;
        let w0 = u * u * u;
        let w1 = 3.0 * u * u * t;
        let w2 = 3.0 * u * t * t;
        let w3 = t * t * t;
        pts.push((
            w0 * p0.0 + w1 * c1.0 + w2 * c2.0 + w3 * end.0,
            w0 * p0.1 + w1 * c1.1 + w2 * c2.1 + w3 * end.1,
        ));
    }
    end
}

fn line(pts: &mut Vec<Pt>, end: Pt) -> Pt {
    pts.push(end);
    end
}

fn apple_polygon(size: f64) -> Vec<Pt> {
    let r = APPLE_CORNER_FACTOR * size;
    let tl = |x: f64, y: f64| -> Pt { (x * r, y * r) };
    let tr = |x: f64, y: f64| -> Pt { (size - x * r, y * r) };
    let br = |x: f64, y: f64| -> Pt { (size - x * r, size - y * r) };
    let bl = |x: f64, y: f64| -> Pt { (x * r, size - y * r) };

    // Each corner reads `cur` (the fixed line endpoint before it) and pushes its
    // bezier points; its return equals the next line's start, so — unlike the TS
    // `cur = corner(...)` — we let the following `line` re-seed `cur` directly.
    let mut pts: Vec<Pt> = vec![tl(1.528665, 0.0)];
    let mut cur = line(&mut pts, tr(1.528665, 0.0));
    corner(
        &mut pts, cur,
        tr(1.08849296, 0.0), tr(0.86840694, 0.0), tr(0.63149379, 0.07491139),
        tr(0.37282383, 0.16905956), tr(0.16905956, 0.37282383), tr(0.07491139, 0.63149379),
        tr(0.0, 0.86840694), tr(0.0, 1.08849296), tr(0.0, 1.52866498),
    );
    cur = line(&mut pts, br(0.0, 1.528665));
    corner(
        &mut pts, cur,
        br(0.0, 1.08849296), br(0.0, 0.86840694), br(0.07491139, 0.63149379),
        br(0.16905956, 0.37282383), br(0.37282383, 0.16905956), br(0.63149379, 0.07491139),
        br(0.86840694, 0.0), br(1.08849296, 0.0), br(1.52866498, 0.0),
    );
    cur = line(&mut pts, bl(1.528665, 0.0));
    corner(
        &mut pts, cur,
        bl(1.08849296, 0.0), bl(0.86840694, 0.0), bl(0.63149379, 0.07491139),
        bl(0.37282383, 0.16905956), bl(0.16905956, 0.37282383), bl(0.07491139, 0.63149379),
        bl(0.0, 0.86840694), bl(0.0, 1.08849296), bl(0.0, 1.52866498),
    );
    cur = line(&mut pts, tl(0.0, 1.528665));
    corner(
        &mut pts, cur,
        tl(0.0, 1.08849296), tl(0.0, 0.86840694), tl(0.07491139, 0.63149379),
        tl(0.16905956, 0.37282383), tl(0.37282383, 0.16905956), tl(0.63149379, 0.07491139),
        tl(0.86840694, 0.0), tl(1.08849296, 0.0), tl(1.52866498, 0.0),
    );
    pts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circle_membership() {
        assert!(shape_contains(IconShape::Circle, 128.0, 128.0, 256.0));
        assert!(shape_contains(IconShape::Circle, 128.0, 0.0, 256.0)); // top tangent
        assert!(!shape_contains(IconShape::Circle, 0.0, 0.0, 256.0)); // corner
        assert!(!shape_contains(IconShape::Circle, 255.9, 255.9, 256.0));
    }

    #[test]
    fn none_is_the_full_box() {
        assert!(shape_contains(IconShape::None, 0.0, 0.0, 256.0));
        assert!(shape_contains(IconShape::None, 256.0, 256.0, 256.0));
        assert!(!shape_contains(IconShape::None, -0.1, 0.0, 256.0));
    }

    #[test]
    fn polygon_shapes_contain_centre_and_reject_corner() {
        for shape in [
            IconShape::Apple,
            IconShape::Samsung,
            IconShape::Tile,
            IconShape::Teardrop,
            IconShape::Bookmark,
            IconShape::Lemon,
            IconShape::Diamond,
            IconShape::Flower,
            IconShape::Pebble,
            IconShape::Folder,
        ] {
            assert!(
                shape_contains(shape, 128.0, 128.0, 256.0),
                "{shape:?} must contain the centre"
            );
            // A rounded shape never covers the extreme (0,0)/(256,256) corners…
            // except Tile (10% corner radius still clips the exact corner point).
            assert!(
                !shape_contains(shape, 0.0, 0.0, 256.0),
                "{shape:?} must reject the top-left corner"
            );
        }
    }

    #[test]
    fn polygon_cache_is_deterministic() {
        let a = shape_contains(IconShape::Apple, 40.0, 40.0, 256.0);
        let b = shape_contains(IconShape::Apple, 40.0, 40.0, 256.0);
        assert_eq!(a, b);
    }

    #[test]
    fn diamond_axis_points_inside_corners_outside() {
        // A diamond covers the axis midpoints but not the box corners.
        assert!(shape_contains(IconShape::Diamond, 128.0, 8.0, 256.0));
        assert!(shape_contains(IconShape::Diamond, 8.0, 128.0, 256.0));
        assert!(!shape_contains(IconShape::Diamond, 20.0, 20.0, 256.0));
    }
}
