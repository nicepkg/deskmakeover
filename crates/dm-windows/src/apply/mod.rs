//! The [`IconApplier`] adapter: dispatches apply/restore to the per-kind writer, marshalling the
//! COM-bearing work (`.lnk` create/edit) onto the STA thread and doing pure filesystem/registry
//! work inline. Ported from `DeskMakeover.Shell/DesktopIconOperationFactory.cs` (kind dispatch).

mod folder;
mod reg_icon;
mod shortcut;
mod url_shortcut;

pub(crate) mod drive;
pub(crate) mod file_wrapper;
pub(crate) mod filetype;
pub(crate) mod recyclebin;
pub(crate) mod system;

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
            // A Store/MSIX wrapper (Task 11): present → the AUMID-preserving IconLocation rewrite;
            // absent → CREATE the wrapper at the prospective path (AppsFolder lookup by stem).
            ItemKind::AppxShortcut => self.exec.run(move || shortcut::apply_appx(&path, &icon))??,
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
            // System virtual items (This PC / Network / User Files / Control Panel) style via each
            // namespace CLSID's per-user `DefaultIcon` value (spec 06 §6) — the single-value sibling
            // of the Recycle Bin. The CLSID rides the item's parsing path (`::{GUID}`).
            // [WINDOWS-VERIFY] runtime.
            ItemKind::System => system::apply(&system::parse_clsid(&target.path)?, &icon)?,
            // A fixed drive styles via its per-user `DefaultIcon` registry value (ADR-0004 D7) —
            // the letter rides the item's own path (`C:\` → `C`). Registry write, no COM.
            ItemKind::Drive => drive::apply(&drive::letter_from_path(&target.path)?, &icon)?,
            // A file TYPE styles via its ProgId's per-user `DefaultIcon` (ADR-0004 D7 amendment
            // 2026-10-16) — the ProgId rides the item's own path and is validated as one key
            // component first. Exactly ONE value is written, never a verb. Registry write, no COM.
            ItemKind::FileType => filetype::apply(&filetype::prog_id_from_path(&target.path)?, &icon)?,
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
            // A virtual AppsFolder entry's original state was "no wrapper exists" (Task 11): the
            // restore removes the wrapper apply created — the file_wrapper Absent→remove_file
            // posture. The driver's CAS (live == last_applied) gates this, so the file removed is
            // the verified styled wrapper, never a user's own shortcut.
            RestoreAnchor::FileAbsent => shortcut::remove(&target.path),
            RestoreAnchor::Folder { attributes, desktop_ini } => {
                folder::restore(&target.path, *attributes, desktop_ini.as_ref())
            }
            RestoreAnchor::RegularFile(wrapper) => file_wrapper::restore(&target.path, wrapper),
            RestoreAnchor::RecycleBin(state) => recyclebin::restore(state),
            RestoreAnchor::SystemIcon(state) => {
                // Defence-in-depth against a corrupt/stale ledger (codex R2 D-4): `system::restore`
                // interpolates the anchor's `clsid` straight into an HKCU key path, so it must be a
                // canonical GUID AND the CLSID of the item we were actually asked to restore. Re-derive
                // the target's CLSID (this also rejects a non-System target and any injection-bearing
                // string) and require an EXACT match — otherwise a mismatched pairing (a This-PC target
                // carrying a Network anchor) or a poisoned anchor string would silently mutate the WRONG
                // key and report success while the real target stays styled. Both sides are the
                // `parse_clsid`-normalised form (state_reader captures via `parse_clsid`), so equality is
                // exact.
                let target_clsid = system::parse_clsid(&target.path)?;
                if target_clsid != state.clsid {
                    return Err(PortError::Unsupported(format!(
                        "System restore CLSID mismatch: target {target_clsid} vs anchor {}",
                        state.clsid
                    )));
                }
                system::restore(state)
            }
            RestoreAnchor::Drive(anchor) => {
                // Defence-in-depth (same discipline as the System CLSID check): the anchor's
                // letter is interpolated into an HKCU per-drive key path, so it must match the
                // drive the target path names — otherwise a mismatched pairing would mutate the
                // WRONG drive's key and report success while the real target stays styled.
                let target_letter = drive::letter_from_path(&target.path)?;
                if !target_letter.eq_ignore_ascii_case(&anchor.letter) {
                    return Err(PortError::Unsupported(format!(
                        "Drive restore letter mismatch: target {target_letter} vs anchor {}",
                        anchor.letter
                    )));
                }
                drive::restore(anchor)
            }
            RestoreAnchor::FileType(anchor) => {
                // Defence-in-depth (same discipline as the System CLSID / Drive letter checks):
                // the anchor's ProgId is interpolated into an HKCU `Software\Classes` key path,
                // so it must be BOTH a valid single key component AND the association the target
                // actually names — otherwise a mismatched pairing (a txtfile target carrying an
                // htmlfile anchor) or a poisoned anchor would mutate the WRONG association and
                // report success while the real target stays styled. `filetype::restore`
                // re-validates the shape; this compares identity.
                let target_prog = filetype::prog_id_from_path(&target.path)?;
                if target_prog != anchor.prog_id {
                    return Err(PortError::Unsupported(format!(
                        "FileType restore ProgId mismatch: target {target_prog} vs anchor {}",
                        anchor.prog_id
                    )));
                }
                filetype::restore(anchor)
            }
            RestoreAnchor::CaptureFailed { reason } => {
                Err(PortError::Unsupported(format!("no restore material: {reason}")))
            }
        }
    }
}

