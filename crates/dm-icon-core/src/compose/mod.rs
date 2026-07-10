//! The tile composer — 1:1 port of the frozen `compose.ts`. ONE function renders
//! the on-screen preview AND the 256 bake master (same functions, two
//! resolutions). Absorbs the Spike-4 `slice.rs` helpers (no duplication). The
//! read-only diagnostics sink records WHICH lane executed for the stage-level
//! differential; pixels are untouched by it.

mod field;
mod helpers;

pub(crate) use helpers::*;

use crate::analysis::{
    bounds_h, bounds_w, find_content_bounds, has_transparent_edges, matches_shape, solid_bounds,
    try_detect_background, ContentBounds,
};
use crate::color::luminance;
use crate::config::{Config, Distinction, FilterStyle, IconShape, MonoStyle, PlateFallback, Subject};
use crate::filters::apply_filter;
use crate::marks::{draw_classic_arrow, resolve_mark, MarkContext, Placement};
use crate::mono::{mono_map_adaptive, mono_ramp, transform_pixel_in_place};
use crate::profile::IconProfile;
use crate::raster::{clip_to_mask, from_rgb_int, over_at, shape_mask, Raster, WHITE};
use crate::sampling::{draw_scaled, sample_bilinear};
use crate::segment::segment_subject;

/// Per-icon inputs resolved OUTSIDE the tile (compose.ts `RenderOpts`).
/// `field_seed` is the hue-spread-adjusted seed already parsed via `hexToInt`
/// (None/absent = derive from artwork). `kindBucket` is not consumed by the
/// pixel path and is omitted.
#[derive(Clone, Debug, Default)]
pub struct RenderOpts {
    pub field_seed: Option<u32>,
}

/// The top-level composition lane renderTile/composeTile executed (compose.ts).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ComposeLane {
    Original,
    #[default]
    Empty,
    DerivedField,
    LayeredMono,
    PassthroughNone,
    PassthroughMatch,
    PlateDetect,
    BareWhite,
    InscribeWhite,
    Stretch,
}

impl ComposeLane {
    pub fn as_str(self) -> &'static str {
        match self {
            ComposeLane::Original => "original",
            ComposeLane::Empty => "empty",
            ComposeLane::DerivedField => "derived-field",
            ComposeLane::LayeredMono => "layered-mono",
            ComposeLane::PassthroughNone => "passthrough-none",
            ComposeLane::PassthroughMatch => "passthrough-match",
            ComposeLane::PlateDetect => "plate-detect",
            ComposeLane::BareWhite => "bare-white",
            ComposeLane::InscribeWhite => "inscribe-white",
            ComposeLane::Stretch => "stretch",
        }
    }
}

/// The sub-branch inside composeField (only when lane == DerivedField).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComposeFieldLane {
    FullSquare,
    UserPlateBoard,
    UserPlateBare,
    OwnBoard,
    DerivedBareShadow,
    DerivedPlate,
}

impl ComposeFieldLane {
    pub fn as_str(self) -> &'static str {
        match self {
            ComposeFieldLane::FullSquare => "full-square",
            ComposeFieldLane::UserPlateBoard => "user-plate-board",
            ComposeFieldLane::UserPlateBare => "user-plate-bare",
            ComposeFieldLane::OwnBoard => "own-board",
            ComposeFieldLane::DerivedBareShadow => "derived-bare-shadow",
            ComposeFieldLane::DerivedPlate => "derived-plate",
        }
    }
}

/// Read-only lane diagnostics (compose.ts `ComposeDiagnostics`). Always present
/// internally (the corpus harness always requests it); pixels are unaffected.
#[derive(Clone, Debug, Default)]
pub struct ComposeDiagnostics {
    pub lane: ComposeLane,
    pub field_lane: Option<ComposeFieldLane>,
    pub pass_through: bool,
}

/// Render one styled tile at `size` px (compose.ts `renderTile`).
#[allow(clippy::too_many_arguments)]
pub fn render_tile(
    artwork: &Raster,
    config: &Config,
    is_shortcut: bool,
    show_original: bool,
    size: usize,
    opts: &RenderOpts,
    diag: &mut ComposeDiagnostics,
) -> Raster {
    render_tile_cached(artwork, config, is_shortcut, show_original, size, opts, diag, None)
}

