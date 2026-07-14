//! The resolved per-tile style contract — the Rust mirror of `bridge/types.ts`
//! `ConfigDto` as `renderTile` consumes it (per-type ladder + shortcut layer
//! already folded upstream by the store's `effectiveTileConfig`). Colours arrive
//! pre-parsed to packed `0xRRGGBB` ints (the TS side calls `hexToInt` at every
//! use site); the string style-key is a UI cache concern and is NOT modelled here.

pub use crate::shapes::IconShape;

/// 主体 axis (ADR-0018): how the artwork renders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subject {
    Original,
    BlackWhite,
    Mono,
}

/// Mono depth: Tonal = single-hue ramp; Flat = 极致单色 flat-subject-on-flat-plate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonoStyle {
    Tonal,
    Flat,
}

/// Derived-plate depth band (`PlateBand`/legacy `FieldBand`): the shared
/// lightness line the themed plates ride.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band {
    Vivid,
    Quiet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Distinction {
    Mark,
    Keep,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkStyle {
    Glass,
    Shadow,
    Halo,
    Satin,
    Arc,
    Fold,
    Ring,
    /// 箭头徽章 (spec 02, owner-disposed 2026-07-15): self-grounded squircle
    /// arrow badge, the un-gated beautiful arrow.
    Comet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterStyle {
    None,
    Gloss,
    Glass,
    Pixel,
    Sticker,
}

/// null-plate fallback policy (ADR-0018): Derived = 满彩 themed plates; White =
/// 本色 classic pipeline (anchored own boards, white bare fallback, no shadows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlateFallback {
    Derived,
    White,
}

/// The resolved style for one tile (`ConfigDto`, colours pre-parsed).
#[derive(Clone, Debug)]
pub struct Config {
    pub shape: IconShape,
    pub subject: Subject,
    /// `hexToInt(config.tint)`.
    pub tint: u32,
    pub mono_style: MonoStyle,
    pub plate_band: Band,
    pub shortcut_shape: Option<IconShape>,
    pub distinction: Distinction,
    pub mark_style: MarkStyle,
    /// `hexToInt(config.markColor)`; None = auto (brand coral).
    pub mark_color: Option<u32>,
    pub filter: FilterStyle,
    /// `hexToInt(config.plateColor)`; None = 随图标 (derived).
    pub plate_color: Option<u32>,
    pub plate_fallback: PlateFallback,
}
