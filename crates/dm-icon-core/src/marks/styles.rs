//! The seven shortcut-mark styles — 1:1 port of the frozen `marks.ts` mark
//! objects. All `x ** 2` are ported as explicit `d*d`: the correctly-rounded
//! `pow(d, 2)` equals the IEEE product, so this dodges both pow paths (and the
//! JSC-vs-libm 2.4-exponent LUT drift is irrelevant to squaring).

use super::{
    draw_arrow_glyph, is_light_tile, lerp_rgba, mark_rgb, outside_distance, over_rgba, stamp_mask,
    Mark, MarkContext, Placement, ADAPTIVE_THRESHOLD,
};
use crate::config::IconShape;
use crate::js_math::{clamp_u8_int, js_round, js_trunc};
use crate::raster::{
    backdrop_blur, box_blur, fade, from_rgb_int, mix, paint, rgba_of, shift, smooth_step01, Raster,
    Rgba, WHITE,
};

// ---- 投影 ShadowMark ----

pub(crate) struct ShadowMark;
pub(crate) static SHADOW_MARK: ShadowMark = ShadowMark;

impl Mark for ShadowMark {
    fn placement(&self) -> Placement {
        Placement::Behind
    }
    fn card_inset(&self, ctx: &MarkContext) -> usize {
        1.max(js_round(ctx.size as f64 * 0.06) as usize)
    }
    fn render(&self, target: &mut Raster, _card_mask: &[f64], ctx: &MarkContext) {
        let size = ctx.size;
        let pad = self.card_inset(ctx);
        let card_size = size - 2 * pad;
        let sil = stamp_mask(
            ctx,
            card_size,
            pad as f64 + (size as f64 * 0.05).max(1.0),
            pad as f64 + (size as f64 * 0.06).max(1.0),
        );
        let soft = box_blur(&sil, size, 1.max(js_trunc(size as f64 * 0.028) as i64) as i32);
        let ink = Rgba { r: 8, g: 10, b: 14, a: 255 };
        for (i, &s) in soft.iter().enumerate() {
            paint(target, i, ink, s * 0.44);
        }
    }
}

// ---- 光环 HaloMark ----

pub(crate) struct HaloMark;
pub(crate) static HALO_MARK: HaloMark = HaloMark;

impl Mark for HaloMark {
    fn placement(&self) -> Placement {
        Placement::Behind
    }
    fn card_inset(&self, ctx: &MarkContext) -> usize {
        1.max(js_round(ctx.size as f64 * 0.07) as usize)
    }
    fn render(&self, target: &mut Raster, card_mask: &[f64], ctx: &MarkContext) {
        let size = ctx.size;
        let sil: &[f64] = if ctx.shape == IconShape::None { &ctx.tile_alpha[..] } else { card_mask };
        let dist = outside_distance(sil, size);
        let radius = 3.0_f64.max(size as f64 * 0.1);
        let tone = if ctx.mark_color.is_some() {
            fade(mark_rgb(ctx), 0.7)
        } else {
            rgba_of(0xfffaf2, 0.7)
        };
        for (i, &dv) in dist.iter().enumerate() {
            if dv < 0.0 {
                continue;
            }
            let px = dv / 3.0;
            if px > radius {
                continue;
            }
            let t = 1.0 - px / radius;
            let a = t * t;
            if a > 0.01 {
                paint(target, i, tone, a);
            }
        }
    }
}

// ---- 细描边 RingMark ----

fn ring_stroke(size: usize) -> usize {
    1.max(js_round(1.5_f64.max(size as f64 * 0.03)) as usize)
}

pub(crate) struct RingMark;
pub(crate) static RING_MARK: RingMark = RingMark;

impl Mark for RingMark {
    fn placement(&self) -> Placement {
        Placement::Behind
    }
    fn card_inset(&self, ctx: &MarkContext) -> usize {
        ring_stroke(ctx.size)
    }
    fn render(&self, target: &mut Raster, _card_mask: &[f64], ctx: &MarkContext) {
        let ring = if ctx.mark_color.is_some() {
            mark_rgb(ctx)
        } else if is_light_tile(ctx) {
            from_rgb_int(0x141414)
        } else {
            from_rgb_int(0xf5f5f5)
        };
        if ctx.shape != IconShape::None {
            for (i, &a) in ctx.tile_alpha.iter().enumerate() {
                paint(target, i, ring, a);
            }
            return;
        }
        let size = ctx.size;
        let dist = outside_distance(&ctx.tile_alpha, size);
        let stroke = ring_stroke(size) as f64 * 1.6;
        for (i, &dv) in dist.iter().enumerate() {
            if dv < 0.0 {
                continue;
            }
            let a = smooth_step01((stroke - dv / 3.0) / 0.9);
            if a > 0.01 {
                paint(target, i, ring, a);
            }
        }
    }
}

