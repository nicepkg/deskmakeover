//! DTO mapping: recipe (de)serialization, look-history + kind mapping, and the persisted-state DTO.

use dm_contracts::{ArrowOverlayDto, IconKindDto, IconPersistedDto, IconStyle, LookVersionDto};
use dm_domain::ItemKind;
use dm_operations::{IconStoreState, LookVersion};

use super::IconHost;

/// Parses + validates the opaque recipe string into an `IconStyle` (rejects a malformed envelope).
pub(super) fn parse_style(style_json: &str) -> Result<IconStyle, String> {
    serde_json::from_str::<IconStyle>(style_json)
        .map_err(|e| format!("invalid icon style: {e}"))
}

fn style_to_json(style: &IconStyle) -> String {
    serde_json::to_string(style).expect("a validated IconStyle always serializes")
}

fn look_to_dto(v: &LookVersion) -> LookVersionDto {
    LookVersionDto {
        id: v.id.clone(),
        created_at: v.created_at as f64,
        label: v.label.clone(),
        pinned: v.pinned,
        style_json: style_to_json(&v.icon_style),
    }
}

pub(super) fn map_kind(k: ItemKind) -> IconKindDto {
    match k {
        ItemKind::Shortcut => IconKindDto::Shortcut,
        ItemKind::UrlShortcut => IconKindDto::UrlShortcut,
        ItemKind::AppxShortcut => IconKindDto::AppxShortcut,
        ItemKind::RecycleBin => IconKindDto::RecycleBin,
        ItemKind::Folder => IconKindDto::Folder,
        ItemKind::RegularFile => IconKindDto::RegularFile,
        ItemKind::System => IconKindDto::SystemIcon,
        ItemKind::Unsupported => IconKindDto::Unsupported,
    }
}

/// Synthetic desktop-grid positions for the dev host (a fixed column-major layout). On Windows the
/// real observed `IFolderView2` positions replace these ([WINDOWS-VERIFY]).
pub(super) fn synthetic_layout(i: usize) -> (i32, i32) {
    const ROWS: usize = 6;
    let col = (i / ROWS) as i32;
    let row = (i % ROWS) as i32;
    (24 + col * 104, 24 + row * 116)
}

impl IconHost {
    /// Maps the ops-layer store snapshot to the wire DTO (recipe as opaque JSON string). Safe to
    /// call while the mut lock is held: it does NOT touch the arrow lock — the caller stamps the
    /// live arrow via [`finish_persisted`] after releasing the mut lock.
    pub(super) fn to_persisted_dto_locked(&self, stores: &IconStoreState) -> IconPersistedDto {
        IconPersistedDto {
            saved_style_json: stores.saved_style.as_ref().map(style_to_json),
            history: stores.history.iter().map(look_to_dto).collect(),
            applied: stores.applied,
            // Placeholder arrow; `finish_persisted` stamps the live value (avoids a nested lock while
            // the mut lock is held).
            arrow_overlay: ArrowOverlayDto::Native,
            active_user_profiles: self.active_user_profiles,
        }
    }

    /// Stamps the live arrow-overlay state onto a persisted DTO built while the mut lock was held.
    pub(super) fn finish_persisted(&self, mut dto: IconPersistedDto) -> IconPersistedDto {
        dto.arrow_overlay = *self.arrow_overlay.lock().unwrap();
        dto
    }
}
