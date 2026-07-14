//! Guarded sRGB-encode byte LUT (M6 kernel-speed Phase 5, `fast` build only).
//!
//! `color::srgb_encode` is the single biggest per-pixel transcendental left on the
//! compose path now that analysis/masks/colour are cached — a `libm::pow` run 3×
//! per output pixel in the sampling downscale/supersample loops. It returns the
//! terminal output BYTE directly (`-> u8`), so its result never feeds further float
//! math, and it is MONOTONIC non-decreasing in `linear`. Those two facts make a byte
//! lookup provably byte-safe.
//!
//! BYTE-SAFETY ARGUMENT (why every cached cell is provably its byte):
//! - The runtime maps `v ∈ [0,1]` to an index by `(v * N) as usize`, clamped to
//!   `N-1`. `N` is a power of two, so `v * N` is EXACT in f64 (scaling by 2^k only
//!   shifts the exponent — no rounding), and `as usize` is therefore an exact floor.
//!   Index `i` (for `i < N-1`) is hit by exactly `v ∈ [i/N, (i+1)/N)`; the endpoints
//!   `i/N` and `(i+1)/N` are themselves exact f64 (division by 2^k is exact). Index
//!   `N-1` additionally takes `v == 1.0` (the clamp of `(1.0*N) as usize == N`), and
//!   its exact upper endpoint is `1.0`.
//! - The build evaluates the CLOSED interval `[v_lo, v_hi] = [i/N, (i+1)/N]` — which
//!   encloses that half-open pre-image (index `N-1` includes 1.0 exactly) — with the
//!   EXACT scalar body itself (`color::srgb_encode_scalar`). If `encode(v_lo) ==
//!   encode(v_hi) == n` then, because `encode` is monotonic non-decreasing,
//!   `encode(v) == n` for every `v` in the cell (monotone + equal endpoints ⇒
//!   uniform). Store `n`. Otherwise the cell straddles a byte transition → FALLBACK.
//! - The table is computed FROM `srgb_encode_scalar`, so a cached byte can only ever
//!   disagree with scalar through an index-enclosure bug — and the four-way corpus
//!   cert (`tests/icon-parity/m6/run.ts`, 0/389,808,128 fast-vs-scalar) plus the
//!   dense `v`-sweep test would catch that as a nonzero diff. A false FALLBACK only
//!   costs one scalar `pow`; a false hit is impossible without a monotonicity break.

use std::sync::OnceLock;

use crate::color::srgb_encode_scalar;

/// Cells over `v ∈ [0,1]`. A power of two so `v * N` is exact (see module doc);
/// `1 << 16` = 65,536 cells = a 128 KiB `u16` table with ~99.6% hit (only the ~255
/// cells straddling a byte transition fall back), cache-friendly for the random
/// per-pixel access pattern.
pub const N: usize = 1 << 16;

/// `N` as f64 — used by both the build endpoints and the runtime quantizer so the
/// two share one exact constant.
pub const N_F64: f64 = N as f64;

/// Stored for a cell that is NOT provably uniform: the runtime runs the exact scalar
/// `pow` body instead. Values `0..=255` are proven output bytes.
pub const FALLBACK: u16 = 256;

static TABLE: OnceLock<Box<[u16]>> = OnceLock::new();

/// The guarded table (built once). `table()[i]` is the single output byte for every
/// `v` that quantizes to index `i`, or [`FALLBACK`] if the cell straddles a byte
/// transition.
#[inline]
pub fn table() -> &'static [u16] {
    TABLE.get_or_init(build).as_ref()
}

fn build() -> Box<[u16]> {
    let mut t = vec![FALLBACK; N].into_boxed_slice();
    for (i, slot) in t.iter_mut().enumerate() {
        let v_lo = i as f64 / N_F64;
        let v_hi = (i + 1) as f64 / N_F64; // i == N-1 ⇒ exactly 1.0
        let lo = srgb_encode_scalar(v_lo);
        let hi = srgb_encode_scalar(v_hi);
        *slot = if lo == hi { lo as u16 } else { FALLBACK };
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_full_and_only_bytes_or_fallback() {
        let t = table();
        assert_eq!(t.len(), N);
        for &c in t {
            assert!(c <= 255 || c == FALLBACK, "cell out of range: {c}");
        }
    }

    #[test]
    fn endpoints_and_hit_rate() {
        let t = table();
        assert_eq!(t[0], 0, "darkest cell must resolve to byte 0");
        assert_eq!(t[N - 1], 255, "brightest cell must resolve to byte 255");
        let fallbacks = t.iter().filter(|&&c| c == FALLBACK).count();
        // One transition cell per byte edge (0→1 … 254→255) ⇒ a few hundred cells.
        assert!(fallbacks < N / 100, "hit rate below 99%: {fallbacks} fallbacks of {N}");
        assert!(fallbacks >= 200, "suspiciously few fallbacks ({fallbacks}) — enclosure too loose?");
    }

    #[test]
    fn every_hit_cell_is_uniform_across_its_endpoints() {
        // The build only stores a byte when the endpoints agree; re-assert it so a
        // future refactor of `build` can't quietly weaken the guard.
        let t = table();
        for (i, &c) in t.iter().enumerate() {
            if c == FALLBACK {
                continue;
            }
            let v_lo = i as f64 / N_F64;
            let v_hi = (i + 1) as f64 / N_F64;
            assert_eq!(srgb_encode_scalar(v_lo) as u16, c, "hit cell {i} lo != stored");
            assert_eq!(srgb_encode_scalar(v_hi) as u16, c, "hit cell {i} hi != stored (straddle stored as hit)");
        }
    }
}
