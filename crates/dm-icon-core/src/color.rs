//! Colour math — 1:1 port of the frozen `color.ts` (slice subset: sRGB
//! transfer + the OKLab machinery behind `fieldShadowTone`). The rest of the
//! file (mono ramp, field plates, hue rotation) ports at M5.
//!
//! Transcendentals (`pow`, `cbrt`, `sqrt`) route through `libm` ONLY —
//! `f64::powf` may lower to target-specific instructions and break the
//! wasm↔native byte gate (ADR-0019). The TS oracle computes these through
//! JSC's `Math.pow`/`Math.cbrt`; whether JSC and musl-libm agree bit-for-bit
//! is exactly what the Spike-4 fixture probes measure.

use crate::config::Band;
use crate::js_math::{clamp01, clamp_byte, js_round};
use crate::raster::{from_rgb_int, Rgba};
use std::sync::OnceLock;

/// Ink threshold for 原彩 (color.ts `ORIGINAL_INK_THRESHOLD`), distinct from the
/// mark adaptivity threshold (0.58).
pub const ORIGINAL_INK_THRESHOLD: f64 = 0.66;

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

// ---- prototype primitives (color.ts) ----

/// Rec.601 luminance of 0-255 channels (color.ts `luminance`).
pub fn luminance(r: u8, g: u8, b: u8) -> f64 {
    (0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64) / 255.0
}

/// color.ts `grayValue`: `255·clamp(0.5+(l−0.5)·1.4, 0.08, 0.94)`, `Math.round`ed.
/// Returns the integral `Number` the TS stores into a `Uint8ClampedArray`.
pub fn gray_value(l: f64) -> f64 {
    js_round(255.0 * (0.5 + (l - 0.5) * 1.4).clamp(0.08, 0.94))
}

/// Hue (0-360) and saturation (0-1) of a packed `0xRRGGBB` (color.ts `hslOf`).
pub fn hsl_of(rgb: u32) -> (f64, f64) {
    let r = ((rgb >> 16) & 0xff) as f64 / 255.0;
    let g = ((rgb >> 8) & 0xff) as f64 / 255.0;
    let b = (rgb & 0xff) as f64 / 255.0;
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let d = mx - mn;
    let mut h = 0.0;
    if d > 0.0 {
        if mx == r {
            h = ((g - b) / d) % 6.0;
        } else if mx == g {
            h = (b - r) / d + 2.0;
        } else {
            h = (r - g) / d + 4.0;
        }
        h = js_round(h * 60.0);
        if h < 0.0 {
            h += 360.0;
        }
    }
    let light = (mx + mn) / 2.0;
    let s = if d > 0.0 { d / (1.0 - (2.0 * light - 1.0).abs()) } else { 0.0 };
    (h, s)
}

