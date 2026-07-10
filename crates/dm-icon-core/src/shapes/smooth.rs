//! Figma-style corner smoothing for the rounded-polygon shape family — 1:1 port
//! of the `shapes.ts` squircle-path-kit section (MIT). Per corner with opening
//! angle φ, radius R, smoothing ξ: `q = R/tan(φ/2)`, `p = (1+ξ)q`, a circular arc
//! keeps `(1−ξ)` of the turn and a tangent cubic ramps in on each side; shared
//! edge budgets split proportionally. All transcendentals route through `libm`.

use super::{vadd, vdot, vnorm, vscale, vsub, IconShape, Pt};

const PI: f64 = std::f64::consts::PI;

#[derive(Clone, Copy)]
struct SmoothVertex {
    x: f64,
    y: f64,
    /// Corner radius (0..100 units).
    r: f64,
    /// Figma cornerSmoothing ξ ∈ [0, 1].
    s: f64,
}

struct SmoothDef {
    vertices: Vec<SmoothVertex>,
    /// Rescale so the ROUNDED outline fills 0..100 (Diamond tips pull inward).
    fit: bool,
}

#[derive(Clone, Copy)]
struct Cubic {
    cp1: Pt,
    cp2: Pt,
    end: Pt,
}

enum Corner {
    Sharp { point: Pt },
    // `end_point` (shapes.ts) drives only the SVG path-string exports, which are
    // UI-layer and out of core scope — the mask path reads start_point + segments.
    Smooth { start_point: Pt, segments: Vec<Cubic> },
}

/// Circular arc → cubic segments (κ construction, sign-carrying sweep).
fn arc_cubics(center: Pt, radius: f64, start_angle: f64, sweep: f64) -> Vec<Cubic> {
    if sweep.abs() < 1e-10 {
        return Vec::new();
    }
    let max_sweep = PI / 2.0;
    if sweep.abs() > max_sweep + 1e-6 {
        let n = (sweep.abs() / max_sweep).ceil() as usize;
        let seg = sweep / n as f64;
        let mut out = Vec::new();
        for i in 0..n {
            out.extend(arc_cubics(center, radius, start_angle + i as f64 * seg, seg));
        }
        return out;
    }
    let kappa = (4.0 / 3.0) * libm::tan(sweep / 4.0);
    let cos_s = libm::cos(start_angle);
    let sin_s = libm::sin(start_angle);
    let cos_e = libm::cos(start_angle + sweep);
    let sin_e = libm::sin(start_angle + sweep);
    let p0 = vadd(center, (cos_s * radius, sin_s * radius));
    let p3 = vadd(center, (cos_e * radius, sin_e * radius));
    vec![Cubic {
        cp1: vadd(p0, vscale((-sin_s, cos_s), kappa * radius)),
        cp2: vsub(p3, vscale((-sin_e, cos_e), kappa * radius)),
        end: p3,
    }]
}

