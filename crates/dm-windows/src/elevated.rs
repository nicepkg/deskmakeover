//! The client side of the elevated desktop-item verb pair (`apply|restore-desktop-items`): stage a
//! batch manifest, verify the helper's signature, launch `dm-elevated` with the `runas` verb (ONE
//! UAC), wait, and map its exit to an [`ElevatedOutcome`]. The privileged work itself is in the
//! `dm-elevated` crate; this is the unelevated staging + invocation only.
//!
//! [WINDOWS-VERIFY]: the live COM icon-location read + `runas` + UAC + exit mapping are proven on the
//! signed on-box build. The manifest text generation is host-testable and unit-tested below.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use dm_domain::{
    ElevatedApplyItem, ElevatedIconApplier, ElevatedOutcome, ElevatedRestoreItem, Fingerprint,
    ItemKind, PortError, PortResult,
};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};

use crate::cmdline::quote_arg;
use crate::fingerprint_surface::expected_after_apply;
use crate::session_channel::{SessionElevated, SessionSend};
use crate::signing::PinnedHelper;

const MANIFEST_HEADER: &str = "dm-desktop-items\t1";

/// Invokes the elevated helper to style / revert privileged shared desktop items. No COM: each row's
/// compare-and-swap anchor is the scan-observed icon location the operations layer threads in
/// (`ElevatedApplyItem::expect_icon`), never a re-read (§P1-1), so the helper writes only when the
/// live target still matches what the app decided to style.
pub struct WindowsElevatedIconApplier {
    helper_path: PathBuf,
    /// Where the (untrusted) manifest + staged original bytes are written — a LOCAL-FIXED, app-owned
    /// dir the helper reads capped. The helper independently re-confirms every target + icon, so this
    /// staging area being user-writable is not a trust boundary (the manifest is untrusted input).
    staging_dir: PathBuf,
    /// The shared session-scoped elevated channel (one UAC per app launch). Tried first; on an
    /// establish failure `run_verb` falls back to a per-op `runas` so elevation never breaks.
    session: Arc<SessionElevated>,
}

impl WindowsElevatedIconApplier {
    pub fn new(helper_path: PathBuf, staging_dir: PathBuf, session: Arc<SessionElevated>) -> Self {
        Self { helper_path, staging_dir, session }
    }

    /// Writes the batch manifest (untrusted input the helper re-validates) to the staging dir.
    fn write_manifest(&self, tag: &str, rows: &[String]) -> PortResult<PathBuf> {
        std::fs::create_dir_all(&self.staging_dir)
            .map_err(|e| PortError::Io(format!("create elevated staging dir: {e}")))?;
        let path = self.staging_dir.join(format!("{tag}-manifest.txt"));
        let mut text = String::from(MANIFEST_HEADER);
        for row in rows {
            text.push('\n');
            text.push_str(row);
        }
        std::fs::write(&path, text)
            .map_err(|e| PortError::Io(format!("write elevated manifest: {e}")))?;
        Ok(path)
    }

    /// Signature-gates the helper (A1/C3) then launches it with `verb --manifest <path>` via `runas`.
    /// The gate PINS the on-disk helper with a deny-write / deny-delete handle held ACROSS the launch,
    /// so the verified image cannot be swapped between the check and `ShellExecuteExW` (§P1-4 TOCTOU).
    fn run_verb(&self, verb: &str, manifest: &Path) -> PortResult<ElevatedOutcome> {
        // Session path FIRST (one UAC per app launch). On an establish failure fall back to a
        // per-op `runas` so elevation degrades gracefully, never breaks (owner 2026-07-17).
        let manifest_str = manifest.to_string_lossy();
        match self.session.send(&[verb, "--manifest", &manifest_str]) {
            Ok(SessionSend::Declined) => return Ok(ElevatedOutcome::Declined),
            Ok(SessionSend::Ran { code: 0, .. }) => return Ok(ElevatedOutcome::Applied),
            Ok(SessionSend::Ran { code, message }) => {
                return Ok(ElevatedOutcome::Failed(format!("{}: {message}", classify_helper_exit(code as u32))))
            }
            // Expected while the session path is gated off; a genuine establish failure with it
            // enabled is still captured, just at debug (the per-op fallback below is the safe path).
            Err(e) => log::debug!("elevated session unavailable ({e}); using per-op runas"),
        }
        // Fallback: A1/C3 pin + verify held across the launch (§P1-4 TOCTOU).
        let _pin = PinnedHelper::open_verified(&self.helper_path)?;
        let params = format!("{verb} --manifest {}", quote_arg(&manifest_str));
        self.run_helper(&params)
    }

