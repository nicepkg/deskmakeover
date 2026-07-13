//! Exact-restore anchors: everything needed to return one item to its true original with
//! zero residue, captured BEFORE any mutation.
//!
//! Harvested from the frozen oracle: `DeskMakeover.Shell/RestoreMetadataCollector.cs` (the
//! per-kind capture) and `DesktopBakeService.TryRestoreItem` (the per-kind replay). The
//! reversibility invariant the anchor exists to guarantee (`DesktopBakeService` codex B4):
//! the anchor is journaled before the desktop is touched, so a crash at any point can always
//! be walked back to the captured original.

use serde::{Deserialize, Serialize};

/// Bytes serialized as base64 so the JSON journal/ledger stays compact (matching the oracle,
/// which stored `Convert.ToBase64String` blobs in snapshot metadata).
mod bytes_base64 {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(d)?;
        STANDARD.decode(text.as_bytes()).map_err(serde::de::Error::custom)
    }
}

/// The captured original of a folder's `desktop.ini` (present-case only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopIniAnchor {
    #[serde(with = "bytes_base64")]
    pub content: Vec<u8>,
    /// The raw Win32 `FILE_ATTRIBUTE_*` bits of the original `desktop.ini`.
    pub attributes: u32,
}

/// The captured original of a loose file's wrapping state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WrapperAnchor {
    /// The original file's `FILE_ATTRIBUTE_*` bits (restored on unwrap).
    pub file_attributes: u32,
    /// The pre-existing same-named wrapper `.lnk` state. An enum so "a wrapper existed but we saved
    /// no bytes" — a state where restore silently leaves OUR styled wrapper in place instead of
    /// resurrecting the user's file — is UNREPRESENTABLE (audit A1-🔴: the old `wrapper_existed:true`
    /// + `wrapper_content:None` pair passed `has_material` yet could not be restored).
    pub prior_wrapper: PriorWrapper,
}

/// A loose file's pre-apply wrapper state — either no wrapper existed, or one did and its exact
/// bytes are captured. Present ALWAYS carries the content restore needs (audit A1-🔴).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PriorWrapper {
    /// No same-named wrapper `.lnk` existed before the apply → restore just removes ours.
    Absent,
    /// A wrapper existed; our apply overwrote it, so restore resurrects these exact bytes.
    Present {
        #[serde(with = "bytes_base64")]
        content: Vec<u8>,
    },
}

/// The captured original Recycle Bin `DefaultIcon` state, kinds preserved so restore writes
/// byte-identical values (REG_SZ vs REG_EXPAND_SZ, unexpanded `%SystemRoot%`).
///
/// `key_existed` refers ONLY to the PER-USER (HKCU) override key — the one apply writes and restore
/// tears down. It drives restore unambiguously: `true` → rewrite these exact values; `false` →
/// REMOVE the per-user key we created (never rewrite). The `default`/`empty`/`full` fields serve a
/// SECOND, independent purpose: they are the EFFECTIVE current values for source extraction — read
/// from the per-user key when it exists, else from the machine (HKCR) fallback. So the
/// `key_existed:false` + present-values shape is NOT a contradiction (audit A1-🟠 二次核实: it is the
/// legitimate machine-default fallback — the per-user key is absent yet the shell shows the machine
/// icon, whose values source extraction must still read); restore ignores the values in that case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecycleBinAnchor {
    pub key_existed: bool,
    pub default: Option<RegistryValue>,
    pub empty: Option<RegistryValue>,
    pub full: Option<RegistryValue>,
}

/// One registry string value plus its kind (1 = `REG_SZ`, 2 = `REG_EXPAND_SZ`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryValue {
    pub raw: String,
    pub kind: u32,
}

/// The exact-restore material for one item, discriminated by capture shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "anchor")]
pub enum RestoreAnchor {
    /// `.lnk` and `.url`: restore by replaying the original file bytes verbatim.
    FileBytes {
        #[serde(with = "bytes_base64")]
        bytes: Vec<u8>,
    },
    /// Folder: original folder attributes + optional original `desktop.ini`.
    Folder {
        attributes: u32,
        #[serde(default)]
        desktop_ini: Option<DesktopIniAnchor>,
    },
    /// Loose file wrapped as a styled shortcut.
    RegularFile(WrapperAnchor),
    /// Recycle Bin registry state.
    RecycleBin(RecycleBinAnchor),
    /// Capture failed — recorded so the presence of restore material can be verified before
    /// an apply proceeds (oracle: `restore.captureError`).
    CaptureFailed { reason: String },
}

