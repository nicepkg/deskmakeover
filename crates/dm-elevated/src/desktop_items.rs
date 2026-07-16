//! The `apply|restore-desktop-items` batch: read the UNTRUSTED manifest, independently validate
//! every target (the LPE gate), then apply/restore ATOMICALLY — any failure rolls back everything
//! already written, so a half-elevated desktop can never result.
//!
//! The orchestration (validate → compare-and-swap → write → rollback) is pure over a
//! [`DesktopWriter`] port, host-tested with a fake. The real Windows COM/FS writer
//! ([`windows_writer`]) reuses [`crate::guards`] for confinement and is [WINDOWS-VERIFY].

use crate::manifest::{ApplyItem, Kind, RestoreItem};

/// Per-file operations the batch needs. Every method names a single target; NONE trusts the
/// manifest — `confirm` is the security gate the orchestration runs before any write.
pub trait DesktopWriter {
    /// LPE gate: prove `target` is a real local file under a privileged root (Public desktop /
    /// ProgramData) that the unelevated app legitimately could not write. Refuse anything else.
    fn confirm_target(&self, target: &str, kind: Kind) -> Result<(), String>;
    /// Validate the baked ICO source (a real, capped .ico on a local fixed disk).
    fn confirm_icon(&self, icon: &str) -> Result<(), String>;
    /// The target's CURRENT icon location (path, index) — the compare-and-swap read.
    fn read_icon(&self, target: &str, kind: Kind) -> Result<(String, i32), String>;
    /// Capture the target's raw bytes (held in memory to roll back on a later failure).
    fn read_bytes(&self, target: &str) -> Result<Vec<u8>, String>;
    /// Read a capped file of original bytes staged by the app (for restore).
    fn read_staged(&self, path: &str) -> Result<Vec<u8>, String>;
    /// Point the target's icon at (icon, index) — the styling write.
    fn set_icon(&self, target: &str, kind: Kind, icon: &str, index: i32) -> Result<(), String>;
    /// Write raw bytes back over the target (rollback, or restore from a captured original).
    fn write_bytes(&self, target: &str, kind: Kind, bytes: &[u8]) -> Result<(), String>;
}

/// Compare-and-swap: does the live icon location match what we expect? Case-insensitive path
/// (Windows) plus an exact index — the same surface the in-process driver treats as a `.lnk`'s
/// styleable identity.
fn icon_matches(cur: &(String, i32), expect_path: &str, expect_index: i32) -> bool {
    cur.1 == expect_index && cur.0.eq_ignore_ascii_case(expect_path)
}

/// A written item we can undo: its target, kind, and the ORIGINAL bytes captured before the write.
struct Undo<'a> {
    target: &'a str,
    kind: Kind,
    original: Vec<u8>,
}

