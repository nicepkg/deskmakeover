//! Subject/background segmentation for 极致单色 — 1:1 port of the frozen
//! `segment.ts`. Stage 1 takes the alpha silhouette (transparent-edge icons) or
//! a border-seeded local-tolerance flood (opaque icons); stage 2 (`plate.rs`)
//! splits a plate-like silhouette into field (→bg) and ink (subject) via Otsu.
//!
//! The flood's acceptance predicate is relative to the pixel it steps FROM, so
//! the BFS order (FIFO queue, neighbour order left/right/up/down) is load-bearing
//! and mirrored exactly.

mod plate;

use crate::analysis::has_transparent_edges;
use crate::raster::{Raster, Rgba};

const SOLID_ALPHA: u8 = 128;
const FLOOD_LOCAL_TOLERANCE: i64 = 14;
const FLOOD_SEED_TOLERANCE: i64 = 42;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegMode {
    Alpha,
    Flood,
    AlphaSplit,
    FloodSplit,
}

pub struct Segmentation {
    /// 1 = subject pixel, 0 = background (same indexing as the raster).
    pub mask: Vec<u8>,
    pub mode: SegMode,
    /// The detected plate/background colour — present ONLY after an alpha-mode
    /// plate split (the "fill with the icon's own plate colour" signal).
    pub field: Option<Rgba>,
}

/// segment.ts `segmentSubject` (memoization is a caller concern — pure here).
pub fn segment_subject(c: &Raster) -> Segmentation {
    segment_subject_with_edges(c, has_transparent_edges(c))
}

/// `segment_subject` given the source's already-computed transparent-edge flag — the
/// exact-input variant the shared analysis bundle feeds so `has_transparent_edges` is
/// not recomputed inside segmentation (the expensive BFS is the same either way).
/// BYTE-IDENTICAL to `segment_subject(c)` when `transparent_edges == has_transparent_edges(c)`:
/// the flag ONLY selects the alpha-silhouette vs flood branch; the 0-dim guard, the
/// majority filter, and the plate split are untouched. (On a 0-dim raster the guard
/// returns before the flag is used, so an eagerly-computed flag is harmless.)
pub fn segment_subject_with_edges(c: &Raster, transparent_edges: bool) -> Segmentation {
    let w = c.width;
    let h = c.height;
    let n = w * h;

    // A 0-dimension raster has no pixels to segment. Every downstream step (border
    // seeding at `(h-1)*w`, the flood BFS, `plate_split`, `binary_majority`) assumes
    // w>0 && h>0 and would underflow / index out of bounds. This is the single
    // lowest-common guard for RenderSession, batch, and SourceFacts::compute. (ICON-1)
    if w == 0 || h == 0 {
        return Segmentation { mask: Vec::new(), mode: SegMode::Alpha, field: None };
    }

    let (sil0, mode) = if transparent_edges {
        let mut s = vec![0u8; n];
        for (i, slot) in s.iter_mut().enumerate() {
            *slot = if c.data[i * 4 + 3] >= SOLID_ALPHA { 1 } else { 0 };
        }
        (s, SegMode::Alpha)
    } else {
        (flood_background(c), SegMode::Flood)
    };
    let sil = binary_majority(&sil0, w, h, 2);

    let solid: usize = sil.iter().map(|&v| v as usize).sum();
    if (solid as f64) < n as f64 * 0.02 {
        return Segmentation { mask: sil, mode, field: None };
    }

    if let Some(split) = plate::plate_split(c, &sil) {
        let mask = binary_majority(&split.ink, w, h, 1);
        let field = if mode == SegMode::Alpha { split.field } else { None };
        let mode = if mode == SegMode::Alpha { SegMode::AlphaSplit } else { SegMode::FloodSplit };
        return Segmentation { mask, mode, field };
    }
    Segmentation { mask: sil, mode, field: None }
}