impl RestoreAnchor {
    /// Whether this anchor actually carries restore material (a failed capture does not).
    /// Mirrors `SnapshotRestoreVerifier.HasRestoreMaterial`: an item without material is
    /// skipped by the planner rather than styled with no way back.
    pub fn has_material(&self) -> bool {
        !matches!(self, RestoreAnchor::CaptureFailed { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_bytes_anchor_round_trips_and_has_material() {
        let anchor = RestoreAnchor::FileBytes { bytes: b"original shortcut".to_vec() };
        assert!(anchor.has_material());
        let json = serde_json::to_string(&anchor).unwrap();
        // Bytes must be base64, not a numeric array (compactness + oracle parity).
        assert!(json.contains("b3JpZ2luYWwgc2hvcnRjdXQ="), "expected base64 blob, got {json}");
        let back: RestoreAnchor = serde_json::from_str(&json).unwrap();
        assert_eq!(anchor, back);
    }

    #[test]
    fn capture_failed_has_no_material() {
        let anchor = RestoreAnchor::CaptureFailed { reason: "locked".into() };
        assert!(!anchor.has_material());
    }

    #[test]
    fn wrapper_anchor_round_trips_with_and_without_prior_wrapper() {
        let with = RestoreAnchor::RegularFile(WrapperAnchor {
            file_attributes: 0x80,
            prior_wrapper: PriorWrapper::Present { content: b"prior lnk".to_vec() },
        });
        let without = RestoreAnchor::RegularFile(WrapperAnchor {
            file_attributes: 0x80,
            prior_wrapper: PriorWrapper::Absent,
        });
        for a in [with, without] {
            let back: RestoreAnchor = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
            assert_eq!(a, back);
        }
    }

    #[test]
    fn present_prior_wrapper_serializes_content_as_base64() {
        let a = RestoreAnchor::RegularFile(WrapperAnchor {
            file_attributes: 0,
            prior_wrapper: PriorWrapper::Present { content: b"prior lnk".to_vec() },
        });
        let json = serde_json::to_string(&a).unwrap();
        // Compact base64 blob, not a numeric byte array.
        assert!(json.contains("cHJpb3IgbG5r"), "expected base64 content, got {json}");
    }

    #[test]
    fn recyclebin_anchor_preserves_value_kinds() {
        let anchor = RestoreAnchor::RecycleBin(RecycleBinAnchor {
            key_existed: false,
            default: Some(RegistryValue { raw: "%SystemRoot%\\System32\\imageres.dll,-54".into(), kind: 2 }),
            empty: None,
            full: None,
        });
        let back: RestoreAnchor = serde_json::from_str(&serde_json::to_string(&anchor).unwrap()).unwrap();
        assert_eq!(anchor, back);
    }

    #[test]
    fn empty_file_bytes_anchor_round_trips() {
        // A 0-byte original (e.g. an empty `.url`) is a valid anchor, not an error.
        let anchor = RestoreAnchor::FileBytes { bytes: Vec::new() };
        assert!(anchor.has_material());
        let back: RestoreAnchor = serde_json::from_str(&serde_json::to_string(&anchor).unwrap()).unwrap();
        assert_eq!(anchor, back);
    }

    #[test]
    fn folder_anchor_round_trips_with_and_without_desktop_ini() {
        let with = RestoreAnchor::Folder {
            attributes: 0x10,
            desktop_ini: Some(DesktopIniAnchor { content: b"[.ShellClassInfo]\r\n".to_vec(), attributes: 0x6 }),
        };
        let without = RestoreAnchor::Folder { attributes: 0x10, desktop_ini: None };
        for anchor in [with, without] {
            assert!(anchor.has_material());
            let back: RestoreAnchor =
                serde_json::from_str(&serde_json::to_string(&anchor).unwrap()).unwrap();
            assert_eq!(anchor, back);
        }
    }

    #[test]
    fn has_material_is_true_for_every_real_variant() {
        let variants = [
            RestoreAnchor::FileBytes { bytes: b"x".to_vec() },
            RestoreAnchor::Folder { attributes: 0, desktop_ini: None },
            RestoreAnchor::RegularFile(WrapperAnchor { file_attributes: 0, prior_wrapper: PriorWrapper::Absent }),
            RestoreAnchor::RecycleBin(RecycleBinAnchor { key_existed: false, default: None, empty: None, full: None }),
        ];
        for v in variants {
            assert!(v.has_material(), "{v:?} should carry restore material");
        }
        assert!(!RestoreAnchor::CaptureFailed { reason: "locked".into() }.has_material());
    }
}
