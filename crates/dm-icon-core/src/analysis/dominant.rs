//! Dominant colour + hue dispersion (analysis.ts `dominantColor`, ADR-0016
//! Field mode). The theme band is grown from the peak hue bucket by
//! neighbour-merging; the band's chroma-weighted RGB mean is summed in the SET
//! INSERTION ORDER of the TS oracle (peak, then −1 steps, then +1 steps) — f64
//! addition is not associative, so the order is load-bearing for byte parity.

use std::collections::HashMap;

use crate::color::to_ok_lab;
use crate::js_math::{clamp_u8_int, js_round};
use crate::raster::{Raster, Rgba};

/// A bit-exact per-call memo of [`to_ok_lab`] keyed by the 24-bit RGB triple. `to_ok_lab`
/// is a deterministic pure function of `(r, g, b)`, so a hit returns the SAME three f64
/// bits a fresh call would — the memo only replaces the recompute (3× `libm::cbrt`), never
/// the value. Dropped at `dominant_color` return; nothing crosses render/thread boundaries.
struct OkLabMemo(HashMap<u32, (f64, f64, f64)>);

impl OkLabMemo {
    fn new() -> Self {
        Self(HashMap::new())
    }

    #[inline]
    fn get(&mut self, r: u8, g: u8, b: u8) -> (f64, f64, f64) {
        let key = (r as u32) << 16 | (g as u32) << 8 | b as u32;
        *self.0.entry(key).or_insert_with(|| to_ok_lab(r, g, b))
    }
}

const DOMINANT_MIN_ALPHA: u8 = 128;
const DOMINANT_MIN_CHROMA: f64 = 0.03;
const THEME_MAJORITY: f64 = 0.5;
const NEIGHBOUR_RATIO: f64 = 0.1;
const NEIGHBOUR_MAX_SPAN: i64 = 6;
const HUE_BUCKETS: usize = 36;

pub struct DominantColour {
    pub colour: Rgba,
    pub dispersion: f64,
}

/// The artwork's dominant colour, chroma-weighted in OKLab hue space; None for
/// the no-hue tail (analysis.ts `dominantColor`). `mask` restricts voting to
/// subject pixels (None = whole canvas).
pub fn dominant_color(c: &Raster, mask: Option<&[u8]>) -> Option<DominantColour> {
    let d = &c.data;
    let n = c.width * c.height;
    let mut bucket_weight = [0.0f64; HUE_BUCKETS];
    let mut bucket_voters = [0u32; HUE_BUCKETS];
    let mut bucket_r = [0.0f64; HUE_BUCKETS];
    let mut bucket_g = [0.0f64; HUE_BUCKETS];
    let mut bucket_b = [0.0f64; HUE_BUCKETS];
    let mut sum_cos = 0.0f64;
    let mut sum_sin = 0.0f64;
    let mut total_weight = 0.0f64;
    let mut voters = 0u32;
    let mut visible = 0u32;
    let pi = std::f64::consts::PI;
    let mut lab_memo = OkLabMemo::new();
    for i in 0..n {
        let i4 = i * 4;
        if d[i4 + 3] < DOMINANT_MIN_ALPHA {
            continue;
        }
        if let Some(m) = mask {
            if m[i] == 0 {
                continue;
            }
        }
        visible += 1;
        let (_l, a, b) = lab_memo.get(d[i4], d[i4 + 1], d[i4 + 2]);
        let chroma = libm::sqrt(a * a + b * b);
        if chroma < DOMINANT_MIN_CHROMA {
            continue;
        }
        let theta = libm::atan2(b, a);
        let bucket = ((((theta + pi) / (2.0 * pi)) * HUE_BUCKETS as f64).floor() as i64)
            .rem_euclid(HUE_BUCKETS as i64) as usize;
        bucket_weight[bucket] += chroma;
        bucket_voters[bucket] += 1;
        bucket_r[bucket] += d[i4] as f64 * chroma;
        bucket_g[bucket] += d[i4 + 1] as f64 * chroma;
        bucket_b[bucket] += d[i4 + 2] as f64 * chroma;
        sum_cos += chroma * libm::cos(theta);
        sum_sin += chroma * libm::sin(theta);
        total_weight += chroma;
        voters += 1;
    }
    if visible == 0 || voters == 0 {
        return None;
    }

    let mut peak = 0usize;
    for b in 1..HUE_BUCKETS {
        if bucket_weight[b] > bucket_weight[peak] {
            peak = b;
        }
    }
    if bucket_weight[peak] <= 0.0 {
        return None;
    }

    // Grow the theme band (SET insertion order: peak, then dir=-1, then dir=+1).
    let mut in_band: Vec<usize> = vec![peak];
    let mut seen = [false; HUE_BUCKETS];
    seen[peak] = true;
    for dir in [-1i64, 1] {
        for step in 1..=NEIGHBOUR_MAX_SPAN {
            let b = (peak as i64 + dir * step).rem_euclid(HUE_BUCKETS as i64) as usize;
            if bucket_weight[b] < bucket_weight[peak] * NEIGHBOUR_RATIO {
                break;
            }
            if !seen[b] {
                seen[b] = true;
                in_band.push(b);
            }
        }
    }

    let mut w = 0.0f64;
    let mut band_voters = 0u32;
    let mut r = 0.0f64;
    let mut g = 0.0f64;
    let mut bl = 0.0f64;
    for &b in &in_band {
        w += bucket_weight[b];
        band_voters += bucket_voters[b];
        r += bucket_r[b];
        g += bucket_g[b];
        bl += bucket_b[b];
    }
    if w <= 0.0 || (band_voters as f64) < visible as f64 * THEME_MAJORITY {
        return None;
    }

    let dispersion = 1.0 - libm::sqrt(sum_cos * sum_cos + sum_sin * sum_sin) / total_weight;
    Some(DominantColour {
        colour: Rgba {
            r: clamp_u8_int(js_round(r / w)),
            g: clamp_u8_int(js_round(g / w)),
            b: clamp_u8_int(js_round(bl / w)),
            a: 255,
        },
        dispersion,
    })
}

