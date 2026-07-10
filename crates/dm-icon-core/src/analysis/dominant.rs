//! Dominant colour + hue dispersion (analysis.ts `dominantColor`, ADR-0016
//! Field mode). The theme band is grown from the peak hue bucket by
//! neighbour-merging; the band's chroma-weighted RGB mean is summed in the SET
//! INSERTION ORDER of the TS oracle (peak, then −1 steps, then +1 steps) — f64
//! addition is not associative, so the order is load-bearing for byte parity.

use crate::color::to_ok_lab;
use crate::js_math::{clamp_u8_int, js_round};
use crate::raster::{Raster, Rgba};

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
        let (_l, a, b) = to_ok_lab(d[i4], d[i4 + 1], d[i4 + 2]);
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
