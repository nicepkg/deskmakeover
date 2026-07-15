//! Icon DTOs — the Rust source for the THIN icon bridge contract in `bridge/types.ts`
//! (BRIDGE_SCHEMA_VERSION). Field names and enum string literals mirror the TS sub-types
//! exactly so the generated bindings drop in place.
//!
//! **D1-consistent THIN boundary (owner ruling 2026-07-12), applied throughout.** Icons Rust
//! does ONLY the genuine platform work: (a) `scan` the desktop into raw classified items, (b)
//! package + apply baked masters through the durable txn engine, (c) `restore`, and (d) persist
//! stores ②/③ (saved-style + look-history, spec 07 §8). It does NOT assemble `IconsStateDto`:
//! presets, palette, swatches, the grid, `activePresetId`, and the config DRAFT are all
//! FRONTEND rendering/session concerns, exactly the line D1 drew for wallpaper. So the bridge
//! SHRINKS:
//!
//! * `scan` → [`IconScanDto`] (revision + raw items, NO embedded state);
//! * `getState`→`getPersisted` → [`IconPersistedDto`] (the persisted + native bits the frontend
//!   overlays onto its assembled state — saved-style, look-history, applied, arrow, profiles);
//! * `setLook` LEAVES the bridge (a config/override/kindPolicy/typeOverrides draft is not
//!   intent — spec 07 §8.2 writes ② only on a completed global Apply — so, like wallpaper's
//!   `setLook`, the draft lives in the frontend store);
//! * the mutating verbs (`applyBaked*` / `restore` / `restoreOverlay` / `exportCompare`) return a
//!   thin [`IconOpResultDto`] carrying the fresh [`IconPersistedDto`], so the frontend overlays
//!   the new persisted state through the SAME path `getPersisted` feeds, without a re-fetch.
//!
//! **`IconStyle` rides as an opaque, validated JSON STRING** (`styleJson` / `savedStyleJson`).
//! The recipe internals (`{config, kindPolicy, typeOverrides}`) are frontend-owned and opaque to
//! Rust — the same ownership line D1 drew for wallpaper looks — so the bridge carries the recipe
//! the way it carries a baked wallpaper PNG: as a string Rust validates on the way in (via
//! [`crate::IconStyle`]) and never types structurally. This keeps the generated bindings free of
//! the recipe's rich shape; the frontend parses the string to its own typed `IconStyle`.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::common::ToastDto;

/// The kind of one desktop item (spec 06 §6 taxonomy). Serialized as the variant name to match
/// the TS `IconKind` union. `SystemIcon`/`ExecutableFile` are host-classification refinements the
/// scanner assigns; the operations layer maps them onto its `ItemKind` write mechanisms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum IconKindDto {
    Shortcut,
    UrlShortcut,
    AppxShortcut,
    RecycleBin,
    SystemIcon,
    Folder,
    RegularFile,
    ExecutableFile,
    Unsupported,
}

/// One desktop item as the scanner observed it — raw platform truth ONLY (no override, no style).
/// The frontend overlays its own draft per-icon overrides + renders the styling locally from
/// `sourceUrls` (256px, `[0]` primary; the Recycle Bin ships TWO — empty + full). Positions are
/// OBSERVED desktop truth, never predicted. Mirrors the styleable subset of the TS `IconItemDto`
/// (the TS shape additionally carries the frontend-owned `overrideMode`/`overrideTint`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IconItemDto {
    pub id: String,
    pub label: String,
    pub kind: IconKindDto,
    pub is_shortcut: bool,
    pub styleable: bool,
    /// Host-localized human reason when `styleable` is false (e.g. a genuinely unreadable item);
    /// `None` when styleable.
    pub status_reason: Option<String>,
    pub x: i32,
    pub y: i32,
    /// The item's 256px source URL(s) the compositor renders from; `[0]` is primary. The Recycle
    /// Bin carries two (empty + full).
    pub source_urls: Vec<String>,
}

/// One baked master in an `icons.applyBakedChunk` batch (a command INPUT). `sourceIndex` 0 =
/// primary, 1 = paired empty (Recycle Bin); `masterPng` is the base64 256px straight-alpha RGBA
/// PNG the frontend WASM-baked. Mirrors the TS `icons.applyBakedChunk` item shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IconChunkItemDto {
    pub id: String,
    pub source_index: u32,
    pub master_png: String,
}

/// The OBSERVED desktop metrics a scan reports, so the frontend assembles the grid from PLATFORM
/// truth instead of fabricating dims (codex Major 5 — a hardcoded 1920×1080 lies on 4K/ultrawide/
/// side-taskbar desktops). `cell_width`/`cell_height`/`icon_px` carry the TRUE snap-cell pitch and
/// icon size from the live shell view (`IFolderView::GetSpacing` + `GetViewModeAndIconSize`);
/// they are `None` when that walk fails, and the frontend then falls back to its approximation
/// constants (owner report 2026-07-16: the fabricated 92px cell rendered every icon ~22px right
/// of where Windows draws it). [WINDOWS-VERIFY] the real `SPI_GETWORKAREA` + shell icon metrics
/// on the box; the dev host synthesizes plausible values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GridMetricsDto {
    pub screen_width: u32,
    pub screen_height: u32,
    pub taskbar_height: u32,
    pub cell_width: Option<u32>,
    pub cell_height: Option<u32>,
    pub icon_px: Option<u32>,
}

