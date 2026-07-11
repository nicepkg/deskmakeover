//! Shared compose geometry + plate-composition helpers (compose.ts), split from
//! `mod.rs` to hold the 500-line cap. Pure layout/draw primitives consumed by
//! both `compose_tile` and the Field lane.

use crate::analysis::{bounds_h, bounds_w, color_distance, find_content_bounds, max_scale_auto, ContentBounds};
use crate::source_facts::{content_bounds, detected_background, foreground, SourceFacts};
use crate::config::IconShape;
use crate::js_math::js_round;
use crate::raster::{Raster, Rgba};
use crate::sampling::draw_scaled;
use crate::shapes::shape_contains;

// Reference StyleBitmap default: the logo occupies the centre ~67% of the tile.
const CONTENT_PADDING_FRACTION: f64 = 42.0 / 256.0;
pub(crate) const FULL_BLEED_FOREGROUND_FRACTION: f64 = 0.82;
const FIELD_CONTENT_PADDING_FRACTION: f64 = 36.0 / 256.0;
const BG_SWAP_TOLERANCE: i32 = 48;
const BG_SWAP_MIN_SHIFT: i32 = 12;

pub(crate) fn inscribe_shapes(shape: IconShape) -> bool {
    matches!(
        shape,
        IconShape::Circle | IconShape::Diamond | IconShape::Flower | IconShape::Pebble
    )
}

fn inscribe_margin(shape: IconShape) -> f64 {
    match shape {
        IconShape::Circle => 0.94,
        IconShape::Pebble => 0.88,
        IconShape::Diamond => 0.82,
        IconShape::Flower => 0.82,
        _ => 0.94,
    }
}

fn max_centred_square_factor(shape: IconShape) -> f64 {
    let mut lo = 0.0f64;
    let mut hi = 1.0f64;
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        let pts = [
            (1.0 - mid, 1.0 - mid),
            (1.0 + mid, 1.0 - mid),
            (1.0 - mid, 1.0 + mid),
            (1.0 + mid, 1.0 + mid),
            (1.0, 1.0 - mid),
            (1.0, 1.0 + mid),
            (1.0 - mid, 1.0),
            (1.0 + mid, 1.0),
        ];
        if pts.iter().all(|&(x, y)| shape_contains(shape, x, y, 2.0)) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

fn inner_box(card_size: usize) -> usize {
    (8_isize).max(card_size as isize - 2 * (js_round(card_size as f64 * CONTENT_PADDING_FRACTION) as isize))
        as usize
}

pub(crate) fn content_box(shape: IconShape, card_size: usize) -> usize {
    let inner = inner_box(card_size);
    if !inscribe_shapes(shape) {
        return inner;
    }
    inner.min(8.max(js_round(card_size as f64 * max_centred_square_factor(shape) * inscribe_margin(shape)) as usize))
}

pub(crate) fn field_content_box(shape: IconShape, card_size: usize) -> usize {
    let inner = (8_isize)
        .max(card_size as isize - 2 * (js_round(card_size as f64 * FIELD_CONTENT_PADDING_FRACTION) as isize))
        as usize;
    if !inscribe_shapes(shape) {
        return inner;
    }
    inner.min(8.max(js_round(card_size as f64 * max_centred_square_factor(shape) * inscribe_margin(shape)) as usize))
}

/// Rebuild a plated icon in the target shape (compose.ts `composeFromPlate`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn compose_from_plate(
    artwork: &Raster,
    content: &mut Raster,
    size: usize,
    pad: usize,
    card_size: usize,
    shape: IconShape,
    bg: Rgba,
    box_cap: Option<usize>,
    source_facts: Option<&SourceFacts>,
) {
    fill_region(content, size, pad, card_size, bg.r, bg.g, bg.b);
    let plate = content_bounds(source_facts, artwork);
    let plate_min = 1.max(bounds_w(plate).min(bounds_h(plate)));
    let fg = foreground(source_facts, artwork);

    let own = detected_background(source_facts, artwork);
    let swapped = match own {
        Some(own) if color_distance(own, Rgba { a: 255, ..bg }) > BG_SWAP_MIN_SHIFT => {
            Some(backdrop_swapped(artwork, own, bg))
        }
        _ => None,
    };
    let source: &Raster = swapped.as_ref().unwrap_or(artwork);

    if let Some(fg) = fg {
        let fg_max = bounds_w(fg).max(bounds_h(fg));
        if (fg_max as f64) <= plate_min as f64 * FULL_BLEED_FOREGROUND_FRACTION {
            let fraction = fg_max as f64 / plate_min as f64;
            let cap = box_cap.unwrap_or_else(|| content_box(shape, card_size));
            let box_ = (js_round(card_size as f64 * fraction) as usize).min(cap);
            draw_centred(source, fg, content, size, pad, card_size, 8.max(box_));
            return;
        }
    }
    if inscribe_shapes(shape) {
        inscribe_content(source, content, size, pad, card_size, shape);
        return;
    }
    let cap = box_cap.unwrap_or_else(|| content_box(shape, card_size));
    draw_centred(source, content_bounds(source_facts, artwork), content, size, pad, card_size, cap);
}

