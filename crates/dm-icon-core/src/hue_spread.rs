//! Cross-icon hue spread — 1:1 port of the frozen `hue-spread.ts` (ADR-0016 D1).
//! Rotates COLLIDING derived-plate hues apart deterministically so a desktop's
//! same-hue pile (the blue pile) stays distinguishable. Pure; identical artwork
//! (same artKey) keeps identical plates.

use std::collections::BTreeMap;

use crate::color::{rotate_seed_hue, to_ok_lab};
use crate::raster::{from_rgb_int, hex_to_int, Rgba};

/// Target minimum hue gap between DISTINCT plates (≈12°).
fn min_gap() -> f64 {
    12.0 * std::f64::consts::PI / 180.0
}
/// Max rotation either way (≈18°) — brand hue stays itself.
fn rotation_cap() -> f64 {
    18.0 * std::f64::consts::PI / 180.0
}
const RELAX_ROUNDS: usize = 4;

pub struct SpreadEntry {
    pub id: String,
    /// Artwork identity (source URL); identical art → identical plate.
    pub art_key: String,
    /// The icon's derived seed colour (hex), or None for the no-hue tail.
    pub seed: Option<String>,
}

fn hue_of(seed: Rgba) -> f64 {
    let (_l, a, b) = to_ok_lab(seed.r, seed.g, seed.b);
    libm::atan2(b, a)
}

fn to_hex(c: Rgba) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
}

struct Rep {
    art_key: String,
    seed: Rgba,
    hue: f64,
    ids: Vec<String>,
}

/// hue-spread.ts `computeHueSpread`: id → adjusted seed hex. Ids with a null seed
/// are absent. Pure and deterministic.
pub fn compute_hue_spread(entries: &[SpreadEntry]) -> BTreeMap<String, String> {
    let tau = std::f64::consts::PI * 2.0;
    let min_gap = min_gap();
    let cap = rotation_cap();

    // One representative per artKey (first by sorted id); members follow it.
    let mut sorted: Vec<&SpreadEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    let mut reps: Vec<Rep> = Vec::new();
    let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for e in sorted {
        let seed_hex = match &e.seed {
            Some(s) => s,
            None => continue,
        };
        if let Some(&i) = index.get(&e.art_key) {
            reps[i].ids.push(e.id.clone());
        } else {
            let seed = from_rgb_int(hex_to_int(seed_hex));
            index.insert(e.art_key.clone(), reps.len());
            reps.push(Rep { art_key: e.art_key.clone(), seed, hue: hue_of(seed), ids: vec![e.id.clone()] });
        }
    }

    reps.sort_by(|a, b| {
        a.hue
            .partial_cmp(&b.hue)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.art_key.cmp(&b.art_key))
    });

    let mut result: BTreeMap<String, String> = BTreeMap::new();
    let n = reps.len();
    if n == 0 {
        return result;
    }

    let mut pos: Vec<f64> = reps.iter().map(|r| r.hue).collect();
    let lo: Vec<f64> = reps.iter().map(|r| r.hue - cap).collect();
    let hi: Vec<f64> = reps.iter().map(|r| r.hue + cap).collect();
    if n > 1 {
        for _ in 0..RELAX_ROUNDS {
            for i in 1..n {
                if pos[i] - pos[i - 1] < min_gap {
                    pos[i] = hi[i].min(pos[i - 1] + min_gap);
                }
            }
            for i in (0..=n - 2).rev() {
                if pos[i + 1] - pos[i] < min_gap {
                    pos[i] = lo[i].max(pos[i + 1] - min_gap);
                }
            }
            // Wrap seam: last→first neighbour pair straddling ±π.
            let seam = pos[0] + tau - pos[n - 1];
            if seam < min_gap {
                pos[0] = hi[0].min(pos[0] + (min_gap - seam) / 2.0);
                pos[n - 1] = lo[n - 1].max(pos[n - 1] - (min_gap - seam) / 2.0);
            }
        }
    }

    for (i, rep) in reps.iter().enumerate() {
        let offset = pos[i] - rep.hue;
        let hex = if offset.abs() < 1e-9 {
            to_hex(rep.seed)
        } else {
            to_hex(rotate_seed_hue(rep.seed, offset))
        };
        for id in &rep.ids {
            result.insert(id.clone(), hex.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(id: &str, art: &str, seed: Option<&str>) -> SpreadEntry {
        SpreadEntry { id: id.into(), art_key: art.into(), seed: seed.map(str::to_owned) }
    }

    #[test]
    fn null_seeds_are_absent() {
        let out = compute_hue_spread(&[e("a", "x", None), e("b", "y", Some("#3366CC"))]);
        assert!(!out.contains_key("a"));
        assert_eq!(out.get("b").map(String::as_str), Some("#3366CC"));
    }

    #[test]
    fn identical_artwork_keeps_identical_plate() {
        // Same artKey → same plate for both ids, no rotation.
        let out = compute_hue_spread(&[
            e("a", "shared", Some("#3366CC")),
            e("b", "shared", Some("#3366CC")),
        ]);
        assert_eq!(out.get("a"), out.get("b"));
        assert_eq!(out.get("a").map(String::as_str), Some("#3366CC"));
    }

    #[test]
    fn colliding_distinct_hues_are_pushed_apart() {
        // Two near-identical blues on DISTINCT artwork → rotated apart.
        let out = compute_hue_spread(&[
            e("a", "artA", Some("#3366CC")),
            e("b", "artB", Some("#3364CE")),
        ]);
        assert_ne!(out.get("a"), out.get("b"));
    }

    // ---- properties (id-sort then hue-sort make the result set-, not order-, defined) ----

    fn dup(list: &[SpreadEntry]) -> Vec<SpreadEntry> {
        list.iter().map(|s| e(&s.id, &s.art_key, s.seed.as_deref())).collect()
    }

    #[test]
    fn output_is_permutation_invariant() {
        let base = vec![
            e("a", "artA", Some("#3366CC")),
            e("b", "artB", Some("#3364CE")),
            e("c", "artC", Some("#CC3366")),
            e("d", "artA", Some("#3366CC")), // shares artA with a
            e("z", "artZ", None),
        ];
        let forward = compute_hue_spread(&base);
        let mut shuffled = dup(&base);
        shuffled.reverse();
        assert_eq!(compute_hue_spread(&shuffled), forward);
        shuffled.rotate_left(2);
        assert_eq!(compute_hue_spread(&shuffled), forward);
    }

    #[test]
    fn same_art_members_share_the_reps_rotated_plate() {
        // artA appears twice (a,d) among colliding blues that force a rotation; both
        // members must carry the identical (rotated) hex, and it must equal a's.
        let out = compute_hue_spread(&[
            e("a", "artA", Some("#3366CC")),
            e("d", "artA", Some("#3366CC")),
            e("b", "artB", Some("#3364CE")),
        ]);
        assert_eq!(out.get("a"), out.get("d"));
    }

    #[test]
    fn empty_and_single_are_trivial() {
        assert!(compute_hue_spread(&[]).is_empty());
        let one = compute_hue_spread(&[e("a", "x", Some("#3366CC"))]);
        // Single rep → no neighbour → no rotation.
        assert_eq!(one.get("a").map(String::as_str), Some("#3366CC"));
    }

    #[test]
    fn is_deterministic_across_calls() {
        let list = [
            e("a", "artA", Some("#3366CC")),
            e("b", "artB", Some("#3364CE")),
            e("c", "artC", Some("#33CC88")),
        ];
        assert_eq!(compute_hue_spread(&list), compute_hue_spread(&list));
    }
}