fn rollback(written: &[Undo<'_>], writer: &dyn DesktopWriter) {
    // LIFO, best-effort: a rollback write that itself fails cannot abort the unwind of the others.
    for u in written.iter().rev() {
        let _ = writer.write_bytes(u.target, u.kind, &u.original);
    }
}

/// Apply the whole batch atomically. Returns the number of targets styled. ALL-OR-NOTHING (v1): a
/// compare-and-swap miss (an item changed since the app scanned it) or a write fault rolls back
/// every prior write and fails, so the desktop is never left half-elevated. Per-item skip reporting
/// is a follow-up (needs a results back-channel; today the client only sees the exit code).
pub fn apply(items: &[ApplyItem], writer: &dyn DesktopWriter) -> Result<usize, String> {
    // Phase 0 — validate EVERY target + icon before touching anything (fail-closed).
    for it in items {
        writer.confirm_target(&it.target, it.kind)?;
        writer.confirm_icon(&it.icon)?;
    }
    // Phase 1 — CAS + write, capturing originals so any later failure fully unwinds.
    let mut written: Vec<Undo<'_>> = Vec::with_capacity(items.len());
    for it in items {
        let cur = match writer.read_icon(&it.target, it.kind) {
            Ok(cur) => cur,
            Err(e) => {
                rollback(&written, writer);
                return Err(format!("read failed for {}: {e}", it.target));
            }
        };
        if !icon_matches(&cur, &it.expect_icon, it.expect_index) {
            rollback(&written, writer);
            return Err(format!("target changed since scan, refusing to clobber: {}", it.target));
        }
        let original = match writer.read_bytes(&it.target) {
            Ok(b) => b,
            Err(e) => {
                rollback(&written, writer);
                return Err(format!("capture failed for {}: {e}", it.target));
            }
        };
        if let Err(e) = writer.set_icon(&it.target, it.kind, &it.icon, it.index) {
            rollback(&written, writer);
            return Err(format!("write failed for {}: {e}", it.target));
        }
        written.push(Undo { target: &it.target, kind: it.kind, original });
    }
    Ok(written.len())
}

/// Restore the whole batch to captured originals. Never clobbers an item the user changed since our
/// apply: restore ONLY when the live icon location is still the one we applied (`expect_icon`).
pub fn restore(items: &[RestoreItem], writer: &dyn DesktopWriter) -> Result<usize, String> {
    for it in items {
        writer.confirm_target(&it.target, it.kind)?;
    }
    let mut done: Vec<Undo<'_>> = Vec::with_capacity(items.len());
    for it in items {
        let original = match writer.read_staged(&it.original) {
            Ok(b) => b,
            Err(e) => {
                rollback(&done, writer);
                return Err(format!("staged original unreadable for {}: {e}", it.target));
            }
        };
        let cur = match writer.read_icon(&it.target, it.kind) {
            Ok(cur) => cur,
            Err(e) => {
                rollback(&done, writer);
                return Err(format!("read failed for {}: {e}", it.target));
            }
        };
        if !icon_matches(&cur, &it.expect_icon, it.expect_index) {
            // The user re-styled it themselves since we applied — theirs now, leave it. Skipping is
            // safe here (unlike apply) because a restore that does nothing changes nothing; roll back
            // the ones we DID restore only on a hard fault, not on a benign skip.
            continue;
        }
        let pre = match writer.read_bytes(&it.target) {
            Ok(b) => b,
            Err(e) => {
                rollback(&done, writer);
                return Err(format!("capture failed for {}: {e}", it.target));
            }
        };
        if let Err(e) = writer.write_bytes(&it.target, it.kind, &original) {
            rollback(&done, writer);
            return Err(format!("restore failed for {}: {e}", it.target));
        }
        done.push(Undo { target: &it.target, kind: it.kind, original: pre });
    }
    Ok(done.len())
}

/// Classified exit codes the desktop-items verbs return, so the unelevated launcher can turn an
/// elevated failure into a human reason WITHOUT the helper writing any caller-named file (codex
/// 2026-07-17 P1). Pure over the batch error string, host-tested.
pub const EXIT_GENERIC: u8 = 3;
pub const EXIT_TARGET_CHANGED: u8 = 10;
pub const EXIT_ACCESS_DENIED: u8 = 11;
pub const EXIT_VALIDATION: u8 = 12;

/// Map a batch error message to its exit-code category. Ordered most-specific first: a
/// CAS-mismatch ("changed since scan", the owner-box re-apply case) and an access denial are the
/// two the launcher gives a tailored, actionable message; validation/unsupported-input rejections
/// collapse to one code; anything else is generic.
pub fn classify_failure(err: &str) -> u8 {
    let e = err.to_ascii_lowercase();
    if e.contains("changed since scan") {
        EXIT_TARGET_CHANGED
    } else if e.contains("access denied")
        || e.contains("access is denied")
        || e.contains("os error 5")
        || e.contains("拒绝访问")
    {
        EXIT_ACCESS_DENIED
    } else if e.contains("not a regular file")
        || e.contains("exceeds the")
        || e.contains("header")
        || e.contains("not valid utf-8")
        || e.contains("outside")
        || e.contains("not yet supported")
        || e.contains("is not a")
    {
        EXIT_VALIDATION
    } else {
        EXIT_GENERIC
    }
}

/// Win32 codes meaning "another process transiently holds the file" — mirrors the main app's
/// `dm_windows::durable` set: ERROR_ACCESS_DENIED (5, also what an AV-held replace surfaces as),
/// ERROR_SHARING_VIOLATION (32), ERROR_LOCK_VIOLATION (33), ERROR_UNABLE_TO_REMOVE_REPLACED /
/// _TO_MOVE_REPLACEMENT(_2) (1175–1177). Pure classification, host-tested; consumed by the
/// Windows writer's bounded-backoff retry.
#[cfg_attr(not(windows), allow(dead_code))]
fn is_transient_win32(code: u32) -> bool {
    matches!(code, 5 | 32 | 33 | 1175..=1177)
}

/// Whether a full HRESULT is the transient-hold family. `IPersistFile::Save`/`Load` report
/// sharing conflicts as STRUCTURED-STORAGE HRESULTs — `STG_E_SHAREVIOLATION (0x8003_0020)`,
/// `STG_E_LOCKVIOLATION (0x8003_0021)`, `STG_E_ACCESSDENIED (0x8003_0005)` — NOT as
/// `HRESULT_FROM_WIN32` wrappings, so unwrapping only `0x8007xxxx` missed every real COM sharing
/// violation (codex 2026-07-17 P1). Both families are recognised, plus a bare Win32 code some
/// layers pass through un-wrapped.
#[cfg_attr(not(windows), allow(dead_code))]
fn is_transient_hresult(hr: u32) -> bool {
    match hr {
        // STG_E_ACCESSDENIED | STG_E_SHAREVIOLATION | STG_E_LOCKVIOLATION
        0x8003_0005 | 0x8003_0020 | 0x8003_0021 => true,
        // HRESULT_FROM_WIN32(win32) → FACILITY_WIN32 wrapping.
        hr if hr & 0xFFFF_0000 == 0x8007_0000 => is_transient_win32(hr & 0xFFFF),
        // A bare Win32 code (no failure bit) some layers pass through un-wrapped.
        hr if hr & 0x8000_0000 == 0 => is_transient_win32(hr),
        _ => false,
    }
}

#[cfg(windows)]
pub use windows_writer::run_apply_file;
#[cfg(windows)]
pub use windows_writer::run_restore_file;

/// Non-Windows builds exist only for the host `cargo test` cross-check; the live batch never runs
/// there. main() routes to these so a host build still links.
#[cfg(not(windows))]
pub fn run_apply_file(_manifest: &str) -> Result<(), String> {
    Err("desktop-items apply is only available on Windows".into())
}
#[cfg(not(windows))]
pub fn run_restore_file(_manifest: &str) -> Result<(), String> {
    Err("desktop-items restore is only available on Windows".into())
}

#[cfg(windows)]
mod windows_writer {
    //! The real Windows [`DesktopWriter`]: confinement via [`crate::guards`], `.lnk` icon writes via
    //! `IShellLinkW`/`IPersistFile` (STA COM), raw-byte capture/restore via the filesystem. v1 covers
    //! the `Shortcut` kind (the common Public-desktop item); other file-backed kinds return a clear
    //! error until wired. [WINDOWS-VERIFY]: the live COM writes + UAC path are proven on the box.

    use std::fs;
    use std::path::Path;

    use windows::core::{Interface, HSTRING};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, STGM_READ, STGM_READWRITE,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    use super::{apply, restore, DesktopWriter};
    use crate::guards::{self, MAX_ICO_BYTES};
    use crate::manifest::{parse_apply, parse_restore, Kind};

    /// The Public/All-Users desktop + ProgramData roots the helper confines every write to. The
    /// target side is a handle-resolved canonical `\\?\X:\...` path; `is_under_resolved_root`
    /// component-normalizes BOTH sides (drops the `\\?\` marker, lowercases), so the raw known-folder
    /// strings compare correctly. If a root were itself relocated by a junction, the target's
    /// collapsed path would simply not match → the write is REFUSED (fail-closed), never mis-allowed.
    struct Roots(Vec<String>);

    impl Roots {
        fn resolve() -> Result<Self, String> {
            let mut out = Vec::new();
            for id in [
                &windows::Win32::UI::Shell::FOLDERID_PublicDesktop,
                &windows::Win32::UI::Shell::FOLDERID_ProgramData,
            ] {
                out.push(known_folder(id)?);
            }
            Ok(Roots(out))
        }
    }

    fn known_folder(id: &windows::core::GUID) -> Result<String, String> {
        use windows::Win32::System::Com::CoTaskMemFree;
        use windows::Win32::UI::Shell::{SHGetKnownFolderPath, KF_FLAG_DEFAULT};
        // SAFETY: SHGetKnownFolderPath allocates a PWSTR we own; read then free with CoTaskMemFree.
        unsafe {
            let pwstr = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).map_err(|e| e.to_string())?;
            if pwstr.is_null() {
                return Err("a privileged known folder resolved to null".into());
            }
            let text = pwstr.to_string();
            CoTaskMemFree(Some(pwstr.0 as *const core::ffi::c_void));
            text.map_err(|e| e.to_string())
        }
    }

    /// An STA COM apartment scoped to the batch (RAII so `CoUninitialize` runs on every path).
    struct Apartment;
    impl Apartment {
        fn enter() -> Result<Self, String> {
            // SAFETY: initialise this thread's COM apartment; balanced by Drop.
            let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            if hr.is_err() {
                return Err(format!("CoInitializeEx failed: {hr:?}"));
            }
            Ok(Apartment)
        }
    }
    impl Drop for Apartment {
        fn drop(&mut self) {
            // SAFETY: balances the CoInitializeEx in `enter`.
            unsafe { CoUninitialize() };
        }
    }

    struct WinWriter {
        roots: Vec<String>,
    }

    impl WinWriter {
        fn shortcut_only(kind: Kind) -> Result<(), String> {
            match kind {
                Kind::Shortcut => Ok(()),
                other => Err(format!("elevated write for {:?} is not yet supported", other.as_str())),
            }
        }
    }

    impl DesktopWriter for WinWriter {
        fn confirm_target(&self, target: &str, kind: Kind) -> Result<(), String> {
            Self::shortcut_only(kind)?;
            guards::confirm_target_under_root(Path::new(target), &self.roots)
        }

        fn confirm_icon(&self, icon: &str) -> Result<(), String> {
            guards::validate_overlay_path(icon)?;
            guards::read_capped_ico(Path::new(icon)).map(|_| ())
        }

        fn read_icon(&self, target: &str, _kind: Kind) -> Result<(String, i32), String> {
            // Retried like the writes: the CAS read racing a transient Explorer/AV hold must not
            // fail the one batch the user's UAC consent paid for.
            retry_transient(|| {
                // SAFETY: STA COM; Load READ then GetIconLocation into an owned buffer.
                unsafe {
                    let link: IShellLinkW =
                        CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(com_err)?;
                    let file: IPersistFile = link.cast().map_err(com_err)?;
                    file.Load(&HSTRING::from(target), STGM_READ).map_err(com_err)?;
                    let mut buf = vec![0u16; 1024];
                    let mut index = 0i32;
                    link.GetIconLocation(&mut buf, &mut index).map_err(com_err)?;
                    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                    Ok((String::from_utf16_lossy(&buf[..end]), index))
                }
            })
        }

        fn read_bytes(&self, target: &str) -> Result<Vec<u8>, String> {
            read_capped_file(Path::new(target))
        }

        fn read_staged(&self, path: &str) -> Result<Vec<u8>, String> {
            read_capped_file(Path::new(path))
        }

        fn set_icon(&self, target: &str, _kind: Kind, icon: &str, index: i32) -> Result<(), String> {
            // Retried: writing the shared Desktop races Explorer/AV handles the batch's own shell
            // notifications wake up — the same transient sharing-violation family the unelevated
            // writer outwaits (owner box 2026-07-16, a helper batch failing with no terminal).
            retry_transient(|| {
                // SAFETY: STA COM; Load READWRITE, SetIconLocation, Save in place (fRemember = false).
                unsafe {
                    let link: IShellLinkW =
                        CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(com_err)?;
                    let file: IPersistFile = link.cast().map_err(com_err)?;
                    file.Load(&HSTRING::from(target), STGM_READWRITE).map_err(com_err)?;
                    link.SetIconLocation(&HSTRING::from(icon), index).map_err(com_err)?;
                    file.Save(&HSTRING::from(target), false).map_err(com_err)
                }
            })
        }

        fn write_bytes(&self, target: &str, _kind: Kind, bytes: &[u8]) -> Result<(), String> {
            // A plain overwrite: the original `.lnk` bytes captured before our write. Retried for
            // the same transient-hold family as `set_icon` — a rollback replay racing Explorer must
            // not strand a half-restored batch.
            retry_transient(|| {
                fs::write(target, bytes).map_err(|e| {
                    (
                        e.raw_os_error().is_some_and(|c| super::is_transient_win32(c as u32)),
                        format!("write {target}: {e}"),
                    )
                })
            })
        }
    }

    /// Runs `op` with bounded backoff (~1s total) while it fails with a transient lock. The helper
    /// runs ONE batch per UAC prompt, so failing fast on a 50 ms Explorer hold would waste the
    /// user's consent; outwaiting it is strictly better.
    fn retry_transient<T>(mut op: impl FnMut() -> Result<T, (bool, String)>) -> Result<T, String> {
        const ATTEMPTS: u32 = 8;
        let mut delay_ms = 25u64;
        for attempt in 1..=ATTEMPTS {
            match op() {
                Ok(v) => return Ok(v),
                Err((transient, msg)) => {
                    if !transient || attempt == ATTEMPTS {
                        return Err(msg);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                    delay_ms = (delay_ms * 2).min(200);
                }
            }
        }
        unreachable!("the retry loop always returns")
    }

    /// Maps a COM error into the retry contract: `(is_transient, message)` via
    /// [`super::is_transient_hresult`] (structured-storage STG_E_* AND `HRESULT_FROM_WIN32`
    /// families — codex 2026-07-17 P1).
    fn com_err(e: windows::core::Error) -> (bool, String) {
        (super::is_transient_hresult(e.code().0 as u32), format!("COM error: {e}"))
    }

    /// A capped, local-fixed read of a target/staged file (bounds a hostile size like the ICO guard).
    fn read_capped_file(path: &Path) -> Result<Vec<u8>, String> {
        use std::io::Read;
        let mut file = guards::open_file_no_follow(path)?;
        guards::assert_handle_local_fixed(&file, "desktop-item file")?;
        let meta = file.metadata().map_err(|e| e.to_string())?;
        if !meta.is_file() {
            return Err(format!("not a regular file: {}", path.display()));
        }
        let mut bytes = Vec::new();
        file.by_ref()
            .take(MAX_ICO_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_ICO_BYTES {
            return Err(format!("file exceeds the {MAX_ICO_BYTES}-byte cap: {}", path.display()));
        }
        Ok(bytes)
    }

    fn writer() -> Result<WinWriter, String> {
        Ok(WinWriter { roots: Roots::resolve()?.0 })
    }

    pub fn run_apply_file(manifest: &str) -> Result<(), String> {
        let _com = Apartment::enter()?;
        let text = read_manifest(manifest)?;
        let items = parse_apply(&text)?;
        let w = writer()?;
        apply(&items, &w).map(|_| ())
    }

    pub fn run_restore_file(manifest: &str) -> Result<(), String> {
        let _com = Apartment::enter()?;
        let text = read_manifest(manifest)?;
        let items = parse_restore(&text)?;
        let w = writer()?;
        restore(&items, &w).map(|_| ())
    }

    /// Read the manifest file itself capped + local-fixed (it is untrusted, so bound its size and
    /// refuse a redirected read just like every other file the helper opens). READ-ONLY — the
    /// helper NEVER writes to a caller-named path (the failure reason travels back as a
    /// classified exit code, not a written report; codex 2026-07-17 P1).
    fn read_manifest(path: &str) -> Result<String, String> {
        let bytes = read_capped_file(Path::new(path))?;
        String::from_utf8(bytes).map_err(|_| "manifest is not valid UTF-8".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[test]
    fn classify_failure_maps_each_batch_error_family_to_its_exit_code() {
        // The owner-box re-apply case → a tailored "rescan and retry" code, not generic.
        assert_eq!(
            classify_failure("target changed since scan, refusing to clobber: C:\\...\\Chrome.lnk"),
            EXIT_TARGET_CHANGED
        );
        // Access denials, in the forms the writers surface (COM/os error / localized).
        assert_eq!(classify_failure("write failed for X: COM error: Access is denied. (0x80070005)"), EXIT_ACCESS_DENIED);
        assert_eq!(classify_failure("write X: 拒绝访问。 (os error 5)"), EXIT_ACCESS_DENIED);
        // Validation / unsupported-input rejections collapse to one code.
        assert_eq!(classify_failure("not a regular file: C:\\x"), EXIT_VALIDATION);
        assert_eq!(classify_failure("manifest is not valid UTF-8"), EXIT_VALIDATION);
        assert_eq!(classify_failure("desktop-item target is outside every privileged root: ..."), EXIT_VALIDATION);
        assert_eq!(classify_failure("elevated write for Folder is not yet supported"), EXIT_VALIDATION);
        // Anything unrecognised stays generic.
        assert_eq!(classify_failure("some unexpected internal error"), EXIT_GENERIC);
    }

    #[test]
    fn transient_hresult_covers_the_structured_storage_family_ipersistfile_reports() {
        // codex 2026-07-17 P1: IPersistFile::Save/Load surface sharing conflicts as STG_E_*, not
        // HRESULT_FROM_WIN32 — a classifier that only unwraps 0x8007xxxx retries NONE of them.
        for hr in [0x8003_0020u32, 0x8003_0021, 0x8003_0005] {
            assert!(is_transient_hresult(hr), "STG_E {hr:#010x} is transient");
        }
        // The HRESULT_FROM_WIN32 wrappings of the Win32 sharing family.
        for win32 in [5u32, 32, 33, 1175, 1176, 1177] {
            assert!(is_transient_hresult(0x8007_0000 | win32), "0x8007|{win32} is transient");
        }
        // Bare Win32 codes passed through un-wrapped.
        assert!(is_transient_hresult(32));
        // Real errors are never retried: E_FAIL, STG_E_FILENOTFOUND, wrapped ERROR_FILE_NOT_FOUND,
        // bare ERROR_FILE_NOT_FOUND.
        for hr in [0x8000_4005u32, 0x8003_0002, 0x8007_0002, 2] {
            assert!(!is_transient_hresult(hr), "{hr:#010x} is NOT transient");
        }
    }

    /// A virtual desktop where each target's raw bytes FULLY encode its (icon_location, index) as
    /// `"<icon>\t<index>"` — so a rollback that replays captured bytes restores the OBSERVABLE icon
    /// exactly, the way a real `.lnk` byte-replay does. Records nothing extra; the map is the truth.
    struct FakeWriter {
        files: RefCell<HashMap<String, (String, i32)>>,
        staged: RefCell<HashMap<String, Vec<u8>>>,
        deny_confirm: RefCell<Vec<String>>,
        fail_set: RefCell<Vec<String>>,
    }

    fn encode(icon: &str, index: i32) -> Vec<u8> {
        format!("{icon}\t{index}").into_bytes()
    }
    fn decode(bytes: &[u8]) -> (String, i32) {
        let s = String::from_utf8_lossy(bytes);
        let (icon, idx) = s.split_once('\t').unwrap_or((s.as_ref(), "0"));
        (icon.to_string(), idx.parse().unwrap_or(0))
    }

    impl FakeWriter {
        fn new() -> Self {
            Self {
                files: RefCell::new(HashMap::new()),
                staged: RefCell::new(HashMap::new()),
                deny_confirm: RefCell::new(Vec::new()),
                fail_set: RefCell::new(Vec::new()),
            }
        }
        fn seed(&self, target: &str, icon: &str, index: i32) {
            self.files.borrow_mut().insert(target.into(), (icon.into(), index));
        }
        fn stage(&self, name: &str, icon: &str, index: i32) {
            self.staged.borrow_mut().insert(name.into(), encode(icon, index));
        }
        fn icon_of(&self, target: &str) -> (String, i32) {
            self.files.borrow().get(target).unwrap().clone()
        }
    }

    impl DesktopWriter for FakeWriter {
        fn confirm_target(&self, target: &str, _kind: Kind) -> Result<(), String> {
            if self.deny_confirm.borrow().iter().any(|t| t == target) {
                return Err(format!("confined out: {target}"));
            }
            if self.files.borrow().contains_key(target) {
                Ok(())
            } else {
                Err(format!("no such target: {target}"))
            }
        }
        fn confirm_icon(&self, _icon: &str) -> Result<(), String> {
            Ok(())
        }
        fn read_icon(&self, target: &str, _kind: Kind) -> Result<(String, i32), String> {
            self.files.borrow().get(target).cloned().ok_or_else(|| "gone".into())
        }
        fn read_bytes(&self, target: &str) -> Result<Vec<u8>, String> {
            let f = self.files.borrow();
            let (icon, idx) = f.get(target).ok_or("gone")?;
            Ok(encode(icon, *idx))
        }
        fn read_staged(&self, path: &str) -> Result<Vec<u8>, String> {
            self.staged.borrow().get(path).cloned().ok_or_else(|| "no staged file".into())
        }
        fn set_icon(&self, target: &str, _kind: Kind, icon: &str, index: i32) -> Result<(), String> {
            if self.fail_set.borrow().iter().any(|t| t == target) {
                return Err("access denied".into());
            }
            let mut f = self.files.borrow_mut();
            *f.get_mut(target).ok_or("gone")? = (icon.into(), index);
            Ok(())
        }
        fn write_bytes(&self, target: &str, _kind: Kind, bytes: &[u8]) -> Result<(), String> {
            let mut f = self.files.borrow_mut();
            *f.get_mut(target).ok_or("gone")? = decode(bytes); // byte-replay restores the observable icon
            Ok(())
        }
    }

    fn apply_item(target: &str, icon: &str, expect_icon: &str) -> ApplyItem {
        ApplyItem {
            kind: Kind::Shortcut,
            target: target.into(),
            icon: icon.into(),
            index: 0,
            expect_icon: expect_icon.into(),
            expect_index: 0,
        }
    }

    #[test]
    fn apply_styles_every_confined_target() {
        let w = FakeWriter::new();
        w.seed("A.lnk", "old-a", 0);
        w.seed("B.lnk", "old-b", 0);
        let items = vec![apply_item("A.lnk", "new-a.ico", "old-a"), apply_item("B.lnk", "new-b.ico", "old-b")];
        assert_eq!(apply(&items, &w).unwrap(), 2);
        assert_eq!(w.icon_of("A.lnk"), ("new-a.ico".into(), 0));
        assert_eq!(w.icon_of("B.lnk"), ("new-b.ico".into(), 0));
    }

    #[test]
    fn a_confinement_refusal_fails_the_batch_before_any_write() {
        let w = FakeWriter::new();
        w.seed("A.lnk", "old-a", 0);
        w.seed("EVIL.lnk", "x", 0);
        w.deny_confirm.borrow_mut().push("EVIL.lnk".into());
        let items = vec![apply_item("A.lnk", "new-a.ico", "old-a"), apply_item("EVIL.lnk", "e.ico", "x")];
        assert!(apply(&items, &w).is_err());
        // Phase-0 validation runs before ANY write, so even the legal target A is untouched.
        assert_eq!(w.icon_of("A.lnk"), ("old-a".into(), 0));
    }

    #[test]
    fn a_write_failure_rolls_back_every_prior_write_atomically() {
        let w = FakeWriter::new();
        w.seed("A.lnk", "old-a", 0);
        w.seed("B.lnk", "old-b", 0);
        w.fail_set.borrow_mut().push("B.lnk".into()); // B's write hits access denied
        let items = vec![apply_item("A.lnk", "new-a.ico", "old-a"), apply_item("B.lnk", "new-b.ico", "old-b")];
        assert!(apply(&items, &w).is_err());
        // A was styled then rolled back to its original — a half-elevated desktop is impossible.
        assert_eq!(w.icon_of("A.lnk"), ("old-a".into(), 0), "A must be rolled back");
    }

    #[test]
    fn a_compare_and_swap_miss_never_clobbers_and_unwinds() {
        let w = FakeWriter::new();
        w.seed("A.lnk", "old-a", 0);
        w.seed("B.lnk", "changed", 0); // changed since the app scanned it
        let items = vec![apply_item("A.lnk", "new-a.ico", "old-a"), apply_item("B.lnk", "new-b.ico", "old-b")];
        assert!(apply(&items, &w).is_err(), "a CAS miss refuses the batch");
        assert_eq!(w.icon_of("A.lnk"), ("old-a".into(), 0), "A rolled back, never clobbered");
        assert_eq!(w.icon_of("B.lnk"), ("changed".into(), 0), "B left exactly as the user has it");
    }

    #[test]
    fn restore_returns_our_targets_and_skips_a_user_re_edit() {
        let w = FakeWriter::new();
        // A is still our applied style → restore it; B the user re-styled → skip (theirs now).
        w.seed("A.lnk", "ours-a", 0);
        w.seed("B.lnk", "user-b", 0);
        w.stage("orig-a.bin", "orig-a", 0);
        w.stage("orig-b.bin", "orig-b", 0);
        let items = vec![
            RestoreItem { kind: Kind::Shortcut, target: "A.lnk".into(), original: "orig-a.bin".into(), expect_icon: "ours-a".into(), expect_index: 0 },
            RestoreItem { kind: Kind::Shortcut, target: "B.lnk".into(), original: "orig-b.bin".into(), expect_icon: "ours-b".into(), expect_index: 0 },
        ];
        assert_eq!(restore(&items, &w).unwrap(), 1, "only A, still ours, is restored");
        assert_eq!(w.icon_of("A.lnk"), ("orig-a".into(), 0), "A restored to its original");
        assert_eq!(w.icon_of("B.lnk"), ("user-b".into(), 0), "the user's re-edit is left alone");
    }
}
