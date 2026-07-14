//! The ONE per-icon metadata extraction — 1:1 port of the frozen `profile.ts`.
//! Every icon yields the same profile (classification, own background, subject /
//! rim colour + lightness, subject mask), computed once per source and consumed
//! by every downstream stage. Pure; a RenderSession owns the caching.

use crate::analysis::{
    bounds_h, bounds_w, dominant_color, find_content_bounds, has_transparent_edges,
    try_detect_background, ContentBounds,
};
use crate::color::perceived_lightness;
use crate::js_math::js_round;
use crate::raster::{Raster, Rgba};
use crate::segment::{segment_subject, Segmentation};

/// Owner five-step classification (profile.ts `IconProfileKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconProfileKind {
    FullSquare,
    OwnBoard,
    Bare,
}

pub struct IconProfile {
    pub kind: IconProfileKind,
    pub transparent_edges: bool,
    pub background: Option<Rgba>,
    pub background_lightness: Option<f64>,
    pub subject_colour: Option<Rgba>,
    pub subject_lightness: f64,
    pub subject_mask: Option<Vec<u8>>,
    pub subject_rim_colour: Option<Rgba>,
    pub subject_rim_lightness: f64,
}

const FULL_SQUARE_MIN_COVERAGE: f64 = 0.98;
const RIM_SOLID_MIN_ALPHA: u8 = 245;
const RIM_BAND_MIN_DEPTH: usize = 2;
const RIM_BAND_DEPTH_DIVISOR: usize = 16;

fn mask_mean_lightness(c: &Raster, mask: Option<&[u8]>) -> f64 {
    let d = &c.data;
    let mut sum = 0.0f64;
    let mut n = 0u32;
    let total = c.width * c.height;
    for i in 0..total {
        if let Some(m) = mask {
            if m[i] == 0 {
                continue;
            }
        }
        let i4 = i * 4;
        if d[i4 + 3] < 128 {
            continue;
        }
        sum += perceived_lightness(d[i4], d[i4 + 1], d[i4 + 2]);
        n += 1;
    }
    if n == 0 {
        0.5
    } else {
        sum / n as f64
    }
}

/// The artwork's outermost BAND: majority hue + mean lightness (profile.ts
/// `subjectRim`). NOT mask-aware — the whole opaque artwork's edge touches the
/// plate. `band` accumulates across the two alpha passes exactly as the TS does.
fn subject_rim(c: &Raster) -> (Option<Rgba>, f64) {
    let d = &c.data;
    let ww = c.width;
    let hh = c.height;
    let n = ww * hh;
    let depth =
        RIM_BAND_MIN_DEPTH.max(js_round(ww.min(hh) as f64 / RIM_BAND_DEPTH_DIVISOR as f64) as usize);
    let mut band = vec![0u8; n];
    // Reuse the two erosion buffers across every depth pass AND both alpha passes instead
    // of `clone()`-ing per pass. `cur` is fully re-seeded from `d` at the top of each alpha
    // pass; `next` is `copy_from_slice(&cur)`-restored to the exact clone-starting state
    // before every erosion pass, then the two are swapped — byte-identical to the old
    // `let mut next = cur.clone(); …; cur = next`, pure allocation removal.
    let mut cur = vec![0u8; n];
    let mut next = vec![0u8; n];
    for min_alpha in [RIM_SOLID_MIN_ALPHA, 128u8] {
        for i in 0..n {
            cur[i] = if d[i * 4 + 3] >= min_alpha { 1 } else { 0 };
        }
        for _pass in 0..depth {
            next.copy_from_slice(&cur);
            for y in 0..hh {
                for x in 0..ww {
                    let i = y * ww + x;
                    if cur[i] == 0 {
                        continue;
                    }
                    let interior = x > 0
                        && cur[i - 1] != 0
                        && x < ww - 1
                        && cur[i + 1] != 0
                        && y > 0
                        && cur[i - ww] != 0
                        && y < hh - 1
                        && cur[i + ww] != 0;
                    if !interior {
                        band[i] = 1;
                        next[i] = 0;
                    }
                }
            }
            std::mem::swap(&mut cur, &mut next);
        }
        let mut sum_l = 0.0f64;
        let mut cnt = 0u32;
        for i in 0..n {
            if band[i] == 0 {
                continue;
            }
            let i4 = i * 4;
            sum_l += perceived_lightness(d[i4], d[i4 + 1], d[i4 + 2]);
            cnt += 1;
        }
        if cnt == 0 {
            continue;
        }
        let colour = dominant_color(c, Some(&band)).map(|dc| Rgba { a: 255, ..dc.colour });
        return (colour, sum_l / cnt as f64);
    }
    (None, 0.5)
}

