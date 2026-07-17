//! 满彩 Field composition + the silhouette-shadow separation device — the
//! `composeField` / `drawBareWithShadow` / `monoSubjectLayer` tail of the frozen
//! `compose.ts`. Iron law (owner): subject pixels are NEVER recoloured —
//! separation comes from the plate and soft shadows.

use super::{
    compose_from_plate, composite_over, draw_centred, field_content_box, fill_region,
    ComposeDiagnostics, ComposeFieldLane, RenderOpts,
};
use crate::analysis::ContentBounds;
use crate::color::{field_shadow_tone, neutral_contrast_tone, themed_contrast_tone};
use crate::config::{Config, IconShape};
use crate::js_math::{clamp_u8_int, js_round};
use crate::profile::{icon_profile, IconProfile, IconProfileKind};
use crate::raster::{from_rgb_int, over_at, Raster, Rgba};
use crate::render_scratch::RenderScratch;
use crate::sampling::draw_scaled;
use crate::separation::{draw_separation_stroke, separation_stroke_ink};
use crate::shape_facts::ShapeFacts;
use crate::source_facts::{content_bounds, segmentation, SourceFacts};

/// Shadow modes (compose.ts `SHADOW_MODES`). `Halo` mirrors the frozen oracle's
/// defined-but-unused pale-art mode.
#[derive(Clone, Copy)]
pub(crate) enum ShadowMode {
    Dock,
    #[allow(dead_code)]
    Halo,
}

struct ShadowSpec {
    alpha: f64,
    blur_fraction: f64,
    offset_fraction: f64,
}

fn shadow_spec(mode: ShadowMode) -> ShadowSpec {
    match mode {
        ShadowMode::Dock => ShadowSpec { alpha: 0.24, blur_fraction: 0.04, offset_fraction: 0.015 },
        ShadowMode::Halo => ShadowSpec { alpha: 0.34, blur_fraction: 0.035, offset_fraction: 0.0 },
    }
}