/// The result of `icons.scan`: a monotonically increasing revision, the raw observed items, and the
/// observed desktop grid metrics. NO embedded `IconsStateDto` (D1: the frontend assembles it from
/// these + `getPersisted` + its own presets/palette). Mirrors the TS `icons.scan` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IconScanDto {
    pub revision: u32,
    pub items: Vec<IconItemDto>,
    pub grid: GridMetricsDto,
}

/// One saved appearance recipe from store ③ (look-history), mirroring `dm_operations::LookVersion`
/// with the recipe carried as an opaque JSON string (`styleJson`). The frontend renders each
/// entry's style-sample mini from the parsed recipe (spec 07 §8.1 — never a desktop screenshot).
// No `Eq`: `created_at` is `f64` (a TS `number`), which has no total equality.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LookVersionDto {
    pub id: String,
    /// UNIX seconds as a TS `number` — specta-typescript forbids raw i64 export (JS has no i64),
    /// and a seconds timestamp is far below 2^53 so `f64` is lossless. The host maps the store's
    /// `i64` `created_at` in; this is output-only (the frontend never sends it back).
    pub created_at: f64,
    /// A user-chosen name; `None` = unnamed. Independent of `pinned` (owner ruling 2026-07-12:
    /// naming is an unlimited label, pinning is the eviction exemption).
    pub label: Option<String>,
    /// Exempt from the store's FIFO eviction while set (spec 07 §17).
    pub pinned: bool,
    /// The `{config, kindPolicy, typeOverrides}` recipe as an opaque JSON string (see module docs).
    pub style_json: String,
}

/// The native shortcut-arrow overlay state (ADR-0021 machine-wide overlay). `native` = Windows
/// draws its own arrow (pre-first-apply, or after a restore); `hidden` = the global transparent
/// overlay is installed and DeskMakeover draws the mark. Serialized lowercase to match the TS
/// `'native' | 'hidden'` union.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum ArrowOverlayDto {
    Native,
    Hidden,
}

/// The persisted + native icon state the frontend overlays onto its assembled `IconsStateDto`
/// (D1: the frontend owns presets/palette/swatches/grid/`activePresetId`/the config draft). This
/// is what `icons.getPersisted` returns and what every mutating op reports back, so there is ONE
/// overlay path. Mirrors the TS `IconPersistedDto`.
// No `Eq`: carries `LookVersionDto`s (which hold an `f64` timestamp).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IconPersistedDto {
    /// Store ② — the current global saved-style recipe as an opaque JSON string, or `None` when no
    /// global Apply has ever run (the resident then treats it as "nothing to project", spec 07 §8.3).
    pub saved_style_json: Option<String>,
    /// Store ③ — up to 10 saved looks, newest-first (already capped + pin-normalized by the store).
    pub history: Vec<LookVersionDto>,
    /// Whether a look is currently applied (the active ledger holds at least one styled row) — the
    /// frontend's `applied`/restore affordance authority, surviving a cold start.
    pub applied: bool,
    pub arrow_overlay: ArrowOverlayDto,
    /// Count of active user profiles on this machine (>1 makes the machine-wide arrow disclosure
    /// non-skippable; owner disposition 3). Host truth on Windows.
    pub active_user_profiles: u32,
}

