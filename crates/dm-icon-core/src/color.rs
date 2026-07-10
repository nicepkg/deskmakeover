//! Colour math — 1:1 port of the frozen `color.ts` (slice subset: sRGB
//! transfer + the OKLab machinery behind `fieldShadowTone`). The rest of the
//! file (mono ramp, field plates, hue rotation) ports at M5.
//!
//! Transcendentals (`pow`, `cbrt`, `sqrt`) route through `libm` ONLY —
//! `f64::powf` may lower to target-specific instructions and break the
//! wasm↔native byte gate (ADR-0019). The TS oracle computes these through
//! JSC's `Math.pow`/`Math.cbrt`; whether JSC and musl-libm agree bit-for-bit
//! is exactly what the Spike-4 fixture probes measure.

use crate::js_math::{clamp01, clamp_byte};
use crate::raster::Rgba;
use std::sync::OnceLock;

// ---- sRGB ↔ linear (SrgbLinear.cs) ----

static DECODE_LUT: OnceLock<[f64; 256]> = OnceLock::new();

fn decode_lut() -> &'static [f64; 256] {
    DECODE_LUT.get_or_init(|| {
        let mut lut = [0.0f64; 256];
        for (i, slot) in lut.iter_mut().enumerate() {
            let srgb = i as f64 / 255.0;
            *slot = if srgb <= 0.04045 {
                srgb / 12.92
            } else {
                libm::pow((srgb + 0.055) / 1.055, 2.4)
            };
        }
        lut
    })
}

/// sRGB byte → linear-light [0,1] (color.ts `srgbDecode`).
pub fn srgb_decode(value: u8) -> f64 {
    decode_lut()[value as usize]
}

/// Linear-light [0,1] → sRGB byte (color.ts `srgbEncode`; exact transfer curve).
pub fn srgb_encode(linear: f64) -> u8 {
    let v = clamp01(linear);
    let srgb = if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * libm::pow(v, 1.0 / 2.4) - 0.055
    };
    clamp_byte(srgb * 255.0)
}

// ---- OKLab (private math the shadow tone rides on) ----

#[derive(Clone, Copy, Debug)]
struct OkLab {
    l: f64,
    a: f64,
    b: f64,
}

/// color.ts `toOkLab` (public there for analysis; the slice uses it via
/// `field_shadow_tone` and the M5 analysis port will need it directly).
pub fn to_ok_lab(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let lab = ok_lab_of(r, g, b);
    (lab.l, lab.a, lab.b)
}

fn ok_lab_of(r: u8, g: u8, b: u8) -> OkLab {
    let lut = decode_lut();
    let rl = lut[r as usize];
    let gl = lut[g as usize];
    let bl = lut[b as usize];
    let l = libm::cbrt(0.4122214708 * rl + 0.5363325363 * gl + 0.0514459929 * bl);
    let m = libm::cbrt(0.2119034982 * rl + 0.6806995451 * gl + 0.1073969566 * bl);
    let s = libm::cbrt(0.0883024619 * rl + 0.2817188376 * gl + 0.6299787005 * bl);
    OkLab {
        l: 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
        a: 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
        b: 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
    }
}

fn ok_lab_to_linear(lab: OkLab) -> (f64, f64, f64) {
    let mut l = lab.l + 0.3963377774 * lab.a + 0.2158037573 * lab.b;
    let mut m = lab.l - 0.1055613458 * lab.a - 0.0638541728 * lab.b;
    let mut s = lab.l - 0.0894841775 * lab.a - 1.291485548 * lab.b;
    l = l * l * l;
    m = m * m * m;
    s = s * s * s;
    (
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
    )
}

fn try_ok_lab_to_srgb(lab: OkLab) -> (Rgba, bool) {
    let (r, g, b) = ok_lab_to_linear(lab);
    let in_gamut = (-0.0005..=1.0005).contains(&r)
        && (-0.0005..=1.0005).contains(&g)
        && (-0.0005..=1.0005).contains(&b);
    let enc = |v: f64| srgb_encode(v.clamp(0.0, 1.0));
    (Rgba { r: enc(r), g: enc(g), b: enc(b), a: 255 }, in_gamut)
}

/// color.ts `gamutFit`: walk chroma down by ×0.82 up to 8 times until sRGB
/// holds the tone.
fn gamut_fit(l: f64, ua: f64, ub: f64, c: f64) -> Rgba {
    let mut c = c;
    for _ in 0..8 {
        let (rgb, in_gamut) = try_ok_lab_to_srgb(OkLab { l, a: ua * c, b: ub * c });
        if in_gamut {
            return rgb;
        }
        c *= 0.82;
    }
    try_ok_lab_to_srgb(OkLab { l, a: ua * c, b: ub * c }).0
}

struct HueUnit {
    ua: f64,
    ub: f64,
    #[allow(dead_code)] // themedContrastTone consumes chroma at M5
    chroma: f64,
    l: f64,
}

/// color.ts `hueUnit`.
fn hue_unit(seed: Rgba) -> HueUnit {
    let lab = ok_lab_of(seed.r, seed.g, seed.b);
    let chroma = libm::sqrt(lab.a * lab.a + lab.b * lab.b);
    HueUnit {
        ua: if chroma < 1e-6 { 0.0 } else { lab.a / chroma },
        ub: if chroma < 1e-6 { 0.0 } else { lab.b / chroma },
        chroma,
        l: lab.l,
    }
}

/// The silhouette-shadow tone for a Field plate, always OPPOSING the plate
/// (color.ts `fieldShadowTone`): light plates take a deep same-hue shadow,
/// dark plates a light glow.
pub fn field_shadow_tone(plate: Rgba) -> Rgba {
    let HueUnit { ua, ub, l, .. } = hue_unit(plate);
    if l < 0.5 {
        gamut_fit(0.92, ua, ub, 0.03)
    } else {
        gamut_fit(0.38, ua, ub, 0.05)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raster::WHITE;

    #[test]
    fn decode_lut_endpoints() {
        assert_eq!(srgb_decode(0), 0.0);
        assert_eq!(srgb_decode(255), 1.0);
        assert!(srgb_decode(1) > 0.0 && srgb_decode(1) < srgb_decode(2));
    }

    #[test]
    fn encode_round_trips_every_byte() {
        for v in 0..=255u8 {
            assert_eq!(srgb_encode(srgb_decode(v)), v, "byte {v} did not round-trip");
        }
    }

    #[test]
    fn encode_clamps() {
        assert_eq!(srgb_encode(-0.5), 0);
        assert_eq!(srgb_encode(2.0), 255);
    }

    /// Cross-language fixture: the TS oracle's `fieldShadowTone(WHITE)` under
    /// Bun/JSC on the M0b corpus machine is (66, 66, 66) — pinned by
    /// scripts/spike4-slice.ts fixtures. The slice renders every shadow pixel
    /// from this one tone, so byte-parity of the whole corpus hangs on it.
    #[test]
    fn white_shadow_tone_matches_ts_oracle() {
        let s = field_shadow_tone(WHITE);
        assert_eq!((s.r, s.g, s.b, s.a), (66, 66, 66, 255));
    }

    #[test]
    fn white_is_achromatic_in_oklab() {
        let (l, a, b) = to_ok_lab(255, 255, 255);
        assert!((l - 1.0).abs() < 1e-6);
        assert!(a.abs() < 1e-7 && b.abs() < 1e-7);
    }
}