/// Border-seeded BFS flood over the background; returns the SUBJECT mask
/// (segment.ts `floodBackground`).
fn flood_background(c: &Raster) -> Vec<u8> {
    let w = c.width;
    let h = c.height;
    let n = w * h;
    let data = &c.data;
    let mut bg = vec![0u8; n];
    let mut queue = vec![0i32; n];
    let mut head = 0usize;
    let mut tail = 0usize;

    let mut border: Vec<usize> = Vec::new();
    for x in 0..w {
        border.push(x);
        border.push((h - 1) * w + x);
    }
    for y in 0..h {
        border.push(y * w);
        border.push(y * w + w - 1);
    }

    let mut med = [0u8; 3];
    for (ch, m) in med.iter_mut().enumerate() {
        let mut vals: Vec<u8> = border.iter().map(|&i| data[i * 4 + ch]).collect();
        vals.sort_unstable();
        *m = vals[vals.len() >> 1];
    }

    let dist2 = |i: usize, r: u8, g: u8, b: u8| -> i64 {
        let dr = data[i * 4] as i64 - r as i64;
        let dg = data[i * 4 + 1] as i64 - g as i64;
        let db = data[i * 4 + 2] as i64 - b as i64;
        dr * dr + dg * dg + db * db
    };

    let seed_tol2 = FLOOD_SEED_TOLERANCE * FLOOD_SEED_TOLERANCE;
    for &i in &border {
        if bg[i] == 0 && dist2(i, med[0], med[1], med[2]) < seed_tol2 {
            bg[i] = 1;
            queue[tail] = i as i32;
            tail += 1;
        }
    }

    let local_tol2 = FLOOD_LOCAL_TOLERANCE * FLOOD_LOCAL_TOLERANCE;
    while head < tail {
        let i = queue[head] as usize;
        head += 1;
        let x = i % w;
        let r = data[i * 4];
        let g = data[i * 4 + 1];
        let b = data[i * 4 + 2];
        // Neighbour order (left, right, up, down) matches the TS tryStep chain.
        let mut neighbours: [Option<usize>; 4] = [None; 4];
        if x > 0 {
            neighbours[0] = Some(i - 1);
        }
        if x < w - 1 {
            neighbours[1] = Some(i + 1);
        }
        if i >= w {
            neighbours[2] = Some(i - w);
        }
        if i < (h - 1) * w {
            neighbours[3] = Some(i + w);
        }
        for nb in neighbours.into_iter().flatten() {
            if bg[nb] == 0 && dist2(nb, r, g, b) < local_tol2 {
                bg[nb] = 1;
                queue[tail] = nb as i32;
                tail += 1;
            }
        }
    }

    let mut subject = vec![0u8; n];
    for i in 0..n {
        subject[i] = if bg[i] != 0 { 0 } else { 1 };
    }
    subject
}

/// Binary majority filter over a (2r+1)² window (segment.ts `binaryMajority`).
pub(crate) fn binary_majority(mask: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
    let integral = build_integral(mask, w, h);
    let mut out = vec![0u8; mask.len()];
    for y in 0..h {
        let y0 = y.saturating_sub(r);
        let y1 = (y + r).min(h - 1);
        for x in 0..w {
            let x0 = x.saturating_sub(r);
            let x1 = (x + r).min(w - 1);
            let area = ((x1 - x0 + 1) * (y1 - y0 + 1)) as i64;
            let sum = box_sum(&integral, w, x0, y0, x1, y1);
            out[y * w + x] = if sum * 2 > area { 1 } else { 0 };
        }
    }
    out
}

/// Binary erosion with a (2r+1)² structuring element (segment.ts `binaryErode`).
pub(crate) fn binary_erode(mask: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
    let integral = build_integral(mask, w, h);
    let mut out = vec![0u8; mask.len()];
    for y in 0..h {
        let y0 = y.saturating_sub(r);
        let y1 = (y + r).min(h - 1);
        for x in 0..w {
            let x0 = x.saturating_sub(r);
            let x1 = (x + r).min(w - 1);
            let area = ((x1 - x0 + 1) * (y1 - y0 + 1)) as i64;
            out[y * w + x] = if box_sum(&integral, w, x0, y0, x1, y1) == area { 1 } else { 0 };
        }
    }
    out
}