/// 满彩 Field composition (compose.ts `composeField`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn compose_field(
    artwork: &Raster,
    content: &mut Raster,
    size: usize,
    pad: usize,
    card_size: usize,
    shape: IconShape,
    config: &Config,
    opts: &RenderOpts,
    diag: &mut ComposeDiagnostics,
    profile_override: Option<&IconProfile>,
    source_facts: Option<&SourceFacts>,
    shape_facts: Option<&ShapeFacts>,
    scratch: &mut RenderScratch,
) {
    let band = config.plate_band;
    let box_ = field_content_box(shape, card_size);
    // The RenderSession may supply the cached profile (same raster → identical);
    // otherwise it is computed here exactly as the TS oracle does.
    let computed;
    let profile: &IconProfile = match profile_override {
        Some(p) => p,
        None => {
            computed = icon_profile(artwork);
            &computed
        }
    };

    // Step 1: a filled standard square is already a complete tile — clip only.
    if profile.kind == IconProfileKind::FullSquare {
        diag.field_lane = Some(ComposeFieldLane::FullSquare);
        let full = ContentBounds { left: 0, top: 0, right: artwork.width, bottom: artwork.height };
        draw_scaled(artwork, full, content, size, pad as i32, pad as i32, card_size, card_size);
        return;
    }

    // 背景色 override IS effective in Field.
    let user_plate = config.plate_color.map(from_rgb_int);
    if let Some(user_plate) = user_plate {
        if profile.kind == IconProfileKind::OwnBoard || !profile.transparent_edges {
            diag.field_lane = Some(ComposeFieldLane::UserPlateBoard);
            compose_from_plate(artwork, content, size, pad, card_size, shape, user_plate, Some(box_), source_facts, shape_facts);
            return;
        }
        diag.field_lane = Some(ComposeFieldLane::UserPlateBare);
        fill_region(content, size, pad, card_size, user_plate.r, user_plate.g, user_plate.b);
        // 自动分离: the user's plate colour is sacred, so a melting rim is rescued
        // by the die-cut stroke, never by changing the plate.
        let stroke = separation_stroke_ink(config, artwork, user_plate);
        draw_bare_with_shadow(artwork, content, size, pad, card_size, box_, user_plate, ShadowMode::Dock, source_facts, scratch, stroke);
        return;
    }

    // Step 2: the own background expands UNCHANGED to fill the target shape.
    if profile.kind == IconProfileKind::OwnBoard {
        if let Some(bg) = profile.background {
            diag.field_lane = Some(ComposeFieldLane::OwnBoard);
            compose_from_plate(artwork, content, size, pad, card_size, shape, bg, Some(box_), source_facts, shape_facts);
            return;
        }
    }

    // Steps 3-5: the plate is computed from the rim's colour and mean lightness.
    let seed = match opts.field_seed {
        Some(s) => Some(from_rgb_int(s)),
        None => profile.subject_rim_colour,
    };
    let plate = match seed {
        Some(seed) => themed_contrast_tone(seed, profile.subject_rim_lightness, band),
        None => neutral_contrast_tone(profile.subject_rim_lightness),
    };
    fill_region(content, size, pad, card_size, plate.r, plate.g, plate.b);

    if profile.transparent_edges {
        diag.field_lane = Some(ComposeFieldLane::DerivedBareShadow);
        // 自动分离: a derived plate melts essentially only on a BIMODAL rim (the
        // contrast-tone derivation already opposes the rim MEAN), and a bimodal rim
        // melts on BOTH lightness sides — so re-deriving the plate cannot help and
        // would break the desktop-wide shared plate-lightness line. Stroke instead.
        let stroke = separation_stroke_ink(config, artwork, plate);
        draw_bare_with_shadow(artwork, content, size, pad, card_size, box_, plate, ShadowMode::Dock, source_facts, scratch, stroke);
        return;
    }
    diag.field_lane = Some(ComposeFieldLane::DerivedPlate);
    compose_from_plate(artwork, content, size, pad, card_size, shape, plate, Some(box_), source_facts, shape_facts);
}

/// The artwork drawn ORIGINAL over a soft silhouette shadow (compose.ts
/// `drawBareWithShadow`). Reuses `scratch.shadow` instead of allocating fresh per call:
/// `layer` is ZEROED by `prepare` (its border pixels are never written by `draw_centred`
/// but ARE read below — `layer.data[i*4+3]` for the whole layer, then `composite_over`
/// — so they must read 0 exactly like the fresh `Raster::new`), while `alpha` + `tmp`
/// are fully overwritten before any read (only their length is restored). Byte-identical
/// to the former fresh-alloc version.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_bare_with_shadow(
    artwork: &Raster,
    content: &mut Raster,
    size: usize,
    pad: usize,
    card_size: usize,
    box_: usize,
    plate: Rgba,
    mode: ShadowMode,
    source_facts: Option<&SourceFacts>,
    scratch: &mut RenderScratch,
    stroke: Option<Rgba>,
) {
    let spec = shadow_spec(mode);
    let sc = &mut scratch.shadow;
    sc.prepare(size);
    draw_centred(artwork, content_bounds(source_facts, artwork), &mut sc.layer, size, pad, card_size, box_);

    for (i, a) in sc.alpha.iter_mut().enumerate() {
        *a = (sc.layer.data[i * 4 + 3] as f64 / 255.0) as f32;
    }
    let radius = 1.max(js_round(size as f64 * spec.blur_fraction) as usize);
    box_blur_in_place(&mut sc.alpha, &mut sc.tmp, size, size, radius);
    box_blur_in_place(&mut sc.alpha, &mut sc.tmp, size, size, radius);

    let shadow = field_shadow_tone(Rgba { a: 255, ..plate });
    let dy = if spec.offset_fraction == 0.0 {
        0
    } else {
        1.max(js_round(size as f64 * spec.offset_fraction) as usize)
    };
    for y in 0..size {
        if y < dy {
            continue;
        }
        let sy = y - dy;
        for x in 0..size {
            let a = sc.alpha[sy * size + x] as f64 * spec.alpha;
            if a <= 0.004 {
                continue;
            }
            over_at(
                &mut content.data,
                (y * size + x) * 4,
                shadow.r,
                shadow.g,
                shadow.b,
                clamp_u8_int(js_round(a * 255.0)),
            );
        }
    }
    // 自动分离 stroke rides between the soft shadow and the subject: crisp over the
    // blur, and the subject composites over the ring's inner AA edge.
    if let Some(ink) = stroke {
        draw_separation_stroke(content, &sc.layer, size, ink);
    }
    composite_over(content, &sc.layer);
}

