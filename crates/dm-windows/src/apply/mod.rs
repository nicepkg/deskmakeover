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
    AssetRef, Fingerprint, IconApplier, ItemKind, ItemStateReader, ItemTarget, PortError,
    PortResult, RestoreAnchor,
};

use crate::com::StaExecutor;
use crate::state_reader::WindowsStateReader;

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
    fn apply(&self, target: &ItemTarget, asset: &AssetRef) -> PortResult<Fingerprint> {
        let path = target.path.clone();
        let icon = asset.path.clone();
        match target.kind {
            // COM writes run on the STA thread (outer `?` = thread-join error, inner `?` = the
            // write's own error).
            ItemKind::Shortcut => self.exec.run(move || shortcut::apply(&path, &icon))??,
            ItemKind::RegularFile => self.exec.run(move || file_wrapper::apply(&path, &icon))??,
            // Filesystem / registry writes need no COM.
            ItemKind::UrlShortcut => url_shortcut::apply(&path, &icon, 0)?,
            ItemKind::Folder => folder::apply(&path, &icon)?,
            // The Recycle Bin needs BOTH state icons; the driver has already materialized the
            // paired empty ICO (`paired_empty(icon)`) and verified it exists before this call
            // (P1-14), so the registry write can reference it safely.
            ItemKind::RecycleBin => recyclebin::apply(&paired_empty(&icon), &icon)?,
            other => return Err(PortError::Unsupported(format!("apply for {other:?}"))),
        };
        // Report the achieved styleable-surface fingerprint for THIS asset so the driver can
        // confirm the apply matched the request (P1-4). [WINDOWS-VERIFY] runtime.
        WindowsStateReader.read_fingerprint(target)
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

/// The paired empty-state ICO path for a Recycle Bin full-state asset (`x.ico` → `x-empty.ico`).
fn paired_empty(full_ico: &str) -> String {
    match full_ico.strip_suffix(".ico") {
        Some(stem) => format!("{stem}-empty.ico"),
        None => format!("{full_ico}-empty"),
    }
}
