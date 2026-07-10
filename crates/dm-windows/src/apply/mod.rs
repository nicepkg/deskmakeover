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
            // write's own error).
            ItemKind::Shortcut => self.exec.run(move || shortcut::apply(&path, &icon))??,
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
            other => return Err(PortError::Unsupported(format!("apply for {other:?}"))),
        };
        // The expected styleable-surface fingerprint, derived from the ASSET — not a re-read of the
        // just-written state. Built via the SAME host-tested surface logic the reader uses, so the
        // driver's independent read-back can confirm the write matched the request; a COM write
        // that reports success yet leaves the old icon is caught (P1-1). [WINDOWS-VERIFY].
        let empty = assets.empty.as_ref().map(|e| e.path.as_str());
        Ok(fp::expected_after_apply(target.kind, &assets.primary.path, empty).fingerprint())
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

