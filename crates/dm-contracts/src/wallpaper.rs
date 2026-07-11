//! Wallpaper DTOs — the Rust source for the thin wallpaper bridge contract in
//! `bridge/types.ts` (BRIDGE_SCHEMA_VERSION). Field names and enum string literals
//! mirror the TS sub-types exactly so the generated bindings drop in place.
//!
//! **Owner ruling D1 (2026-07-12), applied throughout.** Wallpaper Rust does ONLY
//! (a) read multi-monitor screen info, (b) get/set wallpaper, (c) capture/restore
//! the pre-first-apply snapshot. Reconcile, per-monitor draft-look persistence, and
//! `WallpaperStateDto` assembly are FRONTEND. So the bridge SHRINKS: `getScreens`
//! returns a THIN shape ([`WallpaperScreensDto`]) — raw screens + globals, NO looks,
//! NO grids, NO reconcile — and apply/restore return a THIN [`WallpaperResultDto`]
//! (the frontend re-fetches `getScreens` and re-assembles state; Rust never
//! assembles it).

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::common::ToastDto;

/// Virtual-desktop bounds of one monitor, in physical pixels (IDesktopWallpaper
/// `GetMonitorRECT`). `x`/`y` may be negative (a monitor left of / above the
/// primary on the virtual desktop). Mirrors the TS `MonitorBounds`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct MonitorBounds {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Screen orientation, derived from the bounds aspect (`h > w` ⇒ portrait).
/// Serialized lowercase to match the TS `ScreenOrientation` union.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum ScreenOrientation {
    Portrait,
    Landscape,
}

/// GLOBAL wallpaper positioning (Windows `DesktopWallpaperPosition`). Only image
/// PATHS are per-monitor; position/slideshow/bg-color are whole-desktop. `Span`
/// stretches ONE image across every monitor. Serialized as the variant name to
/// match the TS `WallpaperPosition` union.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum WallpaperPosition {
    Center,
    Tile,
    Stretch,
    Fit,
    Fill,
    Span,
}

/// Decoded, cover-cropped source the compositor renders from. On the host `url` is
/// the `dmwallpaper://<monitorId>?rev=N` custom-protocol URL (WIC-decoded PNG); in
/// the mock it is the scene bitmap URL. `width`/`height` are the DECODED image's
/// true pixel dims (NOT the monitor bounds), so the compositor cover-crops to each
/// screen's aspect. Mirrors the TS `WallpaperSourceDto`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperSourceDto {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

/// One physical monitor's raw screen info (thin, per D1: NO look, NO grid — the
/// frontend reconciles + overlays persisted looks + assembles `WallpaperStateDto`).
/// Mirrors the TS `ScreenInfoDto`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ScreenInfoDto {
    /// Windows device path (`GetMonitorDevicePathAt`) — durable-ish, not permanent
    /// across port/driver/dock/EDID changes. The frontend reconciles by this path
    /// then by exact bounds.
    pub monitor_id: String,
    pub name: String,
    pub bounds: MonitorBounds,
    pub orientation: ScreenOrientation,
    /// Decoded per-screen source; `None` when unreadable — a third-party
    /// dynamic/video wallpaper is invisible to `IDesktopWallpaper` (import CTA).
    pub source: Option<WallpaperSourceDto>,
    /// Windows slideshow active on this monitor (rotation won't re-arm after apply).
    pub slideshow_active: bool,
    /// `GetWallpaper` returned a readable image path (`false` ⇒ dynamic/video
    /// wallpaper — distinct from a solid-colour desktop, which reads readable-empty).
    pub has_readable_source: bool,
}

/// The THIN screen enumeration `wallpaper.getScreens` returns: raw screens + global
/// desktop flags only. NO looks, NO grids, NO reconcile (D1: all frontend). Mirrors
/// the TS `wallpaper.getScreens` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperScreensDto {
    pub screens: Vec<ScreenInfoDto>,
    pub position: WallpaperPosition,
    /// `position == Span` — the UI degrades to a unified canvas. Reported
    /// explicitly (the host detects it) rather than derived. Mirrors the TS
    /// `spanActive`.
    pub span_active: bool,
}

