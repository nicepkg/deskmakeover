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
            // SAFETY: STA COM; Load READ then GetIconLocation into an owned buffer.
            unsafe {
                let link: IShellLinkW =
                    CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(com)?;
                let file: IPersistFile = link.cast().map_err(com)?;
                file.Load(&HSTRING::from(target), STGM_READ).map_err(com)?;
                let mut buf = vec![0u16; 1024];
                let mut index = 0i32;
                link.GetIconLocation(&mut buf, &mut index).map_err(com)?;
                let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                Ok((String::from_utf16_lossy(&buf[..end]), index))
            }
        }

        fn read_bytes(&self, target: &str) -> Result<Vec<u8>, String> {
            read_capped_file(Path::new(target))
        }

        fn read_staged(&self, path: &str) -> Result<Vec<u8>, String> {
            read_capped_file(Path::new(path))
        }

        fn set_icon(&self, target: &str, _kind: Kind, icon: &str, index: i32) -> Result<(), String> {
            // SAFETY: STA COM; Load READWRITE, SetIconLocation, Save in place (fRemember = false).
            unsafe {
                let link: IShellLinkW =
                    CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(com)?;
                let file: IPersistFile = link.cast().map_err(com)?;
                file.Load(&HSTRING::from(target), STGM_READWRITE).map_err(com)?;
                link.SetIconLocation(&HSTRING::from(icon), index).map_err(com)?;
                file.Save(&HSTRING::from(target), false).map_err(com)
            }
        }

        fn write_bytes(&self, target: &str, _kind: Kind, bytes: &[u8]) -> Result<(), String> {
            // A plain overwrite: the original `.lnk` bytes captured before our write.
            fs::write(target, bytes).map_err(|e| format!("write {target}: {e}"))
        }
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

    fn com(e: windows::core::Error) -> String {
        format!("COM error: {e}")
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
    /// refuse a redirected read just like every other file the helper opens).
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