    /// Launches `dm-elevated` elevated (`runas`, one UAC), waits, and maps the exit code. Mirrors
    /// [`crate::WindowsOverlayControl`]'s helper invocation: exit 0 → Applied, a UAC cancel
    /// (`ERROR_CANCELLED`) → Declined, any other exit → Failed. [WINDOWS-VERIFY] runtime.
    fn run_helper(&self, params: &str) -> PortResult<ElevatedOutcome> {
        let verb = HSTRING::from("runas");
        let file = HSTRING::from(self.helper_path.as_os_str());
        let parameters = HSTRING::from(params);

        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS,
            lpVerb: PCWSTR(verb.as_ptr()),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(parameters.as_ptr()),
            nShow: 0, // SW_HIDE — the console helper needs no window; UAC still prompts.
            ..Default::default()
        };

        // SAFETY: `info` is fully initialized and lives across the call; the wide-string buffers it
        // points at (`verb`/`file`/`parameters`) outlive the call.
        let launched = unsafe { ShellExecuteExW(&mut info) };
        if let Err(e) = launched {
            // A UAC cancellation surfaces as ERROR_CANCELLED → the user declined, not a failure.
            return if e.code() == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0) {
                Ok(ElevatedOutcome::Declined)
            } else {
                Err(PortError::Com(format!("ShellExecuteEx runas failed: {e}")))
            };
        }

        // SAFETY: `SEE_MASK_NOCLOSEPROCESS` populated `hProcess`; wait, read the exit code, then close
        // the handle exactly once.
        unsafe {
            let wait = WaitForSingleObject(info.hProcess, u32::MAX);
            if wait != WAIT_OBJECT_0 {
                let _ = CloseHandle(info.hProcess);
                return Err(PortError::Com(format!(
                    "WaitForSingleObject on the elevated helper did not complete (0x{:08X})",
                    wait.0
                )));
            }
            let mut exit_code = 0u32;
            let read = GetExitCodeProcess(info.hProcess, &mut exit_code);
            let _ = CloseHandle(info.hProcess);
            match read {
                Ok(()) if exit_code == 0 => Ok(ElevatedOutcome::Applied),
                // ERROR_CANCELLED can also surface as an exit code (not just a launch error) on some
                // UAC-cancel paths — treat it as Declined, not a failure.
                Ok(()) if exit_code == ERROR_CANCELLED.0 => Ok(ElevatedOutcome::Declined),
                Ok(()) => Ok(ElevatedOutcome::Failed(classify_helper_exit(exit_code))),
                Err(e) => Err(PortError::Com(format!("GetExitCodeProcess failed: {e}"))),
            }
        }
    }
}

/// Turn the helper's CLASSIFIED exit code into a human reason (the only failure channel across
/// `runas` — no stderr pipe, and a written report would be a caller-controlled elevated write,
/// codex 2026-07-17 P1). Mirrors `dm_elevated::desktop_items`'s taxonomy; keeps the raw code so an
/// unrecognised value is still diagnosable.
fn classify_helper_exit(code: u32) -> String {
    match code {
        10 => "a shared-desktop shortcut changed since the scan — rescan and retry (exit 10)".to_string(),
        11 => "access denied writing a shared-desktop item (exit 11)".to_string(),
        12 => "the helper rejected the request as invalid or unsupported (exit 12)".to_string(),
        other => format!("helper exit code {other}"),
    }
}

/// The manifest kind string for an elevatable item. v1 is the real `.lnk` kinds (Shortcut /
/// AppxShortcut — the operations layer's `is_elevatable_kind` gate), both written via COM
/// `SetIconLocation`, so both map to `shortcut`.
fn manifest_kind(kind: ItemKind) -> PortResult<&'static str> {
    match kind {
        ItemKind::Shortcut | ItemKind::AppxShortcut => Ok("shortcut"),
        other => Err(PortError::Unsupported(format!("elevated manifest kind for {other:?}"))),
    }
}

/// Builds one apply manifest row. Kept free + pure so the manifest text is host-testable.
fn apply_row(kind: &str, target: &str, icon: &str, expect_icon: &str, expect_index: i32) -> String {
    format!("{kind}\t{target}\t{icon}\t0\t{expect_icon}\t{expect_index}")
}

/// Builds one restore manifest row (`applied_icon` is the CAS anchor — only revert an item still
/// wearing the icon WE applied).
fn restore_row(kind: &str, target: &str, original: &str, applied_icon: &str) -> String {
    format!("{kind}\t{target}\t{original}\t{applied_icon}\t0")
}

