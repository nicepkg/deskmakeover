//! 快捷方式标识 — 1:1 port of the frozen `marks.ts`. Marks are stateless; the
//! tile composer owns the z-order (behind siblings, over overlays). Trait +
//! shared helpers + the arrow glyph live here; the seven styles in `styles.rs`.

mod styles;

use std::cell::RefCell;

use crate::analysis::ContentBounds;
use crate::config::{IconShape, MarkStyle};
use crate::filters::chamfer_distance;
use crate::js_math::{clamp_u8_int, js_round, js_trunc};
use crate::raster::{
    dist_to_segment, from_rgb_int, in_triangle, paint, shape_mask, smooth_step01, Raster, Rgba,
};
use crate::sampling::draw_scaled;

/// 品牌珊瑚 — accent used when the user has not chosen a mark colour.
pub const MARK_ACCENT: u32 = 0xff6f5e;
/// Mark adaptivity crossover (distinct from the 0.66 ink threshold).
pub const ADAPTIVE_THRESHOLD: f64 = 0.58;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Behind,
    Over,
}

pub struct MarkContext {
    pub size: usize,
    pub shape: IconShape,
    pub luminance: f64,
    pub mark_color: Option<u32>,
    pub tile_alpha: Vec<f64>,
}

/// A shortcut mark (marks.ts `Mark`). Defaults mirror the TS `base`.
pub trait Mark {
    fn placement(&self) -> Placement;
    fn card_inset(&self, _ctx: &MarkContext) -> usize {
        0
    }
    fn carves_card(&self) -> bool {
        false
    }
    fn carve_card(&self, _card_mask: &mut [f64], _ctx: &MarkContext) {}
    fn render(&self, target: &mut Raster, card_mask: &[f64], ctx: &MarkContext);
}

pub(crate) fn is_light_tile(ctx: &MarkContext) -> bool {
    ctx.luminance > ADAPTIVE_THRESHOLD
}

pub(crate) fn mark_rgb(ctx: &MarkContext) -> Rgba {
    from_rgb_int(ctx.mark_color.unwrap_or(MARK_ACCENT))
}

/// A scaled/offset stamp of the mark geometry (marks.ts `stampMask`).
pub(crate) fn stamp_mask(ctx: &MarkContext, mask_size: usize, off_x: f64, off_y: f64) -> Vec<f64> {
    if ctx.shape != IconShape::None {
        return shape_mask(ctx.shape, ctx.size, mask_size, off_x, off_y);
    }
    let size = ctx.size;
    let mut out = vec![0.0f64; size * size];
    let scale = size as f64 / mask_size as f64;
    for y in 0..size {
        for x in 0..size {
            let sx = js_round((x as f64 - off_x) * scale) as i64;
            let sy = js_round((y as f64 - off_y) * scale) as i64;
            if sx >= 0 && sy >= 0 && sx < size as i64 && sy < size as i64 {
                out[y * size + x] = ctx.tile_alpha[sy as usize * size + sx as usize];
            }
        }
    }
    out
}

/// Outside-distance (px) from a coverage field's silhouette (marks.ts `outsideDistance`).
pub(crate) fn outside_distance(field: &[f64], size: usize) -> Vec<f64> {
    let mut probe = Raster::new(size, size);
    for (i, &v) in field.iter().enumerate() {
        probe.data[i * 4 + 3] = if v >= 0.5 { 255 } else { 0 };
    }
    chamfer_distance(&probe, size, false)
}

pub(crate) fn lerp_rgba(a: Rgba, b: Rgba, t: f64) -> Rgba {
    Rgba {
        r: clamp_u8_int(js_round(a.r as f64 + (b.r as f64 - a.r as f64) * t)),
        g: clamp_u8_int(js_round(a.g as f64 + (b.g as f64 - a.g as f64) * t)),
        b: clamp_u8_int(js_round(a.b as f64 + (b.b as f64 - a.b as f64) * t)),
        a: clamp_u8_int(js_round(a.a as f64 + (b.a as f64 - a.a as f64) * t)),
    }
}

/// Straight-alpha "over" for two colours (marks.ts `overRgba`, Glass frost).
pub(crate) fn over_rgba(top: Rgba, bottom: Rgba) -> Rgba {
    if top.a == 0 {
        return bottom;
    }
    if top.a == 255 {
        return top;
    }
    let ta = top.a as f64 / 255.0;
    let ba = bottom.a as f64 / 255.0;
    let out_a = ta + ba * (1.0 - ta);
    if out_a <= 0.0 {
        return Rgba { r: 0, g: 0, b: 0, a: 0 };
    }
    let inv = 1.0 / out_a;
    Rgba {
        r: clamp_u8_int(js_round((top.r as f64 * ta + bottom.r as f64 * ba * (1.0 - ta)) * inv)),
        g: clamp_u8_int(js_round((top.g as f64 * ta + bottom.g as f64 * ba * (1.0 - ta)) * inv)),
        b: clamp_u8_int(js_round((top.b as f64 * ta + bottom.b as f64 * ba * (1.0 - ta)) * inv)),
        a: clamp_u8_int(js_round(out_a * 255.0)),
    }
}

// ---- ArrowGlyph + classic arrow ----