// ---- 缎光角 SatinMark ----

pub(crate) struct SatinMark;
pub(crate) static SATIN_MARK: SatinMark = SatinMark;

impl Mark for SatinMark {
    fn placement(&self) -> Placement {
        Placement::Over
    }
    fn render(&self, target: &mut Raster, _card_mask: &[f64], ctx: &MarkContext) {
        let size = ctx.size;
        let tone = if ctx.mark_color.is_some() {
            if is_light_tile(ctx) {
                mix(mark_rgb(ctx), from_rgb_int(0x101010), 0.62)
            } else {
                mix(mark_rgb(ctx), WHITE, 0.72)
            }
        } else if is_light_tile(ctx) {
            from_rgb_int(0x2a241e)
        } else {
            from_rgb_int(0xffffff)
        };
        let centre = size as f64 / 2.0;
        let dx = 0.70710678;
        let dy = -0.70710678;
        let length = size as f64 * 1.41421356;
        let sheen_alpha = |g: f64| -> f64 {
            if g <= 0.0 {
                0.62
            } else if g <= 0.2 {
                0.62 + (0.3 - 0.62) * (g / 0.2)
            } else if g <= 0.46 {
                0.3 * (1.0 - (g - 0.2) / 0.26)
            } else {
                0.0
            }
        };
        for y in 0..size {
            for x in 0..size {
                let i = y * size + x;
                let cover = ctx.tile_alpha[i];
                if cover <= 0.0 {
                    continue;
                }
                let g = ((x as f64 + 0.5 - centre) * dx + (y as f64 + 0.5 - centre) * dy) / length + 0.5;
                let alpha = sheen_alpha(g);
                if alpha > 0.0 {
                    paint(target, i, fade(tone, alpha), cover);
                }
            }
        }
    }
}

// ---- 珐琅光弧 ArcMark ----

pub(crate) struct ArcMark;
pub(crate) static ARC_MARK: ArcMark = ArcMark;

impl Mark for ArcMark {
    fn placement(&self) -> Placement {
        Placement::Over
    }
    fn render(&self, target: &mut Raster, _card_mask: &[f64], ctx: &MarkContext) {
        let size = ctx.size;
        let arc = if is_light_tile(ctx) {
            mix(mark_rgb(ctx), from_rgb_int(0x141414), 0.78)
        } else {
            mix(mark_rgb(ctx), WHITE, 0.82)
        };
        let cx = 0.15 * size as f64;
        let cy = 0.88 * size as f64;
        let mut radius = 0.0f64;
        for (px, py) in [(0.0, 0.0), (size as f64, 0.0), (0.0, size as f64), (size as f64, size as f64)] {
            let ddx = px - cx;
            let ddy = py - cy;
            radius = radius.max(libm::sqrt(ddx * ddx + ddy * ddy));
        }
        if radius <= 0.0 {
            radius = 1.0;
        }
        let glow_alpha = |d: f64| -> f64 {
            if d <= 0.2 {
                1.0 + (0.55 - 1.0) * (d / 0.2)
            } else if d <= 0.46 {
                0.55 * (1.0 - (d - 0.2) / 0.26)
            } else {
                0.0
            }
        };
        for y in 0..size {
            for x in 0..size {
                let i = y * size + x;
                let cover = ctx.tile_alpha[i];
                if cover <= 0.0 {
                    continue;
                }
                let ddx = x as f64 + 0.5 - cx;
                let ddy = y as f64 + 0.5 - cy;
                let d = libm::sqrt(ddx * ddx + ddy * ddy) / radius;
                let alpha = glow_alpha(d);
                if alpha > 0.0 {
                    paint(target, i, fade(arc, alpha), cover);
                }
            }
        }
    }
}

// ---- 卷角 FoldMark ----

const ROOT2: f64 = std::f64::consts::SQRT_2;
const FOLD_START: f64 = 0.493;
const CREASE_FADE: f64 = 0.02;
const FOLD_BLACK: Rgba = Rgba { r: 0, g: 0, b: 0, a: 255 };