impl ElevatedIconApplier for WindowsElevatedIconApplier {
    /// Pure derivation — the styled surface `expected_after_apply` produces, no COM / write / UAC.
    fn plan(&self, items: &[ElevatedApplyItem]) -> PortResult<Vec<Fingerprint>> {
        Ok(items
            .iter()
            .map(|it| {
                expected_after_apply(it.target.kind, &it.target.path, &it.asset_path, None)
                    .fingerprint()
            })
            .collect())
    }

    fn apply(&self, items: &[ElevatedApplyItem]) -> PortResult<ElevatedOutcome> {
        if items.is_empty() {
            return Ok(ElevatedOutcome::Applied);
        }
        let mut rows = Vec::with_capacity(items.len());
        for it in items {
            let kind = manifest_kind(it.target.kind)?;
            // The CAS anchor is the SCAN-observed icon the operations layer threaded in — NEVER a
            // re-read (§P1-1), so the helper refuses to clobber a value the preflight did not accept.
            rows.push(apply_row(kind, &it.target.path, &it.asset_path, &it.expect_icon, it.expect_index));
        }
        let manifest = self.write_manifest("apply", &rows)?;
        self.run_verb("apply-desktop-items", &manifest)
    }

    fn restore(&self, items: &[ElevatedRestoreItem]) -> PortResult<ElevatedOutcome> {
        if items.is_empty() {
            return Ok(ElevatedOutcome::Applied);
        }
        let mut rows = Vec::with_capacity(items.len());
        for (i, it) in items.iter().enumerate() {
            let kind = manifest_kind(it.target.kind)?;
            // Stage the captured original bytes to a local-fixed file the helper reads (capped).
            std::fs::create_dir_all(&self.staging_dir)
                .map_err(|e| PortError::Io(format!("create elevated staging dir: {e}")))?;
            let staged = self.staging_dir.join(format!("restore-orig-{i}.bin"));
            std::fs::write(&staged, &it.original_bytes)
                .map_err(|e| PortError::Io(format!("stage original bytes: {e}")))?;
            rows.push(restore_row(kind, &it.target.path, &staged.to_string_lossy(), &it.applied_icon));
        }
        let manifest = self.write_manifest("restore", &rows)?;
        self.run_verb("restore-desktop-items", &manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_row_matches_the_helper_manifest_grammar() {
        // `kind \t target \t icon \t index \t expect_icon \t expect_index` (6 fields), index always 0.
        let row = apply_row("shortcut", r"C:\Users\Public\Desktop\Chrome.lnk", r"C:\a\s.ico", r"C:\chrome.exe", 0);
        let parts: Vec<&str> = row.split('\t').collect();
        assert_eq!(parts.len(), 6);
        assert_eq!(parts[0], "shortcut");
        assert_eq!(parts[3], "0", "apply always writes icon index 0");
        assert_eq!(parts[4], r"C:\chrome.exe");
    }

    #[test]
    fn an_iconless_target_yields_an_empty_expect_icon_field() {
        // A target with no current icon location (`("", 0)`) produces an empty expect_icon — the two
        // adjacent tabs keep the field count at 6, and the helper's `"" == ""` CAS matches it.
        let row = apply_row("shortcut", r"C:\Users\Public\Desktop\Bare.lnk", r"C:\a\s.ico", "", 0);
        let parts: Vec<&str> = row.split('\t').collect();
        assert_eq!(parts.len(), 6);
        assert_eq!(parts[4], "");
    }

    #[test]
    fn restore_row_matches_the_helper_manifest_grammar() {
        // `kind \t target \t original \t expect_icon \t expect_index` (5 fields).
        let row = restore_row("shortcut", r"C:\Users\Public\Desktop\Chrome.lnk", r"C:\stage\orig-0.bin", r"C:\a\s.ico");
        let parts: Vec<&str> = row.split('\t').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[2], r"C:\stage\orig-0.bin");
        assert_eq!(parts[3], r"C:\a\s.ico", "restore CAS anchor is the icon we applied");
    }

    #[test]
    fn only_the_lnk_kinds_are_manifest_writable() {
        assert_eq!(manifest_kind(ItemKind::Shortcut).unwrap(), "shortcut");
        assert_eq!(manifest_kind(ItemKind::AppxShortcut).unwrap(), "shortcut");
        for k in [ItemKind::UrlShortcut, ItemKind::Folder, ItemKind::RegularFile, ItemKind::System, ItemKind::RecycleBin] {
            assert!(manifest_kind(k).is_err(), "{k:?} is not an elevated manifest kind in v1");
        }
    }
}