/// Separable box blur on an **f32** coverage field with **f64** running sums
/// (compose.ts `boxBlurInPlace`), narrowing to f32 on every store.
fn box_blur_in_place(field: &mut [f32], tmp: &mut [f32], w: usize, h: usize, radius: usize) {
    let win = (radius * 2 + 1) as f64;
    let r = radius as isize;
    for y in 0..h {
        let mut acc = 0.0f64;
        let row = y * w;
        for x in -r..=r {
            acc += field[row + (x.max(0).min(w as isize - 1)) as usize] as f64;
        }
        for x in 0..w {
            tmp[row + x] = (acc / win) as f32;
            let out_x = x.saturating_sub(radius);
            let in_x = (x + radius + 1).min(w - 1);
            acc += field[row + in_x] as f64 - field[row + out_x] as f64;
        }
    }
    for x in 0..w {
        let mut acc = 0.0f64;
        for y in -r..=r {
            acc += tmp[(y.max(0).min(h as isize - 1)) as usize * w + x] as f64;
        }
        for y in 0..h {
            field[y * w + x] = (acc / win) as f32;
            let out_y = y.saturating_sub(radius);
            let in_y = (y + radius + 1).min(h - 1);
            acc += tmp[in_y * w + x] as f64 - tmp[out_y * w + x] as f64;
        }
    }
}

/// The segmented subject as its own layer (compose.ts `monoSubjectLayer`).
/// `flat_tint` recolours the subject to one flat colour; None keeps its pixels.
/// Returns None when segmentation is degenerate (< 2% of the canvas).
pub(crate) fn mono_subject_layer(artwork: &Raster, flat_tint: Option<u32>, source_facts: Option<&SourceFacts>) -> Option<Raster> {
    let seg = segmentation(source_facts, artwork);
    let mask = &seg.mask;
    // Second line behind the accessor's self-heal: the mask indexes `artwork` pixel-
    // for-pixel below, so a mask sized to a different (stale) raster would read OOB.
    debug_assert_eq!(mask.len(), artwork.width * artwork.height, "segmentation mask must match the current artwork");
    let solid: usize = mask.iter().map(|&v| v as usize).sum();
    if (solid as f64) < mask.len() as f64 * 0.02 {
        return None;
    }
    let mut layer = Raster::new(artwork.width, artwork.height);
    let src = &artwork.data;
    let dst = &mut layer.data;
    let (tr, tg, tb) = match flat_tint {
        None => (0u8, 0u8, 0u8),
        Some(t) => (((t >> 16) & 0xff) as u8, ((t >> 8) & 0xff) as u8, (t & 0xff) as u8),
    };
    for i in 0..mask.len() {
        if mask[i] == 0 {
            continue;
        }
        let i4 = i * 4;
        if src[i4 + 3] == 0 {
            continue;
        }
        if flat_tint.is_none() {
            dst[i4] = src[i4];
            dst[i4 + 1] = src[i4 + 1];
            dst[i4 + 2] = src[i4 + 2];
        } else {
            dst[i4] = tr;
            dst[i4 + 1] = tg;
            dst[i4 + 2] = tb;
        }
        dst[i4 + 3] = src[i4 + 3];
    }
    Some(layer)
}
