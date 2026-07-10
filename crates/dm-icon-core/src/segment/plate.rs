//! Stage 2 — split a plate-like silhouette into field (→bg) and ink (subject)
//! via Otsu over colour distance, and the STRICT flat-plate detector
//! (segment.ts `plateSplit` + `detectFlatPlate`).

use super::{binary_erode, largest_component_share};
use crate::raster::{Raster, Rgba};

const PLATE_BBOX_FILL: f64 = 0.72;
const SPLIT_SEPARATION: f64 = 26.0;
const SPLIT_INK_MIN: f64 = 0.04;
const SPLIT_INK_MAX: f64 = 0.6;
const SPLIT_COHERENCE: f64 = 0.2;
const RIM_OWNED: f64 = 0.6;
const LINE_EDGE_DENSITY: f64 = 0.4;
const LINE_FRAC_MAX: f64 = 0.3;

const PLATE_ASPECT_MIN: f64 = 0.92;
const PLATE_ASPECT_MAX: f64 = 1.09;
const PLATE_SHAPE_IOU: f64 = 0.95;
const PLATE_RIM_TOLERANCE: i64 = 16;
const PLATE_RIM_UNIFORM: f64 = 0.9;
const PLATE_FLAT_TOLERANCE: i64 = 22;
const PLATE_FLAT_FRACTION: f64 = 0.85;

const MAXD: f64 = 442.0;
const BINS: usize = 64;

pub(crate) struct PlateSplit {
    pub ink: Vec<u8>,
    pub field: Option<Rgba>,
}

fn median(mut v: Vec<u8>) -> i64 {
    v.sort_unstable();
    v[v.len() >> 1] as i64
}

pub(crate) fn plate_split(c: &Raster, sil: &[u8]) -> Option<PlateSplit> {
    let w = c.width;
    let h = c.height;
    let n = w * h;
    let data = &c.data;

    let mut min_x = w;
    let mut max_x: isize = -1;
    let mut min_y = h;
    let mut max_y: isize = -1;
    let mut solid = 0usize;
    for i in 0..n {
        if sil[i] == 0 {
            continue;
        }
        solid += 1;
        let x = i % w;
        let y = i / w;
        if x < min_x {
            min_x = x;
        }
        if x as isize > max_x {
            max_x = x as isize;
        }
        if y < min_y {
            min_y = y;
        }
        if y as isize > max_y {
            max_y = y as isize;
        }
    }
    if max_x < 0 {
        return None;
    }
    let max_x = max_x as usize;
    let max_y = max_y as usize;
    let bw = max_x - min_x + 1;
    let bh = max_y - min_y + 1;
    let bbox_fill = solid as f64 / (bw * bh) as f64;
    if bbox_fill <= PLATE_BBOX_FILL {
        return None;
    }

    let eroded = binary_erode(sil, w, h, 2);

    // Field = the silhouette's MEDIAN colour (the majority/plate colour).
    let mut ch_r = Vec::with_capacity(solid);
    let mut ch_g = Vec::with_capacity(solid);
    let mut ch_b = Vec::with_capacity(solid);
    for i in 0..n {
        if sil[i] != 0 {
            ch_r.push(data[i * 4]);
            ch_g.push(data[i * 4 + 1]);
            ch_b.push(data[i * 4 + 2]);
        }
    }
    let fr = median(ch_r) as f64;
    let fg = median(ch_g) as f64;
    let fb = median(ch_b) as f64;

    // Otsu over colour distance from the field (64 bins across 0..442).
    let mut hist = [0.0f64; BINS];
    let mut dist = vec![0.0f64; n];
    for i in 0..n {
        if sil[i] == 0 {
            continue;
        }
        let dr = data[i * 4] as f64 - fr;
        let dg = data[i * 4 + 1] as f64 - fg;
        let db = data[i * 4 + 2] as f64 - fb;
        let dd = libm::sqrt(dr * dr + dg * dg + db * db);
        dist[i] = dd;
        let bin = ((dd / MAXD * BINS as f64) as i64).min(BINS as i64 - 1) as usize;
        hist[bin] += 1.0;
    }
    let mut sum_all = 0.0f64;
    for i in 0..BINS {
        sum_all += ((i as f64 + 0.5) / BINS as f64) * MAXD * hist[i];
    }
    let solid_f = solid as f64;
    let mut w0 = 0.0f64;
    let mut sum0 = 0.0f64;
    let mut best_t = 0.0f64;
    let mut best_v = -1.0f64;
    for i in 0..BINS {
        w0 += hist[i];
        if w0 == 0.0 || w0 == solid_f {
            continue;
        }
        sum0 += ((i as f64 + 0.5) / BINS as f64) * MAXD * hist[i];
        let m0 = sum0 / w0;
        let m1 = (sum_all - sum0) / (solid_f - w0);
        let v = (w0 / solid_f) * (1.0 - w0 / solid_f) * (m0 - m1) * (m0 - m1);
        if v > best_v {
            best_v = v;
            best_t = ((i as f64 + 0.5) / BINS as f64) * MAXD;
        }
    }
    if libm::sqrt(best_v.max(0.0)) <= SPLIT_SEPARATION {
        return None;
    }

    let mut ink = vec![0u8; n];
    let mut ink_count = 0usize;
    for i in 0..n {
        if sil[i] != 0 && dist[i] > best_t {
            ink[i] = 1;
            ink_count += 1;
        }
    }
    let frac = ink_count as f64 / solid_f;
    if frac <= SPLIT_INK_MIN || frac >= SPLIT_INK_MAX {
        return None;
    }

    // Polarity guard: ink owning the rim is an inversion — except line-art.
    let mut rim_total = 0usize;
    let mut rim_ink = 0usize;
    for i in 0..n {
        if sil[i] != 0 && eroded[i] == 0 {
            rim_total += 1;
            if ink[i] != 0 {
                rim_ink += 1;
            }
        }
    }
    if rim_total >= 20 && rim_ink as f64 / rim_total as f64 > RIM_OWNED {
        let ink_eroded = binary_erode(&ink, w, h, 1);
        let inner: usize = ink_eroded.iter().map(|&v| v as usize).sum();
        let edge_density = 1.0 - inner as f64 / ink_count as f64;
        if !(edge_density > LINE_EDGE_DENSITY && frac < LINE_FRAC_MAX) {
            return None;
        }
    }

    // Fragmentation guard: photo-like art shatters into speckle — reject.
    if largest_component_share(&ink, w, h, ink_count) < SPLIT_COHERENCE {
        return None;
    }

    let field = detect_flat_plate(c, sil, &ink, min_x, min_y, max_x, max_y, &eroded);
    Some(PlateSplit { ink, field })
}