/// The THIN result of a mutating wallpaper op (`applyBaked` / `restore`). Per D1 the
/// host does NOT assemble `WallpaperStateDto`; it reports only success, an optional
/// toast, and whether a pre-first-apply snapshot now exists (so the frontend can
/// enable the whole-desktop restore affordance). The frontend re-fetches
/// `getScreens` and re-assembles state itself. Mirrors the TS `WallpaperResultDto`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperResultDto {
    pub ok: bool,
    pub toast: Option<ToastDto>,
    /// `true` once the pre-first-apply snapshot has been captured and persisted —
    /// the single durable guard against the first apply destroying the original
    /// desktop with no way back.
    pub has_backup: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orientation_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&ScreenOrientation::Portrait).unwrap(), "\"portrait\"");
        assert_eq!(serde_json::to_string(&ScreenOrientation::Landscape).unwrap(), "\"landscape\"");
    }

    #[test]
    fn orientation_round_trips_from_ts_literals() {
        assert_eq!(
            serde_json::from_str::<ScreenOrientation>("\"portrait\"").unwrap(),
            ScreenOrientation::Portrait
        );
        assert_eq!(
            serde_json::from_str::<ScreenOrientation>("\"landscape\"").unwrap(),
            ScreenOrientation::Landscape
        );
    }

    #[test]
    fn position_serializes_as_variant_name() {
        for (variant, literal) in [
            (WallpaperPosition::Center, "\"Center\""),
            (WallpaperPosition::Tile, "\"Tile\""),
            (WallpaperPosition::Stretch, "\"Stretch\""),
            (WallpaperPosition::Fit, "\"Fit\""),
            (WallpaperPosition::Fill, "\"Fill\""),
            (WallpaperPosition::Span, "\"Span\""),
        ] {
            assert_eq!(serde_json::to_string(&variant).unwrap(), literal);
            assert_eq!(serde_json::from_str::<WallpaperPosition>(literal).unwrap(), variant);
        }
    }

    #[test]
    fn bounds_keys_are_the_ts_field_names() {
        let json = serde_json::to_string(&MonitorBounds { x: -1920, y: 0, w: 1080, h: 1920 }).unwrap();
        assert_eq!(json, r#"{"x":-1920,"y":0,"w":1080,"h":1920}"#);
    }

    #[test]
    fn source_uses_camel_case_and_true_dims() {
        let dto = WallpaperSourceDto { url: "dmwallpaper://m0?rev=3".into(), width: 3840, height: 2400 };
        let json = serde_json::to_string(&dto).unwrap();
        assert_eq!(json, r#"{"url":"dmwallpaper://m0?rev=3","width":3840,"height":2400}"#);
        let back: WallpaperSourceDto = serde_json::from_str(&json).unwrap();
        assert_eq!(back, dto);
    }

    #[test]
    fn screen_info_uses_camel_case_keys() {
        let dto = ScreenInfoDto {
            monitor_id: "\\\\?\\DISPLAY#0".into(),
            name: "主显示器".into(),
            bounds: MonitorBounds { x: 0, y: 0, w: 1920, h: 1080 },
            orientation: ScreenOrientation::Landscape,
            source: None,
            slideshow_active: false,
            has_readable_source: true,
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("\"monitorId\""), "got {json}");
        assert!(json.contains("\"slideshowActive\""));
        assert!(json.contains("\"hasReadableSource\""));
        // A null source round-trips to None (an unreadable dynamic wallpaper).
        assert!(json.contains("\"source\":null"));
    }

    #[test]
    fn screens_dto_round_trips_field_by_field() {
        let dto = WallpaperScreensDto {
            screens: vec![ScreenInfoDto {
                monitor_id: "m0".into(),
                name: "primary".into(),
                bounds: MonitorBounds { x: 0, y: 0, w: 3840, h: 2400 },
                orientation: ScreenOrientation::Landscape,
                source: Some(WallpaperSourceDto { url: "dmwallpaper://m0?rev=1".into(), width: 3840, height: 2400 }),
                slideshow_active: false,
                has_readable_source: true,
            }],
            position: WallpaperPosition::Fill,
            span_active: false,
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("\"spanActive\""), "got {json}");
        let back: WallpaperScreensDto = serde_json::from_str(&json).unwrap();
        assert_eq!(back, dto);
    }

    #[test]
    fn result_dto_keys_and_null_toast() {
        let dto = WallpaperResultDto { ok: true, toast: None, has_backup: true };
        let json = serde_json::to_string(&dto).unwrap();
        assert_eq!(json, r#"{"ok":true,"toast":null,"hasBackup":true}"#);
        let back: WallpaperResultDto = serde_json::from_str(&json).unwrap();
        assert_eq!(back, dto);
    }

    #[test]
    fn result_dto_carries_a_toast() {
        let dto = WallpaperResultDto {
            ok: false,
            toast: Some(ToastDto { key: "wallpaper.restoreFailed".into(), arg: Some("m0".into()) }),
            has_backup: false,
        };
        let back: WallpaperResultDto =
            serde_json::from_str(&serde_json::to_string(&dto).unwrap()).unwrap();
        assert_eq!(back, dto);
    }

    #[test]
    fn dtos_tolerate_unknown_fields_from_a_newer_frontend() {
        // No `deny_unknown_fields`: a newer TS build sending an extra key must not break decode.
        let src: WallpaperSourceDto =
            serde_json::from_str(r#"{"url":"u","width":1,"height":2,"future":true}"#).unwrap();
        assert_eq!(src.width, 1);
        let res: WallpaperResultDto =
            serde_json::from_str(r#"{"ok":true,"toast":null,"hasBackup":false,"extra":9}"#).unwrap();
        assert!(res.ok);
    }
}