#[allow(clippy::too_many_arguments)]
fn compute_corner(prev: Pt, curr: Pt, next: Pt, radius: f64, smoothness: f64, budget: f64) -> Corner {
    let dir_in = vnorm(vsub(prev, curr));
    let dir_out = vnorm(vsub(next, curr));
    let d = vdot(dir_in, dir_out).clamp(-1.0, 1.0);
    let phi = libm::acos(d);
    let half_phi = phi / 2.0;
    if half_phi < 1e-6 || (phi - PI).abs() < 1e-6 || radius < 1e-6 {
        return Corner::Sharp { point: curr };
    }

    let sin_half = libm::sin(half_phi);
    let tan_half = libm::tan(half_phi);
    let mut q = radius / tan_half;
    let mut xi = smoothness.clamp(0.0, 1.0);
    if q > budget {
        q = budget;
        xi = 0.0;
    } else if (1.0 + xi) * q > budget {
        xi = budget / q - 1.0;
    }
    let p = (1.0 + xi) * q;
    let eff_r = q * tan_half;
    if eff_r < 1e-6 || q < 1e-6 {
        return Corner::Sharp { point: curr };
    }

    let bisector = vnorm(vadd(dir_in, dir_out));
    let center = vadd(curr, vscale(bisector, eff_r / sin_half));
    let tangent_in = vadd(curr, vscale(dir_in, q));
    let tangent_out = vadd(curr, vscale(dir_out, q));

    let radial_in = vnorm(vsub(tangent_in, center));
    let is_ccw = vdot((-radial_in.1, radial_in.0), vscale(dir_in, -1.0)) > 0.0;
    let start_angle = libm::atan2(radial_in.1, radial_in.0);
    let radial_out = vnorm(vsub(tangent_out, center));
    let mut sweep = libm::atan2(radial_out.1, radial_out.0) - start_angle;
    if is_ccw {
        while sweep < 0.0 {
            sweep += 2.0 * PI;
        }
        if sweep > 2.0 * PI - 1e-6 {
            sweep -= 2.0 * PI;
        }
    } else {
        while sweep > 0.0 {
            sweep -= 2.0 * PI;
        }
        if sweep < -2.0 * PI + 1e-6 {
            sweep += 2.0 * PI;
        }
    }

    if xi < 1e-6 {
        return Corner::Smooth {
            start_point: tangent_in,
            segments: arc_cubics(center, eff_r, start_angle, sweep),
        };
    }

    let turn = PI - phi;
    let beta = (turn / 2.0) * xi;
    let t = eff_r * libm::tan(beta / 2.0);
    let b = (p - (q - t)) / 3.0;
    let a = 2.0 * b;

    let reduced_sweep = sweep * (1.0 - xi);
    let r_start = start_angle + sweep / 2.0 - reduced_sweep / 2.0;
    let arc_segments = if reduced_sweep.abs() > 1e-6 {
        arc_cubics(center, eff_r, r_start, reduced_sweep)
    } else {
        Vec::new()
    };
    let arc_start_pt = vadd(center, (libm::cos(r_start) * eff_r, libm::sin(r_start) * eff_r));

    let in_bezier = Cubic {
        cp1: vadd(curr, vscale(dir_in, p - a)),
        cp2: vadd(curr, vscale(dir_in, q - t)),
        end: arc_start_pt,
    };
    let out_bezier = Cubic {
        cp1: vadd(curr, vscale(dir_out, q - t)),
        cp2: vadd(curr, vscale(dir_out, p - a)),
        end: vadd(curr, vscale(dir_out, p)),
    };
    let mut segments = vec![in_bezier];
    segments.extend(arc_segments);
    segments.push(out_bezier);
    Corner::Smooth { start_point: vadd(curr, vscale(dir_in, p)), segments }
}

/// Order-independent proportional per-edge budget split (squircle-path-kit).
fn resolve_budgets(demands: &[(f64, f64)], edge_lengths: &[f64]) -> Vec<f64> {
    let n = demands.len();
    let max_p: Vec<f64> = demands.iter().map(|d| d.1).collect();
    let mut allow_next = vec![0.0f64; n];
    let mut allow_prev = vec![0.0f64; n];
    for i in 0..n {
        let j = (i + 1) % n;
        let total = max_p[i] + max_p[j];
        if total > edge_lengths[i] && total > 1e-6 {
            allow_next[i] = edge_lengths[i] * (max_p[i] / total);
            allow_prev[j] = edge_lengths[i] * (max_p[j] / total);
        } else {
            allow_next[i] = max_p[i];
            allow_prev[j] = max_p[j];
        }
    }
    (0..n).map(|i| max_p[i].min(allow_next[i]).min(allow_prev[i])).collect()
}

fn corners_of(vertices: &[SmoothVertex]) -> Vec<Corner> {
    let n = vertices.len();
    let mut edge_lengths = Vec::with_capacity(n);
    for i in 0..n {
        let j = (i + 1) % n;
        edge_lengths.push(libm::hypot(vertices[j].x - vertices[i].x, vertices[j].y - vertices[i].y));
    }
    let demands: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            let prev = &vertices[(i + n - 1) % n];
            let next = &vertices[(i + 1) % n];
            let v = &vertices[i];
            let dir_in = vnorm((prev.x - v.x, prev.y - v.y));
            let dir_out = vnorm((next.x - v.x, next.y - v.y));
            let d = vdot(dir_in, dir_out).clamp(-1.0, 1.0);
            let tan_half = libm::tan(libm::acos(d) / 2.0);
            let q = if tan_half > 1e-9 { v.r / tan_half } else { 0.0 };
            (q, (1.0 + v.s.clamp(0.0, 1.0)) * q)
        })
        .collect();
    let budgets = resolve_budgets(&demands, &edge_lengths);
    (0..n)
        .map(|i| {
            let prev = &vertices[(i + n - 1) % n];
            let next = &vertices[(i + 1) % n];
            let v = &vertices[i];
            compute_corner((prev.x, prev.y), (v.x, v.y), (next.x, next.y), v.r, v.s, budgets[i])
        })
        .collect()
}

