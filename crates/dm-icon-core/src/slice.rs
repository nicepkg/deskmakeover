//! Spike-4 slice composition (ADR-0019 M1 gate): "Circle shape + fixed white
//! plate + subject blit + silhouette dock shadow" — a 1:1 port of the frozen
//! compose.ts internals the slice touches (line refs inline). This module
//! GROWS INTO the full `compose` port at M5; the helpers here are the real
//! ones (fill_region, fit, draw_centred, box_blur_in_place,
//! draw_bare_with_shadow, composite_over), not spike throwaways.
//!
//! Precision law: the silhouette-shadow blur is **f32 storage with f64
//! accumulation** (TS `Float32Array` + JS-number arithmetic) — distinct from
//! raster.rs `box_blur`, which is all-f64 (TS `Float64Array`).

use crate::analysis::{bounds_h, bounds_w, find_content_bounds, ContentBounds};
use crate::color::field_shadow_tone;
use crate::js_math::{clamp_u8_int, js_round};
use crate::raster::{clip_to_mask, over_at, shape_mask, Raster, Rgba, WHITE};
use crate::sampling::draw_scaled;
use crate::shapes::{shape_contains, IconShape};

// compose.ts:33
const FIELD_CONTENT_PADDING_FRACTION: f64 = 36.0 / 256.0;
// compose.ts:90-95 (Circle entry — the slice is Circle-fixed)
const INSCRIBE_MARGIN_CIRCLE: f64 = 0.94;

/// compose.ts:478-483 — `SHADOW_MODES.dock`.
struct ShadowSpec {
    alpha: f64,
    blur_fraction: f64,
    offset_fraction: f64,
}

const SHADOW_DOCK: ShadowSpec =
    ShadowSpec { alpha: 0.24, blur_fraction: 0.04, offset_fraction: 0.015 };

/// compose.ts:105-125 — largest centred axis-aligned square inside the shape
/// (24-step bisection over corner + edge-midpoint membership).
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

/// compose.ts:381-385 — the Field-lane content keyline (Circle inscribes).
fn field_content_box(shape: IconShape, card_size: usize) -> usize {
    let card = card_size as f64;
    let inner =
        (card_size as isize - 2 * js_round(card * FIELD_CONTENT_PADDING_FRACTION) as isize).max(8);
    let inscribed =
        (js_round(card * max_centred_square_factor(shape) * INSCRIBE_MARGIN_CIRCLE) as isize)
            .max(8);
    inner.min(inscribed) as usize
}