/// CSS `hsl(h[0-360], s[0-1], l[0-100])` → opaque RGB (color.ts `hslToRgb`).
pub fn hsl_to_rgb(h: f64, s: f64, l_percent: f64) -> Rgba {
    let l = l_percent / 100.0;
    let sat = s.clamp(0.0, 1.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * sat;
    let hp = (((h % 360.0) + 360.0) % 360.0) / 60.0;
    let x = c * (1.0 - ((hp % 2.0) - 1.0).abs());
    let (mut r, mut g, mut b) = (0.0f64, 0.0f64, 0.0f64);
    match (hp.floor() as i64).rem_euclid(6) {
        0 => {
            r = c;
            g = x;
        }
        1 => {
            r = x;
            g = c;
        }
        2 => {
            g = c;
            b = x;
        }
        3 => {
            g = x;
            b = c;
        }
        4 => {
            r = x;
            b = c;
        }
        _ => {
            r = c;
            b = x;
        }
    }
    let m = l - c / 2.0;
    Rgba {
        r: clamp_byte((r + m) * 255.0),
        g: clamp_byte((g + m) * 255.0),
        b: clamp_byte((b + m) * 255.0),
        a: 255,
    }
}

// ---- OKLab ----

#[derive(Clone, Copy, Debug)]
pub(crate) struct OkLab {
    pub(crate) l: f64,
    pub(crate) a: f64,
    pub(crate) b: f64,
}

/// color.ts `toOkLab` — public tuple form for callers that only read L/A/B.
pub fn to_ok_lab(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let lab = ok_lab_of(r, g, b);
    (lab.l, lab.a, lab.b)
}

/// Perceived lightness (OKLab L) of 0-255 channels (color.ts `perceivedLightness`).
pub fn perceived_lightness(r: u8, g: u8, b: u8) -> f64 {
    ok_lab_of(r, g, b).l
}

pub(crate) fn ok_lab_of(r: u8, g: u8, b: u8) -> OkLab {
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
pub(crate) fn gamut_fit(l: f64, ua: f64, ub: f64, c: f64) -> Rgba {
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

pub(crate) struct HueUnit {
    pub(crate) ua: f64,
    pub(crate) ub: f64,
    pub(crate) chroma: f64,
    pub(crate) l: f64,
}

/// color.ts `hueUnit`.
pub(crate) fn hue_unit(seed: Rgba) -> HueUnit {
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

// ---- 满彩 Field harmony band + plated clamp (color.ts) ----

struct FieldSlot {
    l: f64,
    c_min: f64,
    c_max: f64,
}

fn field_slot(band: Band) -> FieldSlot {
    match band {
        Band::Vivid => FieldSlot { l: 0.87, c_min: 0.09, c_max: 0.12 },
        Band::Quiet => FieldSlot { l: 0.91, c_min: 0.04, c_max: 0.07 },
    }
}

const PLATED_L_MIN: f64 = 0.6;
const PLATED_L_MAX: f64 = 0.8;
const PLATED_NEUTRAL_CHROMA: f64 = 0.04;

/// The Field plate for a seed colour (color.ts `fieldPlateTone`): the seed's hue
/// in the band's shared lightness line, gamut-limited chroma per icon.
pub fn field_plate_tone(seed: Rgba, band: Band, chroma_window: Option<(f64, f64)>) -> Rgba {
    let slot = field_slot(band);
    let (c_min, c_max) = chroma_window.unwrap_or((slot.c_min, slot.c_max));
    let HueUnit { ua, ub, chroma, .. } = hue_unit(seed);
    let c = chroma.clamp(c_min, c_max);
    gamut_fit(slot.l, ua, ub, c)
}

/// A plated source keeps its OWN plate colour, chromatic plates clamped into the
/// light window, near-neutral plates untouched (color.ts `clampPlateLightness`).
pub fn clamp_plate_lightness(bg: Rgba) -> Rgba {
    let HueUnit { ua, ub, chroma, l } = hue_unit(bg);
    if chroma < PLATED_NEUTRAL_CHROMA {
        return Rgba { a: 255, ..bg };
    }
    if (PLATED_L_MIN..=PLATED_L_MAX).contains(&l) {
        return Rgba { a: 255, ..bg };
    }
    let target = l.clamp(PLATED_L_MIN, PLATED_L_MAX);
    gamut_fit(target, ua, ub, chroma)
}

/// Only genuinely LIGHT subjects take a dark board (color.ts).
const DARK_BOARD_SUBJECT_MIN_L: f64 = 0.7;

/// The NEUTRAL lightness-contrast plate for artwork with no theme colour
/// (color.ts `neutralContrastTone`).
pub fn neutral_contrast_tone(subject_mean_l: f64) -> Rgba {
    let l = if subject_mean_l >= DARK_BOARD_SUBJECT_MIN_L {
        (subject_mean_l - 0.45).clamp(0.2, 0.42)
    } else {
        (subject_mean_l + 0.45).clamp(0.82, 0.97)
    };
    gamut_fit(l, 0.0, 0.0, 0.0)
}

/// Deep boards must still carry their hue (color.ts `DEEP_MIN_CHROMA`).
const DEEP_MIN_CHROMA: f64 = 0.09;
const DEEP_MAX_LIFT: f64 = 0.12;

/// The THEMED contrast plate (color.ts `themedContrastTone`): the plate takes the
/// theme hue at whichever lightness side sits further from the subject's mean.
pub fn themed_contrast_tone(seed: Rgba, subject_mean_l: f64, band: Band) -> Rgba {
    let HueUnit { mut ua, mut ub, chroma, .. } = hue_unit(seed);
    let light_l = if band == Band::Quiet { 0.91 } else { 0.87 };
    let dark_l = if band == Band::Quiet { 0.34 } else { 0.3 };
    let use_dark = subject_mean_l >= DARK_BOARD_SUBJECT_MIN_L;
    if !use_dark {
        return gamut_fit(light_l, ua, ub, chroma.clamp(0.06, 0.1));
    }

    // Deep boards: pull yellow-green toward amber (深金, never 军绿)…
    let deg = libm::atan2(ub, ua) * 180.0 / std::f64::consts::PI;
    if deg > 82.0 && deg < 125.0 {
        let rad = (78.0 + (deg - 82.0) * 0.15) * std::f64::consts::PI / 180.0;
        ua = libm::cos(rad);
        ub = libm::sin(rad);
    }
    // …and buy chroma headroom by lifting L until the FITTED plate keeps colour.
    let c_req = chroma.clamp(0.06, 0.12);
    let mut l = dark_l;
    let mut plate = gamut_fit(l, ua, ub, c_req);
    while c_req >= DEEP_MIN_CHROMA && l < dark_l + DEEP_MAX_LIFT {
        let lab = ok_lab_of(plate.r, plate.g, plate.b);
        if libm::sqrt(lab.a * lab.a + lab.b * lab.b) >= DEEP_MIN_CHROMA {
            break;
        }
        l += 0.02;
        plate = gamut_fit(l, ua, ub, c_req);
    }
    plate
}

/// The seed with its OKLab hue rotated by `delta_rad` (color.ts `rotateSeedHue`).
pub fn rotate_seed_hue(seed: Rgba, delta_rad: f64) -> Rgba {
    if delta_rad == 0.0 {
        return Rgba { a: 255, ..seed };
    }
    let HueUnit { ua, ub, chroma, l } = hue_unit(seed);
    if chroma < 1e-6 {
        return Rgba { a: 255, ..seed };
    }
    let cos = libm::cos(delta_rad);
    let sin = libm::sin(delta_rad);
    gamut_fit(l, ua * cos - ub * sin, ua * sin + ub * cos, chroma)
}

/// The colour with its OKLab lightness shifted by `d_l` (color.ts `shiftLightness`).
pub fn shift_lightness(c: Rgba, d_l: f64) -> Rgba {
    let HueUnit { ua, ub, chroma, l } = hue_unit(c);
    gamut_fit((l + d_l).clamp(0.05, 0.98), ua, ub, chroma)
}

/// color.ts `monoTone` — one tone of the tint's hue at the given OKLab lightness
/// and chroma scale, gamut-fit. (Shared with the mono ramp in `mono.rs`.)
pub fn mono_tone(lightness: f64, chroma_scale: f64, tint: u32) -> Rgba {
    let HueUnit { ua, ub, chroma, .. } = hue_unit(from_rgb_int(tint));
    let c = chroma.clamp(0.035, 0.145) * chroma_scale;
    gamut_fit(lightness, ua, ub, c)
}

/// color.ts `paleTone` — a pale, near-white tone of the tint's hue.
pub fn pale_tone(tint: u32) -> Rgba {
    mono_tone(0.965, 0.5, tint)
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