#[cfg(test)]
mod ok_lab_memo_cert {
    use super::*;

    /// The `to_ok_lab` memo must be BYTE-EXACT: for every RGB key — first sight AND
    /// repeat — a memo `get` returns the identical three f64 bits `to_ok_lab` produces
    /// directly. This pins Win 1's invariant (memo replaces the recompute, never the
    /// value) independently of the corpus cert. Every channel compared via `to_bits`.
    #[test]
    fn memo_get_is_bit_identical_to_direct_to_ok_lab() {
        // Duplicates interleaved so the SAME key is served both as a miss and a hit.
        let samples: [(u8, u8, u8); 12] = [
            (0, 0, 0),
            (255, 255, 255),
            (200, 60, 40),
            (200, 60, 40), // repeat → memo hit
            (1, 2, 3),
            (128, 128, 128),
            (17, 233, 91),
            (17, 233, 91), // repeat → memo hit
            (255, 0, 0),
            (0, 255, 0),
            (0, 0, 255),
            (0, 0, 0), // repeat of the very first → memo hit
        ];
        let mut memo = OkLabMemo::new();
        for &(r, g, b) in &samples {
            let (ml, ma, mb) = memo.get(r, g, b);
            let (dl, da, db) = to_ok_lab(r, g, b);
            assert_eq!(ml.to_bits(), dl.to_bits(), "L bits diverge at {r},{g},{b}");
            assert_eq!(ma.to_bits(), da.to_bits(), "a bits diverge at {r},{g},{b}");
            assert_eq!(mb.to_bits(), db.to_bits(), "b bits diverge at {r},{g},{b}");
        }
    }

    /// A denser sweep across the channel corners + interior, each key visited twice,
    /// proving stability of the hit path over many entries.
    #[test]
    fn memo_hits_are_stable_across_a_channel_sweep() {
        let mut memo = OkLabMemo::new();
        for r in (0u16..=255).step_by(51) {
            for g in (0u16..=255).step_by(51) {
                for b in (0u16..=255).step_by(51) {
                    let (r, g, b) = (r as u8, g as u8, b as u8);
                    let first = memo.get(r, g, b);
                    let second = memo.get(r, g, b); // hit
                    let direct = to_ok_lab(r, g, b);
                    assert_eq!(first.0.to_bits(), direct.0.to_bits());
                    assert_eq!(first.1.to_bits(), direct.1.to_bits());
                    assert_eq!(first.2.to_bits(), direct.2.to_bits());
                    assert_eq!(second.0.to_bits(), direct.0.to_bits());
                    assert_eq!(second.1.to_bits(), direct.1.to_bits());
                    assert_eq!(second.2.to_bits(), direct.2.to_bits());
                }
            }
        }
    }
}