/// The artwork with backdrop pixels swapped to the new plate colour
/// (compose.ts `backdropSwapped`; cache is a caller concern — pure here).
fn backdrop_swapped(artwork: &Raster, own: Rgba, plate: Rgba) -> Raster {
    let mut out = artwork.clone();
    let d = &mut out.data;
    let mut i4 = 0;
    while i4 < d.len() {
        if d[i4 + 3] > 24 {
            let dist = (d[i4] as i32 - own.r as i32).abs()
                + (d[i4 + 1] as i32 - own.g as i32).abs()
                + (d[i4 + 2] as i32 - own.b as i32).abs();
            if dist <= BG_SWAP_TOLERANCE {
                d[i4] = plate.r;
                d[i4 + 1] = plate.g;
                d[i4 + 2] = plate.b;
            }
        }
        i4 += 4;
    }
    out
}

pub(crate) fn inscribe_content(
    artwork: &Raster,
    content: &mut Raster,
    size: usize,
    pad: usize,
    card_size: usize,
    shape: IconShape,
) {
    let bounds = find_content_bounds(artwork);
    let scale = max_scale_auto(artwork, shape) * inscribe_margin(shape);
    let box_ = 8.max(js_round(card_size as f64 * scale) as usize);
    draw_centred(artwork, bounds, content, size, pad, card_size, box_);
}

pub(crate) fn draw_centred(
    artwork: &Raster,
    bounds: ContentBounds,
    content: &mut Raster,
    size: usize,
    pad: usize,
    card_size: usize,
    box_: usize,
) {
    let (w, h) = fit(1.max(bounds_w(bounds)), 1.max(bounds_h(bounds)), box_);
    draw_scaled(
        artwork, bounds, content, size,
        pad as i32 + (card_size as i32 - w as i32) / 2,
        pad as i32 + (card_size as i32 - h as i32) / 2,
        w, h,
    );
}

pub(crate) fn fill_region(
    content: &mut Raster,
    size: usize,
    pad: usize,
    card_size: usize,
    r: u8,
    g: u8,
    b: u8,
) {
    let end = size.min(pad + card_size);
    for y in pad..end {
        for x in pad..end {
            let i4 = (y * size + x) * 4;
            content.data[i4] = r;
            content.data[i4 + 1] = g;
            content.data[i4 + 2] = b;
            content.data[i4 + 3] = 255;
        }
    }
}

pub(crate) fn fit(w: usize, h: usize, max: usize) -> (usize, usize) {
    let scale = (max as f64 / w as f64).min(max as f64 / h as f64);
    (
        (js_round(w as f64 * scale) as usize).max(1),
        (js_round(h as f64 * scale) as usize).max(1),
    )
}
