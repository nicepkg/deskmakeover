//! The opaque generated-asset reference and the record of which fields DeskMakeover owns.
//!
//! The pixel/ICO bytes are produced by the icon core (`dm-icon-core`/`dm-icon-codec`); the
//! transaction layer never inspects them. It sees only a content-addressed [`AssetRef`]
//! (`<source-hash>-<style-hash>.ico`, spec 07 §5) plus the [`OwnedFields`] the apply set, so
//! restore/CAS can tell our writes from external ones.

use serde::{Deserialize, Serialize};

/// A content-addressed reference to a generated `.ico`. The `hash` is the caller's content
/// hash (source-hash + style-hash); `path` is where the bytes were materialized. Two applies
/// that need the same asset reuse the same file (write-new-then-swap, then GC unreferenced).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetRef {
    pub hash: String,
    pub path: String,
}

impl AssetRef {
    pub fn new(hash: impl Into<String>, path: impl Into<String>) -> Self {
        Self { hash: hash.into(), path: path.into() }
    }
}

/// Which fields of an item DeskMakeover owns after an apply (ADR-0020 §2 "owned fields").
/// For icons the only owned field in v1 is the icon location; the struct is future-proofed so
/// additional owned surfaces (e.g. a folder's `desktop.ini` sections) can be tracked without a
/// ledger schema break.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedFields {
    /// We set this item's icon location / `DefaultIcon` / `desktop.ini` `IconResource`.
    pub icon_location: bool,
}

impl Default for OwnedFields {
    fn default() -> Self {
        Self { icon_location: true }
    }
}

impl OwnedFields {
    /// The v1 default: DeskMakeover owns the icon location and nothing else.
    pub fn icon_only() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_fields_default_is_icon_only() {
        assert_eq!(OwnedFields::default(), OwnedFields::icon_only());
        assert!(OwnedFields::default().icon_location);
    }

    #[test]
    fn asset_ref_round_trips_and_equates_by_value() {
        let a = AssetRef::new("abc-def", r"C:\gen\abc-def.ico");
        assert_eq!(a, AssetRef::new("abc-def", r"C:\gen\abc-def.ico"));
        assert_ne!(a, AssetRef::new("other", r"C:\gen\abc-def.ico"));
        let back: AssetRef = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        assert_eq!(a, back);
    }
}
