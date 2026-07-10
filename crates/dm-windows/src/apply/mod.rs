//! The [`IconApplier`] adapter: dispatches an apply/restore to the per-kind writer, marshalling
//! the mutating COM work onto the STA thread. Ported from
//! `DeskMakeover.Shell/DesktopIconOperationFactory.cs` (kind dispatch) unified behind the port.
//!
//! M3 covers `.lnk` shortcuts (the vertical slice); `.url`/folder/file/Recycle-Bin land in M4.

mod shortcut;

use std::sync::Arc;

use dm_domain::{AssetRef, IconApplier, ItemKind, ItemTarget, PortError, PortResult, RestoreAnchor};

use crate::com::StaExecutor;

/// Applies generated icons to desktop items and restores them from anchors.
pub struct WindowsIconApplier {
    exec: Arc<StaExecutor>,
}

impl WindowsIconApplier {
    pub fn new(exec: Arc<StaExecutor>) -> Self {
        Self { exec }
    }
}

impl IconApplier for WindowsIconApplier {
    fn apply(&self, target: &ItemTarget, asset: &AssetRef) -> PortResult<()> {
        match target.kind {
            ItemKind::Shortcut => {
                // Owned copies cross to the STA thread; the COM object never leaves it.
                let shortcut_path = target.path.clone();
                let icon_path = asset.path.clone();
                self.exec.run(move || shortcut::apply(&shortcut_path, &icon_path))?
            }
            other => Err(PortError::Unsupported(format!("apply for {other:?} lands in M4"))),
        }
    }

    fn restore(&self, target: &ItemTarget, anchor: &RestoreAnchor) -> PortResult<()> {
        match anchor {
            // Restoring a shortcut/url is a plain byte replay — no COM, no STA thread needed
            // (oracle: `RestoreOriginalContent` writes the captured bytes back).
            RestoreAnchor::FileBytes { bytes } => shortcut::restore_bytes(&target.path, bytes),
            other => Err(PortError::Unsupported(format!(
                "restore for {other:?} lands in M4"
            ))),
        }
    }
}