/// `render_tile` with an optional RenderSession-cached profile (same raster →
/// identical to `iconProfile(artwork)`, so byte parity is preserved). The
/// derived-field lane consumes it, skipping the per-render re-analysis.
#[allow(clippy::too_many_arguments)]
pub fn render_tile_cached(
    artwork: &Raster,
    config: &Config,
    is_shortcut: bool,
    show_original: bool,
    size: usize,
    opts: &RenderOpts,
    diag: &mut ComposeDiagnostics,
    profile: Option<&IconProfile>,
) -> Raster {
    assert!(size > 0, "size must be positive");
    let tint = config.tint;

    if show_original {
        diag.lane = ComposeLane::Original;
        diag.pass_through = true;
        let mut original = build_original_card(artwork, size);
        if is_shortcut {
            draw_classic_arrow(&mut original, size);
        }
        return original;
    }

    let shape = config.shape;
    let tile_alpha = shape_mask(shape, size, size, 0.0, 0.0);
    let mark = if is_shortcut && config.distinction == Distinction::Mark {
        Some(resolve_mark(config.mark_style))
    } else {
        None
    };

    let geometry_ctx = MarkContext {
        size,
        shape,
        luminance: 0.5,
        mark_color: config.mark_color,
        tile_alpha: tile_alpha.clone(),
    };

    let pad = mark.map(|m| m.card_inset(&geometry_ctx)).unwrap_or(0);
    let card_size = size - 2 * pad;
    let mut card_mask = shape_mask(shape, size, card_size, pad as f64, pad as f64);
    let carves = mark.map(|m| m.carves_card()).unwrap_or(false);
    if carves {
        if let Some(m) = mark {
            m.carve_card(&mut card_mask, &geometry_ctx);
        }
    }

    let (mut tile, pass_through) =
        compose_tile(artwork, size, pad, card_size, shape, config, tint, opts, diag, profile);
    diag.pass_through = pass_through;

    if !pass_through || carves {
        clip_to_mask(&mut tile, &card_mask);
    }

    if config.filter != FilterStyle::None {
        apply_filter(&mut tile, size, config.filter, config.subject, tint);
    }

    let mark_alpha = if shape == IconShape::None {
        alpha_field_of(&tile)
    } else {
        tile_alpha
    };
    let ctx = MarkContext {
        size,
        shape,
        luminance: composed_luminance(&tile),
        mark_color: config.mark_color,
        tile_alpha: mark_alpha,
    };

    let mut target = Raster::new(size, size);
    if let Some(m) = mark {
        if m.placement() == Placement::Behind {
            m.render(&mut target, &card_mask, &ctx);
        }
    }
    composite_over(&mut target, &tile);
    if let Some(m) = mark {
        if m.placement() == Placement::Over {
            m.render(&mut target, &card_mask, &ctx);
        }
    }

    if is_shortcut && config.distinction == Distinction::Keep {
        draw_classic_arrow(&mut target, size);
    }
    target
}

/// The composed tile's own alpha as a coverage field (compose.ts `alphaFieldOf`).
fn alpha_field_of(tile: &Raster) -> Vec<f64> {
    let n = tile.width * tile.height;
    let mut field = vec![0.0f64; n];
    for (i, v) in field.iter_mut().enumerate() {
        *v = tile.data[i * 4 + 3] as f64 / 255.0;
    }
    field
}

/// 保留原样 / peek (compose.ts `buildOriginalCard`).
fn build_original_card(artwork: &Raster, size: usize) -> Raster {
    let mut card = Raster::new(size, size);
    for y in 0..size {
        for x in 0..size {
            let u = (x as f64 + 0.5) / size as f64;
            let v = (y as f64 + 0.5) / size as f64;
            let (r, g, b, a) = sample_bilinear(artwork, u, v);
            if a == 0 {
                continue;
            }
            let i4 = (y * size + x) * 4;
            card.data[i4] = r;
            card.data[i4 + 1] = g;
            card.data[i4 + 2] = b;
            card.data[i4 + 3] = a;
        }
    }
    card
}

