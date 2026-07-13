//! Contracts (ADR-0019): serde DTOs and generated TypeScript bindings
//! (tauri-specta) — the single contract source; hand-mirrored schemas are banned.
//!
//! M2 covers the first slice only: settings get/set + a diagnostics ping. Every
//! DTO derives `specta::Type`, so `src-tauri` exports byte-identical
//! TypeScript into `src/bridge/generated.ts` (drift is a CI failure).
//! The rest of bridge schema 4 keeps living in `bridge/types.ts` until later
//! phases migrate it here.

mod common;
mod icons;
mod settings;
mod style;
mod tweaks;
mod wallpaper;

pub use common::{SystemInfoDto, ToastDto};
pub use icons::{
    ArrowOverlayDto, GridMetricsDto, IconChunkItemDto, IconItemDto, IconKindDto, IconOpResultDto,
    IconPersistedDto, IconScanDto, LookVersionDto,
};
pub use settings::{Language, SettingsDto, SettingsPatch, Theme};
pub use tweaks::{
    CalmApplyOutcomeDto, CalmApplyRowDto, CalmGuidedProbeDto, CalmProbeRowDto, CalmProbeStateDto,
    CalmRestoreOutcomeDto, CalmRestoreRowDto, CalmSkipReasonDto,
};
pub use style::{IconStyle, IconStyleError};
pub use wallpaper::{
    MonitorBounds, ScreenInfoDto, ScreenOrientation, WallpaperPosition, WallpaperResultDto,
    WallpaperScreensDto, WallpaperSourceDto,
};