/// profile.ts `iconProfile`. Standalone: computes the shared sub-analysis (background,
/// segmentation) LAZILY — only the non-FullSquare branch pays for them, exactly as
/// before. The shared-bundle entry is [`icon_profile_from`].
pub fn icon_profile(c: &Raster) -> IconProfile {
    let transparent_edges = has_transparent_edges(c);
    let bounds = find_content_bounds(c);
    if is_full_square(c, transparent_edges, bounds) {
        full_square_profile(c, transparent_edges)
    } else {
        let background = try_detect_background(c);
        let mask = segment_subject(c).mask;
        subject_profile(c, transparent_edges, mask, background)
    }
}

/// Build the profile from ALREADY-computed shared analysis — the source analysis
/// bundle path (codex R2 C-5). `transparent_edges`, content `bounds`, the detected
/// `background`, and the ONE shared `segmentation` are computed once and shared with
/// `SourceFacts`, so `segment_subject` / `try_detect_background` do NOT run a second
/// time. BYTE-IDENTICAL to `icon_profile(c)` when every input equals its standalone
/// recompute (`build_analysis_bundle` guarantees that via the exact-input `_with_*`
/// variants): the FullSquare test and both field builders are the same code; the ONLY
/// change is the subject mask is CLONED out of the shared `Segmentation` instead of
/// recomputed — a memcpy that yields the identical bytes the BFS would.
pub fn icon_profile_from(
    c: &Raster,
    transparent_edges: bool,
    bounds: ContentBounds,
    background: Option<Rgba>,
    segmentation: &Segmentation,
) -> IconProfile {
    if is_full_square(c, transparent_edges, bounds) {
        full_square_profile(c, transparent_edges)
    } else {
        subject_profile(c, transparent_edges, segmentation.mask.clone(), background)
    }
}

/// A filled standard square (opaque edges + ≥98% coverage) is a complete subject by
/// itself. Depends only on `transparent_edges` + content `bounds`, both always cheap.
fn is_full_square(c: &Raster, transparent_edges: bool, bounds: ContentBounds) -> bool {
    let coverage = (bounds_w(bounds) * bounds_h(bounds)) as f64 / (c.width * c.height) as f64;
    !transparent_edges && coverage >= FULL_SQUARE_MIN_COVERAGE
}

/// The FullSquare profile — no own background, no subject mask.
fn full_square_profile(c: &Raster, transparent_edges: bool) -> IconProfile {
    let (rim_colour, rim_lightness) = subject_rim(c);
    IconProfile {
        kind: IconProfileKind::FullSquare,
        transparent_edges,
        background: None,
        background_lightness: None,
        subject_colour: dominant_color(c, None).map(|d| d.colour),
        subject_lightness: mask_mean_lightness(c, None),
        subject_mask: None,
        subject_rim_colour: rim_colour,
        subject_rim_lightness: rim_lightness,
    }
}

/// The OwnBoard / Bare profile, built from the segmentation `mask` (moved in) and the
/// detected `background`. Shared by the standalone and bundle entry points.
fn subject_profile(
    c: &Raster,
    transparent_edges: bool,
    mask: Vec<u8>,
    background: Option<Rgba>,
) -> IconProfile {
    let subject_colour = dominant_color(c, Some(&mask)).map(|d| d.colour);
    let subject_lightness = mask_mean_lightness(c, Some(&mask));
    let (rim_colour, rim_lightness) = subject_rim(c);
    match background {
        Some(bg) => IconProfile {
            kind: IconProfileKind::OwnBoard,
            transparent_edges,
            background: Some(Rgba { a: 255, ..bg }),
            background_lightness: Some(perceived_lightness(bg.r, bg.g, bg.b)),
            subject_colour,
            subject_lightness,
            subject_mask: Some(mask),
            subject_rim_colour: rim_colour,
            subject_rim_lightness: rim_lightness,
        },
        None => IconProfile {
            kind: IconProfileKind::Bare,
            transparent_edges,
            background: None,
            background_lightness: None,
            subject_colour,
            subject_lightness,
            subject_mask: Some(mask),
            subject_rim_colour: rim_colour,
            subject_rim_lightness: rim_lightness,
        },
    }
}

