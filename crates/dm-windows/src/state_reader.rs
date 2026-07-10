//! The filesystem-backed [`ItemStateReader`] for file items (`.lnk`/`.url`/loose files).
//!
//! The CAS fingerprint is `SHA-256` over the whole file (mirroring the oracle's `SequenceEqual`
//! preflight), and the restore anchor is the file's original bytes (oracle:
//! `RestoreMetadataCollector.CaptureShortcut`/`CaptureUrlShortcut`). Folder / Recycle-Bin /
//! system state (attributes + registry) is captured by the M4 adapters, not here.

use dm_domain::{
    Fingerprint, ItemKind, ItemStateReader, ItemTarget, PortError, PortResult, RestoreAnchor,
};

/// Reads fingerprints and anchors for file-backed items via plain filesystem I/O (no COM).
pub struct FsStateReader;

impl ItemStateReader for FsStateReader {
    /// [WINDOWS-VERIFY] runtime (path semantics).
    fn read_fingerprint(&self, target: &ItemTarget) -> PortResult<Fingerprint> {
        match target.kind {
            ItemKind::Shortcut | ItemKind::UrlShortcut | ItemKind::RegularFile => {
                Ok(Fingerprint::of_bytes(&read_bytes(&target.path)?))
            }
            other => Err(PortError::Unsupported(format!(
                "fingerprint for {other:?} is captured by the M4 adapter"
            ))),
        }
    }

    /// [WINDOWS-VERIFY] runtime.
    fn capture_anchor(&self, target: &ItemTarget) -> PortResult<RestoreAnchor> {
        match target.kind {
            ItemKind::Shortcut | ItemKind::UrlShortcut => {
                Ok(RestoreAnchor::FileBytes { bytes: read_bytes(&target.path)? })
            }
            other => Err(PortError::Unsupported(format!(
                "anchor for {other:?} is captured by the M4 adapter"
            ))),
        }
    }
}

fn read_bytes(path: &str) -> PortResult<Vec<u8>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(PortError::NotFound(path.to_string()))
        }
        Err(e) => Err(PortError::Io(e.to_string())),
    }
}
