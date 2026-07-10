//! JavaScript numeric semantics, mirrored exactly (ADR-0019 §wasm↔native).
//!
//! The frozen TS oracle stores bytes through `Math.round` + `Uint8ClampedArray`
//! assignment. Rust's `f64::round` rounds half AWAY FROM ZERO while JS
//! `Math.round` rounds half toward +∞, and a `Uint8ClampedArray` store rounds
//! half to EVEN — three different rules at one byte boundary. Every byte-facing
//! call site in this crate goes through these helpers; bare `.round()` is
//! banned in the core.

/// `Math.round` (ES2026 §Math.round): the integral Number closest to `x`,
/// ties toward +∞. NOT `floor(x + 0.5)` — for `x = 0.49999999999999994`,
/// `x + 0.5` rounds up to `1.0` in f64 but the closest integer is `0`.
pub fn js_round(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let f = x.floor();
    if x - f >= 0.5 {
        f + 1.0
    } else {
        f
    }
}

/// `Math.trunc`.
pub fn js_trunc(x: f64) -> f64 {
    x.trunc()
}

/// `Uint8ClampedArray` assignment (ES2026 ToUint8Clamp): clamp to [0,255],
/// round half to EVEN. Only reached by NON-integer stores — the slice always
/// pre-rounds via `js_round`, but M5 modules (filters, backdropBlur) store raw
/// products and MUST use this.
pub fn clamp_u8_round_half_even(v: f64) -> u8 {
    if v.is_nan() || v <= 0.0 {
        return 0;
    }
    if v >= 255.0 {
        return 255;
    }
    let f = v.floor();
    let frac = v - f;
    let round_up = frac > 0.5 || (frac == 0.5 && !(f as u64).is_multiple_of(2));
    (if round_up { f + 1.0 } else { f }) as u8
}

/// A `js_round` result stored into a `Uint8ClampedArray` slot: the round value
/// is integral, so the store only clamps.
pub fn clamp_u8_int(v: f64) -> u8 {
    if v <= 0.0 {
        0
    } else if v >= 255.0 {
        255
    } else {
        v as u8
    }
}

/// TS `clampByte` (raster.ts): clamp FIRST, then `Math.round`.
pub fn clamp_byte(v: f64) -> u8 {
    if v < 0.0 {
        0
    } else if v > 255.0 {
        255
    } else {
        js_round(v) as u8
    }
}

/// TS `clamp01` (raster.ts). `f64::clamp` matches the TS branch chain on every
/// input incl. NaN (both pass NaN through).
pub fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_round_matches_math_round() {
        assert_eq!(js_round(0.5), 1.0);
        assert_eq!(js_round(1.5), 2.0);
        assert_eq!(js_round(2.5), 3.0); // Rust f64::round would give 3.0 too…
        assert_eq!(js_round(-0.5), 0.0); // …but here Rust rounds to -1.0
        assert_eq!(js_round(-1.5), -1.0);
        assert_eq!(js_round(-0.6), -1.0);
        // The floor(x+0.5) trap: closest integer to this is 0, x+0.5 == 1.0 in f64.
        assert_eq!(js_round(0.499_999_999_999_999_94), 0.0);
        assert_eq!(js_round(254.499_999_999_999_97), 254.0);
    }

    #[test]
    fn uint8_clamped_ties_go_even() {
        assert_eq!(clamp_u8_round_half_even(0.5), 0);
        assert_eq!(clamp_u8_round_half_even(1.5), 2);
        assert_eq!(clamp_u8_round_half_even(2.5), 2);
        assert_eq!(clamp_u8_round_half_even(254.5), 254);
        assert_eq!(clamp_u8_round_half_even(254.7), 255);
        assert_eq!(clamp_u8_round_half_even(-3.0), 0);
        assert_eq!(clamp_u8_round_half_even(300.0), 255);
        assert_eq!(clamp_u8_round_half_even(f64::NAN), 0);
    }

    #[test]
    fn clamp_byte_clamps_before_rounding() {
        assert_eq!(clamp_byte(255.4), 255);
        assert_eq!(clamp_byte(254.5), 255); // Math.round, not half-even
        assert_eq!(clamp_byte(-0.4), 0);
        assert_eq!(clamp_byte(127.5), 128);
    }

    // ---- exhaustive byte-boundary references (js_math is the parity foundation) ----

    /// ECMAScript ToUint8Clamp: clamp to [0,255], round half to EVEN.
    fn to_uint8_clamp(x: f64) -> u8 {
        if x.is_nan() || x <= 0.0 {
            return 0;
        }
        if x >= 255.0 {
            return 255;
        }
        let f = x.floor();
        let d = x - f;
        let up = d > 0.5 || (d == 0.5 && (f as i64) % 2 == 1);
        (if up { f + 1.0 } else { f }) as u8
    }

    /// `Math.round`: floor(x) + 1 iff the fraction is ≥ 0.5 (ties toward +∞).
    fn math_round(x: f64) -> f64 {
        let f = x.floor();
        if x - f >= 0.5 {
            f + 1.0
        } else {
            f
        }
    }

    #[test]
    fn clamp_u8_int_is_identity_in_range_and_saturates_outside() {
        for v in 0..=255u32 {
            assert_eq!(clamp_u8_int(v as f64), v as u8);
        }
        assert_eq!(clamp_u8_int(-0.1), 0);
        assert_eq!(clamp_u8_int(-1000.0), 0);
        assert_eq!(clamp_u8_int(255.0), 255);
        assert_eq!(clamp_u8_int(255.9), 255);
        assert_eq!(clamp_u8_int(1e9), 255);
    }

    #[test]
    fn clamp_u8_round_half_even_matches_reference_exhaustively() {
        // Every integer + half-integer across (and past) the range: exercises the
        // ties-to-even path at all 256 midpoints, plus out-of-range and NaN.
        let mut k = -8;
        while k <= 520 {
            let x = k as f64 / 2.0;
            assert_eq!(clamp_u8_round_half_even(x), to_uint8_clamp(x), "half-step mismatch at {x}");
            k += 1;
        }
        // Dense thirds to cover non-half fractions too.
        for k in -30..=(255 * 3 + 30) {
            let x = k as f64 / 3.0;
            assert_eq!(clamp_u8_round_half_even(x), to_uint8_clamp(x), "thirds mismatch at {x}");
        }
        assert_eq!(clamp_u8_round_half_even(f64::NAN), 0);
    }

    #[test]
    fn js_round_matches_reference_over_a_dense_sweep() {
        for k in -2000..=2000 {
            let x = k as f64 / 7.0;
            assert_eq!(js_round(x), math_round(x), "mismatch at {x}");
        }
        // Exact half-integers: ties always go UP toward +∞ (NOT half-even).
        for n in -5..=5 {
            assert_eq!(js_round(n as f64 + 0.5), n as f64 + 1.0);
        }
    }

    #[test]
    fn clamp_byte_matches_clamp_then_math_round_exhaustively() {
        for k in -40..=(255 * 4 + 40) {
            let x = k as f64 / 4.0;
            let expect = if x < 0.0 {
                0.0
            } else if x > 255.0 {
                255.0
            } else {
                math_round(x)
            };
            assert_eq!(clamp_byte(x), expect as u8, "mismatch at {x}");
        }
    }

    #[test]
    fn clamp01_saturates_and_passes_nan_through() {
        assert_eq!(clamp01(-0.5), 0.0);
        assert_eq!(clamp01(0.3), 0.3);
        assert_eq!(clamp01(1.5), 1.0);
        assert!(clamp01(f64::NAN).is_nan());
    }
}