/// The THIN result of a mutating icon op (`applyBaked*` commit / `restore` / `restoreOverlay` /
/// `exportCompare`): success, an optional localized toast, and the FRESH persisted state so the
/// frontend re-overlays without a second round-trip. Mirrors the TS `IconOpResultDto`.
// No `Eq`: carries `IconPersistedDto` (which holds `f64` timestamps).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IconOpResultDto {
    pub ok: bool,
    pub toast: Option<ToastDto>,
    pub persisted: IconPersistedDto,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_serializes_as_the_ts_literals() {
        for (variant, literal) in [
            (IconKindDto::Shortcut, "\"Shortcut\""),
            (IconKindDto::UrlShortcut, "\"UrlShortcut\""),
            (IconKindDto::AppxShortcut, "\"AppxShortcut\""),
            (IconKindDto::RecycleBin, "\"RecycleBin\""),
            (IconKindDto::SystemIcon, "\"SystemIcon\""),
            (IconKindDto::Folder, "\"Folder\""),
            (IconKindDto::RegularFile, "\"RegularFile\""),
            (IconKindDto::ExecutableFile, "\"ExecutableFile\""),
            (IconKindDto::Unsupported, "\"Unsupported\""),
        ] {
            assert_eq!(serde_json::to_string(&variant).unwrap(), literal);
            assert_eq!(serde_json::from_str::<IconKindDto>(literal).unwrap(), variant);
        }
    }

    #[test]
    fn arrow_overlay_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&ArrowOverlayDto::Native).unwrap(), "\"native\"");
        assert_eq!(serde_json::to_string(&ArrowOverlayDto::Hidden).unwrap(), "\"hidden\"");
        assert_eq!(serde_json::from_str::<ArrowOverlayDto>("\"hidden\"").unwrap(), ArrowOverlayDto::Hidden);
    }

    #[test]
    fn item_uses_camel_case_keys_and_no_override_fields() {
        let item = IconItemDto {
            id: "abc".into(),
            label: "记事本".into(),
            kind: IconKindDto::Shortcut,
            is_shortcut: true,
            styleable: true,
            status_reason: None,
            x: 14,
            y: 62,
            source_urls: vec!["dmicon://abc/0".into()],
        };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("\"isShortcut\""), "got {json}");
        assert!(json.contains("\"statusReason\":null"));
        assert!(json.contains("\"sourceUrls\""));
        // Per-icon overrides are frontend draft state — they must NOT appear on a scan item.
        assert!(!json.contains("override"), "scan items carry no override fields: {json}");
        let back: IconItemDto = serde_json::from_str(&json).unwrap();
        assert_eq!(back, item);
    }

    #[test]
    fn scan_dto_round_trips() {
        let scan = IconScanDto {
            revision: 3,
            grid: GridMetricsDto {
                screen_width: 3840,
                screen_height: 2160,
                taskbar_height: 48,
                cell_width: Some(76),
                cell_height: Some(97),
                icon_px: Some(48),
            },
            items: vec![IconItemDto {
                id: "bin".into(),
                label: "回收站".into(),
                kind: IconKindDto::RecycleBin,
                is_shortcut: false,
                styleable: true,
                status_reason: None,
                x: 0,
                y: 0,
                // Recycle Bin ships two sources (empty + full).
                source_urls: vec!["dmicon://bin/0".into(), "dmicon://bin/1".into()],
            }],
        };
        let back: IconScanDto = serde_json::from_str(&serde_json::to_string(&scan).unwrap()).unwrap();
        assert_eq!(back, scan);
    }

    #[test]
    fn look_version_carries_the_style_as_a_string() {
        let v = LookVersionDto {
            id: "v1".into(),
            created_at: 1_700_000_000.0,
            label: Some("我的最爱".into()),
            pinned: true,
            style_json: r#"{"config":{},"kindPolicy":{},"typeOverrides":{}}"#.into(),
        };
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains("\"createdAt\""), "got {json}");
        assert!(json.contains("\"styleJson\""));
        let back: LookVersionDto = serde_json::from_str(&json).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn persisted_dto_keys_and_null_saved_style() {
        let p = IconPersistedDto {
            saved_style_json: None,
            history: vec![],
            applied: false,
            arrow_overlay: ArrowOverlayDto::Native,
            active_user_profiles: 1,
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"savedStyleJson\":null"), "got {json}");
        assert!(json.contains("\"arrowOverlay\":\"native\""));
        assert!(json.contains("\"activeUserProfiles\":1"));
        let back: IconPersistedDto = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn op_result_round_trips_with_a_toast() {
        let res = IconOpResultDto {
            ok: false,
            toast: Some(ToastDto { key: "icons.restoreDeclined".into(), arg: None }),
            persisted: IconPersistedDto {
                saved_style_json: Some(r#"{"config":{},"kindPolicy":{},"typeOverrides":{}}"#.into()),
                history: vec![],
                applied: true,
                arrow_overlay: ArrowOverlayDto::Hidden,
                active_user_profiles: 2,
            },
        };
        let back: IconOpResultDto =
            serde_json::from_str(&serde_json::to_string(&res).unwrap()).unwrap();
        assert_eq!(back, res);
    }

    #[test]
    fn chunk_item_uses_camel_case_keys() {
        let c = IconChunkItemDto { id: "app".into(), source_index: 1, master_png: "iVBOR".into() };
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("\"sourceIndex\":1"), "got {json}");
        assert!(json.contains("\"masterPng\""));
        let back: IconChunkItemDto = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn dtos_tolerate_unknown_fields_from_a_newer_frontend() {
        // No `deny_unknown_fields`: a newer TS build sending an extra key must not break decode.
        let item: IconItemDto = serde_json::from_str(
            r#"{"id":"a","label":"l","kind":"Folder","isShortcut":false,"styleable":true,"statusReason":null,"x":1,"y":2,"sourceUrls":[],"future":true}"#,
        )
        .unwrap();
        assert_eq!(item.kind, IconKindDto::Folder);
        let persisted: IconPersistedDto = serde_json::from_str(
            r#"{"savedStyleJson":null,"history":[],"applied":false,"arrowOverlay":"native","activeUserProfiles":1,"extra":9}"#,
        )
        .unwrap();
        assert!(!persisted.applied);
    }
}