fn smooth_corners(def: &SmoothDef) -> Vec<Corner> {
    let mut vertices: Vec<SmoothVertex> = def.vertices.clone();
    let mut corners = corners_of(&vertices);
    if def.fit {
        // Two rescale passes converge under 0.1 (Diamond tips).
        for _ in 0..2 {
            let pts = flatten_corners(&corners, 100.0);
            let min_x = pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
            let max_x = pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
            let min_y = pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
            let max_y = pts.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
            vertices = vertices
                .iter()
                .map(|v| SmoothVertex {
                    x: (v.x - min_x) * 100.0 / (max_x - min_x),
                    y: (v.y - min_y) * 100.0 / (max_y - min_y),
                    r: v.r,
                    s: v.s,
                })
                .collect();
            corners = corners_of(&vertices);
        }
    }
    corners
}

/// Steps per cubic when flattening for the mask — chord error < 0.1 px @256.
const FLATTEN_STEPS: usize = 12;

// `cur` mirrors the TS `let cur: Pt | null = null` seed and the sharp-corner
// write that the following corner always overwrites before reading (dead in the
// oracle too) — kept for a 1:1 structure.
#[allow(unused_assignments)]
fn flatten_corners(corners: &[Corner], size: f64) -> Vec<Pt> {
    let s = size / 100.0;
    let mut pts: Vec<Pt> = Vec::new();
    let mut cur: Pt = (0.0, 0.0);
    for corner in corners {
        match corner {
            Corner::Sharp { point } => {
                pts.push((point.0 * s, point.1 * s));
                cur = *point;
            }
            Corner::Smooth { start_point, segments } => {
                pts.push((start_point.0 * s, start_point.1 * s));
                cur = *start_point;
                for c in segments {
                    for i in 1..=FLATTEN_STEPS {
                        let t = i as f64 / FLATTEN_STEPS as f64;
                        let u = 1.0 - t;
                        let w0 = u * u * u;
                        let w1 = 3.0 * u * u * t;
                        let w2 = 3.0 * u * t * t;
                        let w3 = t * t * t;
                        pts.push((
                            (w0 * cur.0 + w1 * c.cp1.0 + w2 * c.cp2.0 + w3 * c.end.0) * s,
                            (w0 * cur.1 + w1 * c.cp1.1 + w2 * c.cp2.1 + w3 * c.end.1) * s,
                        ));
                    }
                    cur = c.end;
                }
            }
        }
    }
    pts
}

/// Rect-family smoothing feel (≈ iOS); auto-clamped where a radius eats the edge.
const XI: f64 = 0.6;

fn smooth_rect(tl: f64, tr: f64, br: f64, bl: f64) -> SmoothDef {
    SmoothDef {
        vertices: vec![
            SmoothVertex { x: 0.0, y: 0.0, r: tl, s: XI },
            SmoothVertex { x: 100.0, y: 0.0, r: tr, s: XI },
            SmoothVertex { x: 100.0, y: 100.0, r: br, s: XI },
            SmoothVertex { x: 0.0, y: 100.0, r: bl, s: XI },
        ],
        fit: false,
    }
}

fn smooth_def(shape: IconShape) -> SmoothDef {
    match shape {
        IconShape::Tile => smooth_rect(10.0, 10.0, 10.0, 10.0),
        IconShape::Teardrop => smooth_rect(50.0, 50.0, 20.0, 50.0),
        IconShape::Bookmark => smooth_rect(20.0, 20.0, 50.0, 50.0),
        IconShape::Lemon => smooth_rect(10.0, 50.0, 10.0, 50.0),
        IconShape::Folder => SmoothDef {
            vertices: vec![
                SmoothVertex { x: 0.0, y: 6.0, r: 8.0, s: XI },
                SmoothVertex { x: 36.0, y: 6.0, r: 6.0, s: XI },
                SmoothVertex { x: 46.0, y: 16.0, r: 4.0, s: XI },
                SmoothVertex { x: 100.0, y: 16.0, r: 10.0, s: XI },
                SmoothVertex { x: 100.0, y: 100.0, r: 12.0, s: XI },
                SmoothVertex { x: 0.0, y: 100.0, r: 12.0, s: XI },
            ],
            fit: false,
        },
        IconShape::Diamond => SmoothDef {
            vertices: vec![
                SmoothVertex { x: 50.0, y: 0.0, r: 20.0, s: 0.8 },
                SmoothVertex { x: 100.0, y: 50.0, r: 20.0, s: 0.8 },
                SmoothVertex { x: 50.0, y: 100.0, r: 20.0, s: 0.8 },
                SmoothVertex { x: 0.0, y: 50.0, r: 20.0, s: 0.8 },
            ],
            fit: true,
        },
        _ => unreachable!("smooth_def called on non-smooth shape {shape:?}"),
    }
}

/// The flattened boundary polygon for a rounded-polygon shape at a size.
pub(crate) fn smooth_polygon(shape: IconShape, size: f64) -> Vec<Pt> {
    flatten_corners(&smooth_corners(&smooth_def(shape)), size)
}