#[cfg(test)]
mod rim_erosion_reuse_cert {
    use super::*;

    /// The exact erosion accumulation `subject_rim` runs, done the OLD way (a fresh
    /// `cur.clone()` per pass) — the byte-identity reference for the buffer-reuse form.
    fn band_via_clone(c: &Raster, depth: usize) -> Vec<u8> {
        let d = &c.data;
        let (ww, hh) = (c.width, c.height);
        let n = ww * hh;
        let mut band = vec![0u8; n];
        for min_alpha in [RIM_SOLID_MIN_ALPHA, 128u8] {
            let mut cur = vec![0u8; n];
            for i in 0..n {
                cur[i] = if d[i * 4 + 3] >= min_alpha { 1 } else { 0 };
            }
            for _pass in 0..depth {
                let mut next = cur.clone();
                for y in 0..hh {
                    for x in 0..ww {
                        let i = y * ww + x;
                        if cur[i] == 0 {
                            continue;
                        }
                        let interior = x > 0
                            && cur[i - 1] != 0
                            && x < ww - 1
                            && cur[i + 1] != 0
                            && y > 0
                            && cur[i - ww] != 0
                            && y < hh - 1
                            && cur[i + ww] != 0;
                        if !interior {
                            band[i] = 1;
                            next[i] = 0;
                        }
                    }
                }
                cur = next;
            }
        }
        band
    }

    /// The buffer-reuse form (allocate `cur`/`next` once, `copy_from_slice` + swap) — the
    /// SHAPE of the production `subject_rim` inner loop after Win 3.
    fn band_via_reuse(c: &Raster, depth: usize) -> Vec<u8> {
        let d = &c.data;
        let (ww, hh) = (c.width, c.height);
        let n = ww * hh;
        let mut band = vec![0u8; n];
        let mut cur = vec![0u8; n];
        let mut next = vec![0u8; n];
        for min_alpha in [RIM_SOLID_MIN_ALPHA, 128u8] {
            for i in 0..n {
                cur[i] = if d[i * 4 + 3] >= min_alpha { 1 } else { 0 };
            }
            for _pass in 0..depth {
                next.copy_from_slice(&cur);
                for y in 0..hh {
                    for x in 0..ww {
                        let i = y * ww + x;
                        if cur[i] == 0 {
                            continue;
                        }
                        let interior = x > 0
                            && cur[i - 1] != 0
                            && x < ww - 1
                            && cur[i + 1] != 0
                            && y > 0
                            && cur[i - ww] != 0
                            && y < hh - 1
                            && cur[i + ww] != 0;
                        if !interior {
                            band[i] = 1;
                            next[i] = 0;
                        }
                    }
                }
                std::mem::swap(&mut cur, &mut next);
            }
        }
        band
    }

    fn alpha_fixture(n: usize, lo: usize, hi: usize, hole: bool) -> Raster {
        let mut r = Raster::new(n, n);
        for y in lo..hi {
            for x in lo..hi {
                r.data[(y * n + x) * 4 + 3] = 255;
            }
        }
        if hole {
            // Punch a transparent hole so both alpha passes AND the interior test exercise
            // non-trivial erosion fronts.
            for y in (lo + 3)..(hi - 3) {
                for x in (lo + 3)..(hi - 3) {
                    r.data[(y * n + x) * 4 + 3] = 0;
                }
            }
        }
        r
    }

    /// Buffer-reuse erosion must produce the BYTE-IDENTICAL `band` the per-pass clone does,
    /// for several alpha shapes and depths — the pure-allocation-removal proof for Win 3.
    #[test]
    fn reuse_band_is_byte_identical_to_clone_band() {
        let fixtures = [
            alpha_fixture(16, 0, 16, false),
            alpha_fixture(24, 4, 20, false),
            alpha_fixture(32, 6, 26, true),
            alpha_fixture(20, 2, 18, true),
        ];
        for c in &fixtures {
            for depth in [1usize, 2, 3, 5] {
                assert_eq!(
                    band_via_reuse(c, depth),
                    band_via_clone(c, depth),
                    "reuse erosion diverged from clone erosion (depth {depth})"
                );
            }
        }
    }
}
