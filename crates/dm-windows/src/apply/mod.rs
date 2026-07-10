//! The [`IconApplier`] adapter: dispatches apply/restore to the per-kind writer, marshalling the
//! COM-bearing work (`.lnk` create/edit) onto the STA thread and doing pure filesystem/registry
//! work inline. Ported from `DeskMakeover.Shell/DesktopIconOperationFactory.cs` (kind dispatch).

mod folder;
mod shortcut;
mod url_shortcut;

pub(crate) mod file_wrapper;
pub(crate) mod recyclebin;

use std::sync::Arc;

use dm_domain::{
    ApplyAssets, Fingerprint, IconApplier, ItemKind, ItemTarget, PortError, PortResult,
    RestoreAnchor,
};

use crate::com::StaExecutor;
use crate::fingerprint_surface as fp;

/// Applies generated icons to desktop items and restores them from anchors (the full M4 matrix).
pub struct WindowsIconApplier {
    exec: Arc<StaExecutor>,
}

impl WindowsIconApplier {
    pub fn new(exec: Arc<StaExecutor>) -> Self {
        Self { exec }
    }
}

impl IconApplier for WindowsIconApplier {
    fn apply(&self, target: &ItemTarget, assets: &ApplyAssets) -> PortResult<Fingerprint> {
        let path = target.path.clone();
        let icon = assets.primary.path.clone();
        match target.kind {
            // COM writes run on the STA thread (outer `?` = thread-join error, inner `?` = the
            // write's own error). A UWP shortcut's desktop entry is an ordinary `.lnk`, so it styles
            // through the exact same mechanism as a Shortcut (spec 06 §6, P1-12).
            ItemKind::Shortcut | ItemKind::AppxShortcut => {
                self.exec.run(move || shortcut::apply(&path, &icon))??
            }
            ItemKind::RegularFile => self.exec.run(move || file_wrapper::apply(&path, &icon))??,
            // Filesystem / registry writes need no COM.
            ItemKind::UrlShortcut => url_shortcut::apply(&path, &icon, 0)?,
            ItemKind::Folder => folder::apply(&path, &icon)?,
            // The Recycle Bin needs BOTH state icons; reference the EXACT empty ref the driver
            // materialized and verified (P1-14/P2-1), never a locally reconstructed path.
            ItemKind::RecycleBin => {
                let empty = assets.empty.as_ref().ok_or_else(|| {
                    PortError::AssetMissing(format!("Recycle Bin apply needs a paired empty icon for {}", target.path))
                })?;
                recyclebin::apply(&empty.path, &icon)?
            }
            // System virtual items (This PC / Network / User Files / Control Panel) style via the
            // per-user CLSID `DefaultIcon` values (spec 06 §6). The classifier advertises them as
            // styleable (P1-12), but the HKCU CLSID writer + their discovery are a Windows-scoped
            // follow-up. Return an HONEST, labelled pending error — never the generic Unsupported or
            // a panic — so a spec-compliant discovered System item is not silently mis-rejected.
            // Tracked in the wave-2 [WINDOWS-VERIFY] ledger.
            ItemKind::System => {
                return Err(PortError::Unsupported(
                    "[WINDOWS-VERIFY] System DefaultIcon styling is not yet wired (HKCU CLSID writer + discovery pending)".into(),
                ))
            }
            // Only genuinely un-styleable (broken/unreadable) items remain.
            ItemKind::Unsupported => {
                return Err(PortError::Unsupported(format!("apply for {:?}", target.kind)))
            }
        };
        // The expected styleable-surface fingerprint, derived from the ASSET (and the item's own
        // path, for the wrapper target/working-dir) — not a re-read of the just-written state. Built
        // via the SAME host-tested surface logic the reader uses, so the driver's independent
        // read-back can confirm the write matched the request; a COM write that reports success yet
        // leaves the old icon — or a partial write that omits a coupled field (folder READONLY, the
        // wrapper target, a Recycle Bin index) — is caught (P1-1/P1-#1). [WINDOWS-VERIFY].
        let empty = assets.empty.as_ref().map(|e| e.path.as_str());
        Ok(fp::expected_after_apply(target.kind, &target.path, &assets.primary.path, empty).fingerprint())
    }

    fn restore(&self, target: &ItemTarget, anchor: &RestoreAnchor) -> PortResult<()> {
        match anchor {
            // `.lnk`/`.url` restore is a byte replay (oracle `RestoreOriginalContent`) — no COM.
            RestoreAnchor::FileBytes { bytes } => shortcut::restore_bytes(&target.path, bytes),
            RestoreAnchor::Folder { attributes, desktop_ini } => {
                folder::restore(&target.path, *attributes, desktop_ini.as_ref())
            }
            RestoreAnchor::RegularFile(wrapper) => file_wrapper::restore(&target.path, wrapper),
            RestoreAnchor::RecycleBin(state) => recyclebin::restore(state),
            RestoreAnchor::CaptureFailed { reason } => {
                Err(PortError::Unsupported(format!("no restore material: {reason}")))
            }
        }
    }
}

