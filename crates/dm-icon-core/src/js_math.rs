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
}