/// STRICT plate detector: outermost-ring colour ONLY for an absolute
/// square/rounded-square/circle with an identical ring and a flat body.
#[allow(clippy::too_many_arguments)]
fn detect_flat_plate(
    c: &Raster,
    sil: &[u8],
    ink: &[u8],
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
    eroded: &[u8],
) -> Option<Rgba> {
    let w = c.width;
    let data = &c.data;
    let bw = max_x - min_x + 1;
    let bh = max_y - min_y + 1;

    // 1. Absolute shape: tight aspect AND high IoU vs the best-fit ideal.
    let aspect = bw as f64 / bh as f64;
    if aspect < PLATE_ASPECT_MIN || aspect > PLATE_ASPECT_MAX {
        return None;
    }
    let cx = min_x as f64 + (bw as f64 - 1.0) / 2.0;
    let cy = min_y as f64 + (bh as f64 - 1.0) / 2.0;
    let rx = bw as f64 / 2.0;
    let ry = bh as f64 / 2.0;
    let rr = 0.2 * bw.min(bh) as f64;
    let (mut inter_c, mut union_c, mut inter_r, mut union_r) = (0i64, 0i64, 0i64, 0i64);
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let s = sil[y * w + x];
            let ex = (x as f64 - cx) / rx;
            let ey = (y as f64 - cy) / ry;
            let in_circle = (ex * ex + ey * ey <= 1.0) as i64;
            let qx = 0.0f64
                .max((min_x as f64 + rr) - x as f64)
                .max(x as f64 - (max_x as f64 - rr));
            let qy = 0.0f64
                .max((min_y as f64 + rr) - y as f64)
                .max(y as f64 - (max_y as f64 - rr));
            let in_round = (qx * qx + qy * qy <= rr * rr) as i64;
            if s != 0 && in_circle != 0 {
                inter_c += 1;
            }
            if s != 0 || in_circle != 0 {
                union_c += 1;
            }
            if s != 0 && in_round != 0 {
                inter_r += 1;
            }
            if s != 0 || in_round != 0 {
                union_r += 1;
            }
        }
    }
    let mut solid = 0i64;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            solid += sil[y * w + x] as i64;
        }
    }
    let iou_square = solid as f64 / (bw * bh) as f64;
    let iou_circle = if union_c != 0 { inter_c as f64 / union_c as f64 } else { 0.0 };
    let iou_round = if union_r != 0 { inter_r as f64 / union_r as f64 } else { 0.0 };
    if iou_square.max(iou_circle).max(iou_round) < PLATE_SHAPE_IOU {
        return None;
    }

    // 2. Outer ring must be ONE identical colour; that colour is the fill.
    let n = w * c.height;
    let mut or_v = Vec::new();
    let mut og_v = Vec::new();
    let mut ob_v = Vec::new();
    for i in 0..n {
        if sil[i] != 0 && eroded[i] == 0 {
            or_v.push(data[i * 4]);
            og_v.push(data[i * 4 + 1]);
            ob_v.push(data[i * 4 + 2]);
        }
    }
    if or_v.len() < 20 {
        return None;
    }
    let er = median(or_v.clone());
    let eg = median(og_v.clone());
    let eb = median(ob_v.clone());
    let ring_tol2 = PLATE_RIM_TOLERANCE * PLATE_RIM_TOLERANCE;
    let mut ring_same = 0usize;
    for k in 0..or_v.len() {
        let dr = or_v[k] as i64 - er;
        let dg = og_v[k] as i64 - eg;
        let db = ob_v[k] as i64 - eb;
        if dr * dr + dg * dg + db * db <= ring_tol2 {
            ring_same += 1;
        }
    }
    if (ring_same as f64) / (or_v.len() as f64) < PLATE_RIM_UNIFORM {
        return None;
    }

    // 3. Flat body (no gradient).
    let flat_tol2 = PLATE_FLAT_TOLERANCE * PLATE_FLAT_TOLERANCE;
    let mut body = 0usize;
    let mut body_same = 0usize;
    for i in 0..n {
        if sil[i] == 0 || ink[i] != 0 {
            continue;
        }
        body += 1;
        let dr = data[i * 4] as i64 - er;
        let dg = data[i * 4 + 1] as i64 - eg;
        let db = data[i * 4 + 2] as i64 - eb;
        if dr * dr + dg * dg + db * db <= flat_tol2 {
            body_same += 1;
        }
    }
    if body < 20 || (body_same as f64) / (body as f64) < PLATE_FLAT_FRACTION {
        return None;
    }

    Some(Rgba { r: er as u8, g: eg as u8, b: eb as u8, a: 255 })
}