/// TileRenderer.ComposeTile — shape intelligence + colour treatment.
#[allow(clippy::too_many_arguments)]
fn compose_tile(
    artwork: &Raster,
    size: usize,
    pad: usize,
    card_size: usize,
    shape: IconShape,
    config: &Config,
    tint: u32,
    opts: &RenderOpts,
    diag: &mut ComposeDiagnostics,
    profile: Option<&IconProfile>,
) -> (Raster, bool) {
    let mut content = Raster::new(size, size);

    let content_b = find_content_bounds(artwork);
    if solid_bounds(artwork).is_none() && bounds_w(content_b) <= 1 && bounds_h(content_b) <= 1 {
        diag.lane = ComposeLane::Empty;
        return (content, true);
    }

    // 派生底板 lane (ADR-0018): subject Original × plate 随图标(derived).
    if config.subject == Subject::Original
        && config.plate_fallback != PlateFallback::White
        && shape != IconShape::None
    {
        diag.lane = ComposeLane::DerivedField;
        field::compose_field(artwork, &mut content, size, pad, card_size, shape, config, opts, diag, profile);
        return (content, false);
    }

    let mut pass_through = false;
    let plate_override = config.plate_color.map(from_rgb_int);
    let plate = if config.subject == Subject::Original { plate_override } else { None };

    // LAYERED Mono (极致单色 + custom plate).
    if config.subject == Subject::Mono
        && shape != IconShape::None
        && (config.mono_style == MonoStyle::Flat || plate_override.is_some())
    {
        let flat = if config.mono_style == MonoStyle::Flat { Some(tint) } else { None };
        if let Some(mut layer) = field::mono_subject_layer(artwork, flat) {
            diag.lane = ComposeLane::LayeredMono;
            if config.mono_style != MonoStyle::Flat {
                mono_map_adaptive(&mut layer, tint);
            }
            let fill = plate_override.unwrap_or_else(|| mono_ramp(1.0, tint));
            fill_region(&mut content, size, pad, card_size, fill.r, fill.g, fill.b);
            let lb = find_content_bounds(&layer);
            draw_centred(&layer, lb, &mut content, size, pad, card_size, content_box(shape, card_size));
            return (content, false);
        }
    }

    if shape == IconShape::None {
        diag.lane = ComposeLane::PassthroughNone;
        let free = find_content_bounds(artwork);
        let (fw, fh) = fit(bounds_w(free), bounds_h(free), card_size);
        draw_scaled(
            artwork, free, &mut content, size,
            pad as i32 + (card_size as i32 - fw as i32) / 2,
            pad as i32 + (card_size as i32 - fh as i32) / 2,
            fw, fh,
        );
        pass_through = true;
    } else if matches_shape(artwork, shape) && solid_bounds(artwork).is_some() {
        diag.lane = ComposeLane::PassthroughMatch;
        let solid = solid_bounds(artwork).unwrap();
        let (w, h) = fit(bounds_w(solid), bounds_h(solid), card_size);
        draw_scaled(
            artwork, solid, &mut content, size,
            pad as i32 + (card_size as i32 - w as i32) / 2,
            pad as i32 + (card_size as i32 - h as i32) / 2,
            w, h,
        );
        pass_through = true;
    } else {
        let bg = try_detect_background(artwork).or_else(|| segment_subject(artwork).field);
        if let Some(bg) = bg {
            diag.lane = ComposeLane::PlateDetect;
            compose_from_plate(artwork, &mut content, size, pad, card_size, shape, plate.unwrap_or(bg), None);
        } else if has_transparent_edges(artwork) {
            diag.lane = ComposeLane::BareWhite;
            let fill = plate.unwrap_or(WHITE);
            fill_region(&mut content, size, pad, card_size, fill.r, fill.g, fill.b);
            draw_centred(artwork, find_content_bounds(artwork), &mut content, size, pad, card_size, content_box(shape, card_size));
        } else if inscribe_shapes(shape) {
            diag.lane = ComposeLane::InscribeWhite;
            let fill = plate.unwrap_or(WHITE);
            fill_region(&mut content, size, pad, card_size, fill.r, fill.g, fill.b);
            inscribe_content(artwork, &mut content, size, pad, card_size, shape);
        } else {
            diag.lane = ComposeLane::Stretch;
            let full = ContentBounds { left: 0, top: 0, right: artwork.width, bottom: artwork.height };
            draw_scaled(artwork, full, &mut content, size, pad as i32, pad as i32, card_size, card_size);
        }
    }

    if config.subject == Subject::Mono {
        mono_map_adaptive(&mut content, tint);
        return (content, pass_through);
    }
    if config.subject == Subject::BlackWhite {
        let d = &mut content.data;
        let mut i = 0;
        while i < d.len() {
            if d[i + 3] > 0 {
                transform_pixel_in_place(d, i, Subject::BlackWhite, tint);
            }
            i += 4;
        }
    }
    (content, pass_through)
}

pub(crate) fn composite_over(target: &mut Raster, over: &Raster) {
    let od = &over.data;
    let mut i4 = 0;
    while i4 < od.len() {
        if od[i4 + 3] > 0 {
            over_at(&mut target.data, i4, od[i4], od[i4 + 1], od[i4 + 2], od[i4 + 3]);
        }
        i4 += 4;
    }
}

fn composed_luminance(tile: &Raster) -> f64 {
    let d = &tile.data;
    let mut sum = 0.0f64;
    let mut weight = 0.0f64;
    let mut i4 = 0;
    while i4 < d.len() {
        let a = d[i4 + 3];
        if a != 0 {
            sum += luminance(d[i4], d[i4 + 1], d[i4 + 2]) * a as f64;
            weight += a as f64;
        }
        i4 += 4;
    }
    if weight <= 0.0 {
        0.5
    } else {
        sum / weight
    }
}

/// Spike-4 compatibility slice (Circle + white plate + subject + dock shadow),
/// now built from the REAL compose helpers — the dm-icon-wasm + xtask spike4
/// gate keeps running through the ported pipeline (no duplicate implementation).
pub fn render_slice_tile(artwork: &Raster, size: usize) -> Raster {
    assert!(size > 0, "size must be positive");
    let card_size = size;
    let mut tile = Raster::new(size, size);
    let box_size = field_content_box(IconShape::Circle, card_size);
    fill_region(&mut tile, size, 0, card_size, WHITE.r, WHITE.g, WHITE.b);
    field::draw_bare_with_shadow(artwork, &mut tile, size, 0, card_size, box_size, WHITE, field::ShadowMode::Dock);
    let card_mask = shape_mask(IconShape::Circle, size, card_size, 0.0, 0.0);
    clip_to_mask(&mut tile, &card_mask);
    let mut target = Raster::new(size, size);
    composite_over(&mut target, &tile);
    target
}