/// The one NE "↗" arrow glyph (marks.ts `drawArrowGlyph`).
#[allow(clippy::too_many_arguments)]
pub fn draw_arrow_glyph(
    target: &mut Raster,
    size: usize,
    cx: f64,
    cy: f64,
    reach: f64,
    ink: Rgba,
    clip: &[f64],
) {
    if reach <= 0.0 {
        return;
    }
    let (tail_u, tail_v) = (-0.44, 0.44);
    let (neck_u, neck_v) = (0.12, -0.12);
    let (tip_u, tip_v) = (0.48, -0.48);
    let shaft_half = 0.135;
    let head_half = 0.28;
    let perp = 0.70710678;
    let head_ax = neck_u + perp * head_half;
    let head_ay = neck_v + perp * head_half;
    let head_bx = neck_u - perp * head_half;
    let head_by = neck_v - perp * head_half;
    let soft = 1.3 / reach;
    let r = libm::ceil(reach) + 2.0;

    let mut y = js_trunc(cy - r) as i64;
    while (y as f64) <= cy + r {
        if y >= 0 && (y as usize) < size {
            let mut x = js_trunc(cx - r) as i64;
            while (x as f64) <= cx + r {
                if x >= 0 && (x as usize) < size {
                    let i = y as usize * size + x as usize;
                    let clip_cov = clip[i];
                    if clip_cov > 0.0 {
                        let u = (x as f64 + 0.5 - cx) / reach;
                        let v = (y as f64 + 0.5 - cy) / reach;
                        let d_shaft = dist_to_segment(u, v, tail_u, tail_v, neck_u, neck_v);
                        let cov = if in_triangle(u, v, tip_u, tip_v, head_ax, head_ay, head_bx, head_by) {
                            1.0
                        } else {
                            smooth_step01((shaft_half - d_shaft) / soft)
                        };
                        paint(target, i, ink, cov * clip_cov);
                    }
                }
                x += 1;
            }
        }
        y += 1;
    }
}

const ARROW_PLATE: Rgba = Rgba { r: 244, g: 244, b: 241, a: 245 };
const ARROW_GLYPH: Rgba = Rgba { r: 46, g: 50, b: 56, a: 255 };

thread_local! {
    // The GENUINE Win11 shortcut-arrow badge (owner-extracted). Mirrors the TS
    // module global `nativeArrow`; the runner sets it once at boot.
    static NATIVE_ARROW: RefCell<Option<Raster>> = const { RefCell::new(None) };
}

/// marks.ts `setNativeArrowRaster`.
pub fn set_native_arrow_raster(raster: Option<Raster>) {
    NATIVE_ARROW.with(|a| *a.borrow_mut() = raster);
}

fn native_arrow() -> Option<Raster> {
    NATIVE_ARROW.with(|a| a.borrow().clone())
}

/// 经典箭头 — the real system badge when available, else the drawn fallback
/// (marks.ts `drawClassicArrow`).
pub fn draw_classic_arrow(target: &mut Raster, size: usize) {
    if let Some(arrow) = native_arrow() {
        let box_size = 14.max(js_round(size as f64 * 0.28) as usize);
        draw_scaled(
            &arrow,
            ContentBounds { left: 0, top: 0, right: arrow.width, bottom: arrow.height },
            target,
            size,
            1,
            size as i32 - 1 - box_size as i32,
            box_size,
            box_size,
        );
        return;
    }
    let asz = 14.0_f64.max(size as f64 * 0.28);
    let left = 1.0;
    let top = size as f64 - 1.0 - asz;
    let radius = 4.0_f64.min(asz / 2.0);

    let mut plate = vec![0.0f64; size * size];
    let mut y = js_trunc(top - 1.0) as i64;
    while (y as f64) <= top + asz + 1.0 {
        if y >= 0 && (y as usize) < size {
            let mut x = js_trunc(left - 1.0) as i64;
            while (x as f64) <= left + asz + 1.0 {
                if x >= 0 && (x as usize) < size {
                    let cov = rounded_rect_coverage(x as f64 + 0.5, y as f64 + 0.5, left, top, asz, radius);
                    if cov > 0.0 {
                        let i = y as usize * size + x as usize;
                        plate[i] = cov;
                        paint(target, i, ARROW_PLATE, cov);
                    }
                }
                x += 1;
            }
        }
        y += 1;
    }
    draw_arrow_glyph(target, size, left + asz / 2.0, top + asz / 2.0, asz * 0.34, ARROW_GLYPH, &plate);
}

fn rounded_rect_coverage(px: f64, py: f64, rx: f64, ry: f64, side: f64, radius: f64) -> f64 {
    let cx = rx + side / 2.0;
    let cy = ry + side / 2.0;
    let half = side / 2.0 - radius;
    let qx = ((px - cx).abs() - half).max(0.0);
    let qy = ((py - cy).abs() - half).max(0.0);
    let d = libm::sqrt(qx * qx + qy * qy) - radius;
    (0.5 - d).clamp(0.0, 1.0)
}

/// marks.ts `resolveMark`.
pub fn resolve_mark(style: MarkStyle) -> &'static dyn Mark {
    match style {
        MarkStyle::Glass => &styles::GLASS_MARK,
        MarkStyle::Shadow => &styles::SHADOW_MARK,
        MarkStyle::Halo => &styles::HALO_MARK,
        MarkStyle::Satin => &styles::SATIN_MARK,
        MarkStyle::Arc => &styles::ARC_MARK,
        MarkStyle::Fold => &styles::FOLD_MARK,
        MarkStyle::Ring => &styles::RING_MARK,
    }
}