/// compose.ts:713-724.
fn fill_region(content: &mut Raster, size: usize, pad: usize, card_size: usize, r: u8, g: u8, b: u8) {
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

/// compose.ts:726-729.
fn fit(w: usize, h: usize, max: usize) -> (usize, usize) {
    let scale = (max as f64 / w as f64).min(max as f64 / h as f64);
    (
        (js_round(w as f64 * scale) as usize).max(1),
        (js_round(h as f64 * scale) as usize).max(1),
    )
}

/// compose.ts:702-707.
fn draw_centred(
    artwork: &Raster,
    bounds: ContentBounds,
    content: &mut Raster,
    size: usize,
    pad: usize,
    card_size: usize,
    box_size: usize,
) {
    let (w, h) = fit(bounds_w(bounds).max(1), bounds_h(bounds).max(1), box_size);
    draw_scaled(
        artwork,
        bounds,
        content,
        size,
        pad as i32 + ((card_size - w) / 2) as i32, // Math.trunc on a non-negative int division
        pad as i32 + ((card_size - h) / 2) as i32,
        w,
        h,
    );
}

/// compose.ts:544-567 — separable box blur on an **f32** coverage field with
/// **f64** running sums (JS numbers), narrowing to f32 on every store exactly
/// where the TS assigns into the `Float32Array`.
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

/// compose.ts:507-540 — the artwork drawn ORIGINAL over a soft silhouette
/// shadow (dock spec). Scratch is allocated fresh: the frozen per-size scratch
/// is zeroed/fully overwritten before every read, so reuse is parity-neutral.
fn draw_bare_with_shadow(
    artwork: &Raster,
    content: &mut Raster,
    size: usize,
    pad: usize,
    card_size: usize,
    box_size: usize,
    plate: Rgba,
) {
    let spec = &SHADOW_DOCK;
    let mut layer = Raster::new(size, size);
    let mut alpha = vec![0.0f32; size * size];
    let mut tmp = vec![0.0f32; size * size];
    draw_centred(artwork, find_content_bounds(artwork), &mut layer, size, pad, card_size, box_size);

    for (i, a) in alpha.iter_mut().enumerate() {
        *a = (layer.data[i * 4 + 3] as f64 / 255.0) as f32;
    }
    let radius = (js_round(size as f64 * spec.blur_fraction) as usize).max(1);
    box_blur_in_place(&mut alpha, &mut tmp, size, size, radius);
    box_blur_in_place(&mut alpha, &mut tmp, size, size, radius);

    let shadow = field_shadow_tone(Rgba { a: 255, ..plate });
    let dy = if spec.offset_fraction == 0.0 {
        0
    } else {
        (js_round(size as f64 * spec.offset_fraction) as usize).max(1)
    };
    for y in 0..size {
        if y < dy {
            continue; // sy = y - dy < 0
        }
        let sy = y - dy;
        for x in 0..size {
            let a = alpha[sy * size + x] as f64 * spec.alpha;
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
    composite_over(content, &layer);
}

/// compose.ts:749-755.
fn composite_over(target: &mut Raster, over: &Raster) {
    let od = &over.data;
    for i4 in (0..od.len()).step_by(4) {
        if od[i4 + 3] > 0 {
            over_at(&mut target.data, i4, od[i4], od[i4 + 1], od[i4 + 2], od[i4 + 3]);
        }
    }
}

/// The Spike-4 slice tile: white plate + subject blit + dock silhouette
/// shadow, clipped to the Circle mask, then composited onto a fresh target
/// exactly like renderTile's tail (compose.ts:184-223 with mark/filter/arrow
/// branches inert). Mirrors scripts/spike4-slice.ts `renderSliceTile`.
pub fn render_slice_tile(artwork: &Raster, size: usize) -> Raster {
    assert!(size > 0, "size must be positive");
    let pad = 0usize; // no mark → cardInset 0
    let card_size = size;
    let mut tile = Raster::new(size, size);
    let box_size = field_content_box(IconShape::Circle, card_size);
    fill_region(&mut tile, size, pad, card_size, WHITE.r, WHITE.g, WHITE.b);
    draw_bare_with_shadow(artwork, &mut tile, size, pad, card_size, box_size, WHITE);
    let card_mask = shape_mask(IconShape::Circle, size, card_size, pad as i32, pad as i32);
    clip_to_mask(&mut tile, &card_mask); // compose.ts applyCoverage — same body
    let mut target = Raster::new(size, size);
    composite_over(&mut target, &tile);
    target
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 256² source: opaque mid-gray square in the centre, transparent rim.
    fn synthetic_source() -> Raster {
        let mut r = Raster::new(256, 256);
        for y in 64..192 {
            for x in 64..192 {
                let i4 = (y * 256 + x) * 4;
                r.data[i4] = 120;
                r.data[i4 + 1] = 130;
                r.data[i4 + 2] = 140;
                r.data[i4 + 3] = 255;
            }
        }
        r
    }

    #[test]
    fn circle_inscribed_square_is_root_half() {
        let f = max_centred_square_factor(IconShape::Circle);
        assert!((f - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-6, "got {f}");
    }

    #[test]
    fn field_content_box_matches_ts_values() {
        // TS oracle (scripts/spike4-slice.ts): 170 @256, 340 @512.
        assert_eq!(field_content_box(IconShape::Circle, 256), 170);
        assert_eq!(field_content_box(IconShape::Circle, 512), 340);
    }

    #[test]
    fn slice_tile_invariants() {
        let src = synthetic_source();
        let tile = render_slice_tile(&src, 256);
        // corners clipped by the circle
        assert_eq!(&tile.data[0..4], &[0, 0, 0, 0]);
        // plate is white near the top mid (inside circle, above subject+shadow)
        let i4 = (8 * 256 + 128) * 4;
        assert_eq!(&tile.data[i4..i4 + 3], &[255, 255, 255]);
        // subject pixels survive untouched in the centre
        let c4 = (128 * 256 + 128) * 4;
        assert_eq!(&tile.data[c4..c4 + 4], &[120, 130, 140, 255]);
        // a shadow pixel below the subject is darker than the plate
        let subject_half = ((128.0 * (170.0 / 128.0)) / 2.0) as usize; // ≈ box/2
        let y = 128 + subject_half + 6;
        let s4 = (y * 256 + 128) * 4;
        assert!(tile.data[s4] < 255, "expected shadow at y={y}, got {:?}", &tile.data[s4..s4 + 4]);
        // deterministic: render twice → identical bytes
        assert_eq!(tile.data, render_slice_tile(&src, 256).data);
    }

    #[test]
    fn blur_preserves_mass_on_constant_field() {
        let mut field = vec![0.5f32; 32 * 32];
        let mut tmp = vec![0.0f32; 32 * 32];
        box_blur_in_place(&mut field, &mut tmp, 32, 32, 3);
        assert!(field.iter().all(|&v| (v - 0.5).abs() < 1e-6));
    }
}