fn fold_depth(ctx: &MarkContext) -> f64 {
    ctx.size as f64
        * match ctx.shape {
            IconShape::Apple => 0.26,
            IconShape::Samsung => 0.28,
            _ => 0.3,
        }
}

fn fold_flap_alpha(ctx: &MarkContext, c0: f64, x0: f64) -> Vec<f64> {
    let size = ctx.size;
    let mut alpha = vec![0.0f64; size * size];
    let rr = 0.6 * c0;
    let start = js_trunc(x0) as usize;
    for y in start..size {
        for x in start..size {
            let p = (size as f64 - (x as f64 + 0.5) + (size as f64 - (y as f64 + 0.5))) / (2.0 * c0);
            let mut a = smooth_step01((p - FOLD_START) / CREASE_FADE);
            if a <= 0.0 {
                continue;
            }
            let lx = x as f64 + 0.5 - x0;
            let ly = y as f64 + 0.5 - x0;
            if lx < rr && ly < rr {
                let dd = libm::sqrt((lx - rr) * (lx - rr) + (ly - rr) * (ly - rr));
                a *= (rr - dd + 0.5).clamp(0.0, 1.0);
            }
            alpha[y * size + x] = a;
        }
    }
    alpha
}

pub(crate) struct FoldMark;
pub(crate) static FOLD_MARK: FoldMark = FoldMark;

impl Mark for FoldMark {
    fn placement(&self) -> Placement {
        Placement::Over
    }
    fn carves_card(&self) -> bool {
        true
    }
    fn carve_card(&self, card_mask: &mut [f64], ctx: &MarkContext) {
        let size = ctx.size;
        let threshold = fold_depth(ctx) / ROOT2;
        for y in 0..size {
            for x in 0..size {
                let proj = (size as f64 - (x as f64 + 0.5) + (size as f64 - (y as f64 + 0.5))) / ROOT2;
                let removed = smooth_step01((threshold - proj) / 0.7);
                if removed > 0.0 {
                    card_mask[y * size + x] *= 1.0 - removed;
                }
            }
        }
    }
    fn render(&self, target: &mut Raster, card_mask: &[f64], ctx: &MarkContext) {
        let size = ctx.size;
        let c0 = fold_depth(ctx);
        let tone = if ctx.mark_color.is_some() {
            if is_light_tile(ctx) {
                mix(mark_rgb(ctx), from_rgb_int(0x101010), 0.7)
            } else {
                mix(mark_rgb(ctx), WHITE, 0.78)
            }
        } else if is_light_tile(ctx) {
            from_rgb_int(0x3a342e)
        } else {
            from_rgb_int(0xf2eee6)
        };
        let hi = mix(tone, WHITE, 0.45);
        let lo = mix(tone, FOLD_BLACK, 0.55);
        let tip = mix(tone, FOLD_BLACK, 0.76);
        let x0 = size as f64 - c0;

        let flap_alpha = fold_flap_alpha(ctx, c0, x0);
        let off = 1.max(js_trunc(size as f64 * 0.02) as i64) as i32;
        let shadow = box_blur(&shift(&flap_alpha, size, -off, -off), size, off);
        for i in 0..shadow.len() {
            if shadow[i] > 0.01 && flap_alpha[i] <= 0.01 {
                paint(target, i, FOLD_BLACK, shadow[i] * 0.3 * card_mask[i]);
            }
        }

        let flap_colour = |p: f64| -> Rgba {
            if p <= 0.498 {
                lo
            } else if p <= 0.57 {
                lerp_rgba(lo, hi, (p - 0.498) / (0.57 - 0.498))
            } else if p <= 0.76 {
                lerp_rgba(hi, tone, (p - 0.57) / (0.76 - 0.57))
            } else {
                lerp_rgba(tone, tip, (p - 0.76) / (1.0 - 0.76))
            }
        };

        let start = js_trunc(x0) as usize;
        for y in start..size {
            for x in start..size {
                let i = y * size + x;
                let a = flap_alpha[i];
                if a <= 0.0 {
                    continue;
                }
                let p = (size as f64 - (x as f64 + 0.5) + (size as f64 - (y as f64 + 0.5))) / (2.0 * c0);
                paint(target, i, flap_colour(p), a * ctx.tile_alpha[i]);
            }
        }
    }
}

// ---- 玻璃箭头 GlassMark ----