fn build_integral(mask: &[u8], w: usize, h: usize) -> Vec<i64> {
    let mut integral = vec![0i64; (w + 1) * (h + 1)];
    for y in 0..h {
        let mut row_sum = 0i64;
        for x in 0..w {
            row_sum += mask[y * w + x] as i64;
            integral[(y + 1) * (w + 1) + x + 1] = integral[y * (w + 1) + x + 1] + row_sum;
        }
    }
    integral
}

fn box_sum(integral: &[i64], w: usize, x0: usize, y0: usize, x1: usize, y1: usize) -> i64 {
    let stride = w + 1;
    integral[(y1 + 1) * stride + x1 + 1] - integral[y0 * stride + x1 + 1]
        - integral[(y1 + 1) * stride + x0]
        + integral[y0 * stride + x0]
}

/// Share of ink owned by its largest 4-connected component (segment.ts
/// `largestComponentShare`).
pub(crate) fn largest_component_share(mask: &[u8], w: usize, h: usize, total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }
    let mut seen = vec![0u8; mask.len()];
    let mut queue = vec![0i32; total];
    let mut best = 0usize;
    for s in 0..mask.len() {
        if mask[s] == 0 || seen[s] != 0 {
            continue;
        }
        let mut head = 0usize;
        let mut tail = 0usize;
        queue[tail] = s as i32;
        tail += 1;
        seen[s] = 1;
        while head < tail {
            let i = queue[head] as usize;
            head += 1;
            let x = i % w;
            let push = |j: usize, seen: &mut [u8], queue: &mut [i32], tail: &mut usize| {
                if mask[j] != 0 && seen[j] == 0 {
                    seen[j] = 1;
                    queue[*tail] = j as i32;
                    *tail += 1;
                }
            };
            if x > 0 {
                push(i - 1, &mut seen, &mut queue, &mut tail);
            }
            if x < w - 1 {
                push(i + 1, &mut seen, &mut queue, &mut tail);
            }
            if i >= w {
                push(i - w, &mut seen, &mut queue, &mut tail);
            }
            if i < (h - 1) * w {
                push(i + w, &mut seen, &mut queue, &mut tail);
            }
        }
        if tail > best {
            best = tail;
        }
    }
    best as f64 / total as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_silhouette_of_floating_dot() {
        // A small solid dot on a transparent canvas → alpha mode, subject = dot.
        let mut r = Raster::new(16, 16);
        for y in 6..10 {
            for x in 6..10 {
                r.data[(y * 16 + x) * 4 + 3] = 255;
            }
        }
        let seg = segment_subject(&r);
        assert_eq!(seg.mode, SegMode::Alpha);
        assert_eq!(seg.mask[8 * 16 + 8], 1);
        assert_eq!(seg.mask[0], 0);
    }

    #[test]
    fn tolerates_zero_dimension_rasters() {
        // ICON-1: a 0-dim raster has no pixels; border seeding `(h-1)*w` / `y*w+w-1`
        // and the flood BFS would underflow / index OOB. Must return an empty mask,
        // never panic. Covers RenderSession / batch / SourceFacts / icon_profile.
        for (w, h) in [(0usize, 0usize), (0, 5), (5, 0)] {
            let seg = segment_subject(&Raster::new(w, h));
            assert!(seg.mask.is_empty(), "0-dim raster {w}x{h} → empty mask");
        }
    }

    #[test]
    fn majority_and_erode_on_constant() {
        let m = vec![1u8; 8 * 8];
        assert!(binary_majority(&m, 8, 8, 1).iter().all(|&v| v == 1));
        assert!(binary_erode(&m, 8, 8, 1).iter().all(|&v| v == 1));
        let z = vec![0u8; 8 * 8];
        assert!(binary_majority(&z, 8, 8, 1).iter().all(|&v| v == 0));
    }

    #[test]
    fn largest_component_of_two_blobs() {
        // 4×4: a 3-pixel L in the top-left, a 1-pixel dot bottom-right.
        let mut m = vec![0u8; 16];
        m[0] = 1;
        m[1] = 1;
        m[4] = 1;
        m[15] = 1;
        assert!((largest_component_share(&m, 4, 4, 4) - 3.0 / 4.0).abs() < 1e-12);
    }
}