pub(crate) struct GlassMark;
pub(crate) static GLASS_MARK: GlassMark = GlassMark;

impl Mark for GlassMark {
    fn placement(&self) -> Placement {
        Placement::Over
    }
    fn render(&self, target: &mut Raster, _card_mask: &[f64], ctx: &MarkContext) {
        let size = ctx.size;
        let cs = (size as f64 * 0.34).max(16.0).min(size as f64 * 0.94);
        let sx = (size as f64 * 0.055).max(0.0).min(size as f64 - cs);
        let sy = (size as f64 - cs - size as f64 * 0.055).max(0.0).min(size as f64 - cs);
        let cx = sx + cs / 2.0;
        let cy = sy + cs / 2.0;
        let seat_r = cs / 2.0;
        let light_seat = ctx.luminance <= ADAPTIVE_THRESHOLD;

        let seat_bg = if light_seat { rgba_of(0xffffff, 0.58) } else { rgba_of(0x18181c, 0.45) };
        let ring_line = if light_seat { rgba_of(0xffffff, 0.55) } else { rgba_of(0xffffff, 0.22) };
        let ink = if ctx.mark_color.is_some() {
            if light_seat {
                mix(mark_rgb(ctx), from_rgb_int(0x101014), 0.72)
            } else {
                mix(mark_rgb(ctx), WHITE, 0.7)
            }
        } else if light_seat {
            from_rgb_int(0x232328)
        } else {
            from_rgb_int(0xf4f4f1)
        };

        let mut seat_cov = vec![0.0f64; size * size];
        let mut y = js_trunc(cy - seat_r - 2.0) as i64;
        while (y as f64) <= cy + seat_r + 2.0 {
            if y >= 0 && (y as usize) < size {
                let mut x = js_trunc(cx - seat_r - 2.0) as i64;
                while (x as f64) <= cx + seat_r + 2.0 {
                    if x >= 0 && (x as usize) < size {
                        let ddx = x as f64 + 0.5 - cx;
                        let ddy = y as f64 + 0.5 - cy;
                        let dist = libm::sqrt(ddx * ddx + ddy * ddy);
                        seat_cov[y as usize * size + x as usize] = (seat_r - dist + 0.5).clamp(0.0, 1.0);
                    }
                    x += 1;
                }
            }
            y += 1;
        }

        let blurred = backdrop_blur(target, 1.max(js_round(size as f64 * 0.06) as i64) as i32);
        let mut y = js_trunc(cy - seat_r - 2.0) as i64;
        while (y as f64) <= cy + seat_r + 2.0 {
            if y >= 0 && (y as usize) < size {
                let mut x = js_trunc(cx - seat_r - 2.0) as i64;
                while (x as f64) <= cx + seat_r + 2.0 {
                    if x >= 0 && (x as usize) < size {
                        let i = y as usize * size + x as usize;
                        let cov = seat_cov[i];
                        if cov > 0.0 {
                            let i4 = i * 4;
                            let bd = &blurred.data;
                            let frosted = over_rgba(
                                seat_bg,
                                Rgba { r: bd[i4], g: bd[i4 + 1], b: bd[i4 + 2], a: bd[i4 + 3] },
                            );
                            {
                                let td = &mut target.data;
                                td[i4] = clamp_u8_int(js_round(td[i4] as f64 + (frosted.r as f64 - td[i4] as f64) * cov));
                                td[i4 + 1] = clamp_u8_int(js_round(td[i4 + 1] as f64 + (frosted.g as f64 - td[i4 + 1] as f64) * cov));
                                td[i4 + 2] = clamp_u8_int(js_round(td[i4 + 2] as f64 + (frosted.b as f64 - td[i4 + 2] as f64) * cov));
                                td[i4 + 3] = clamp_u8_int(js_round(td[i4 + 3] as f64 + (frosted.a as f64 - td[i4 + 3] as f64) * cov));
                            }
                            let ddx = x as f64 + 0.5 - cx;
                            let ddy = y as f64 + 0.5 - cy;
                            let dist = libm::sqrt(ddx * ddx + ddy * ddy);
                            let ring_cov = smooth_step01((1.2 - (dist - (seat_r - 0.6)).abs()) / 1.2);
                            paint(target, i, ring_line, ring_cov);
                        }
                    }
                    x += 1;
                }
            }
            y += 1;
        }

        draw_arrow_glyph(target, size, cx, cy, cs * 0.3, ink, &seat_cov);
    }
}
