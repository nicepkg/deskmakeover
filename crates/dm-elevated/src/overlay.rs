//! The privileged overlay verbs (ADR-0021 §4): apply/restore the machine-wide
//! `HKLM ...\Shell Icons\29` shortcut-overlay value. Ported from
//! `ElevatedHelper/OverlayCommands.cs`.
//!
//! Invariants preserved from the oracle:
//! * the pre-DeskMakeover value is snapshotted exactly once (with an explicit `__absent__`
//!   marker) before the first modification, under `%ProgramData%`, so restore never depends on
//!   the caller;
//! * the registry NEVER points at a caller-supplied path — the ICO is validated and copied into
//!   `%ProgramData%` first (LPE guard);
//! * restore rewrites the exact original value (or deletes it when it was absent), zero residue.

use crate::args::Style;

/// Applies the overlay for `style`, using the rendered ICO at `source_file`.
pub fn apply(style: Style, source_file: Option<&str>) -> Result<(), String> {
    #[cfg(windows)]
    {
        windows_impl::apply(style, source_file)
    }
    #[cfg(not(windows))]
    {
        let _ = (style, source_file);
        Err("overlay verbs only run on Windows".to_string())
    }
}

/// Restores the original overlay registry state.
pub fn restore() -> Result<(), String> {
    #[cfg(windows)]
    {
        windows_impl::restore()
    }
    #[cfg(not(windows))]
    {
        Err("overlay verbs only run on Windows".to_string())
    }
}

#[cfg(windows)]
mod windows_impl {
    use std::path::{Path, PathBuf};

    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    use crate::args::Style;
    use crate::guards::read_capped_ico;
    use crate::secure_dir::secure_data_dir;

    const SHELL_ICONS_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Shell Icons";
    const OVERLAY_VALUE: &str = "29";
    const ABSENT_MARKER: &str = "__absent__";
    const STATE_FILE: &str = "overlay-state.txt";

    pub fn apply(style: Style, source_file: Option<&str>) -> Result<(), String> {
        // The data dir is resolved from a known folder and proven admin-owned + non-reparse with a
        // restrictive DACL before we trust anything inside it (P1 LPE fix — see secure_dir).
        let dir = secure_data_dir()?;
        let ico = materialize_ico(&dir, style, source_file)?;

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let key = hklm.create_subkey(SHELL_ICONS_KEY).map_err(io)?.0;

        // Snapshot the pre-DeskMakeover value exactly once, DURABLY, BEFORE touching the registry.
        // The write must be fsync'd first (codex 2026-07-12): a plain `fs::write` that has not
        // reached disk when the registry `set_value` below lands means a power loss loses the
        // snapshot, and the NEXT apply then re-captures OUR OWN value as the "original" — a permanent
        // loss of the true restore anchor. The state file lives in the hardened, admin-only data dir,
        // so a standard user cannot pre-plant it to defeat this guard.
        let state = dir.join(STATE_FILE);
        if !state.exists() {
            let original: String =
                key.get_value(OVERLAY_VALUE).unwrap_or_else(|_| ABSENT_MARKER.to_string());
            // Non-replacing atomic claim: if another elevated apply raced in between the check above
            // and here, our publish fails-closed and does NOT overwrite their real-original snapshot.
            snapshot_once(&dir, STATE_FILE, original.as_bytes())?;
        }

        // Point the registry at the ProgramData copy — NEVER the caller path (LPE guard).
        key.set_value(OVERLAY_VALUE, &format!("{},0", ico.display())).map_err(io)?;
        notify_shell();
        Ok(())
    }

    pub fn restore() -> Result<(), String> {
        let dir = secure_data_dir()?;
        let state = dir.join(STATE_FILE);
        if !state.exists() {
            return Ok(()); // untouched — nothing to restore
        }
        let original = std::fs::read_to_string(&state).map_err(io)?;
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let key = hklm.create_subkey(SHELL_ICONS_KEY).map_err(io)?.0;
        if original == ABSENT_MARKER {
            let _ = key.delete_value(OVERLAY_VALUE); // idempotent if already gone
        } else {
            key.set_value(OVERLAY_VALUE, &original).map_err(io)?;
        }
        std::fs::remove_file(&state).map_err(io)?;
        notify_shell();
        Ok(())
    }

    /// Validates the rendered ICO and copies it into the hardened data `dir`, returning its path.
    ///
    /// `read_capped_ico` opens the caller's `--file` exactly once and does the size cap and the
    /// structural `parse` validation through that single handle, so a swap of the path between a
    /// check and the read cannot smuggle an over-cap file past the cap (TOCTOU). The exact
    /// validated bytes we already hold are written to ProgramData — no second read of `src`.
    ///
    /// NOTE (blind-write divergence): the oracle generated the built-in refined/transparent ICOs
    /// in-process (`OverlayBadgeIconFactory`). Here EVERY style's ICO arrives as a validated
    /// `--file` from the client, which owns the icon core. The LPE guard (validate + copy into
    /// ProgramData) is applied uniformly. [icon-core-need] the client renders the
    /// transparent/refined overlay ICOs. [WINDOWS-VERIFY] registry + refresh behaviour.
    fn materialize_ico(dir: &Path, style: Style, source_file: Option<&str>) -> Result<PathBuf, String> {
        let src = source_file
            .ok_or_else(|| format!("--file (a rendered .ico) is required for the {} overlay", style.as_str()))?;
        let bytes = read_capped_ico(Path::new(src))?;
        // Atomic O_EXCL-temp + fsync + rename, matching the durability discipline the
        // .lnk/.url/desktop.ini writers use: this ICO is referenced live by HKLM Shell Icons and
        // read by Explorer's icon cache, so a crash mid-write must not leave a torn/corrupt icon
        // (ELEV-2), and the temp is create_new so a symlink can't be followed (defense-in-depth atop
        // the admin-only secure_dir).
        let name = format!("{}-overlay.ico", style.as_str());
        write_durable(dir, &name, &bytes)
    }

    /// Writes `bytes` to a UNIQUE `O_EXCL` temp in `dir` (create_new refuses to follow a pre-placed
    /// symlink — defense-in-depth atop the admin-only secure_dir), `FlushFileBuffers`, and returns
    /// the temp path for the caller to publish. A partial temp is cleaned up on write failure.
    fn write_temp(dir: &Path, name: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
        use std::io::{ErrorKind, Write};
        let pid = std::process::id();
        let (tmp, mut file) = {
            let mut attempt = 0u32;
            loop {
                let tmp = dir.join(format!(".{name}.dm-{pid}-{attempt}.tmp"));
                match std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp) {
                    Ok(f) => break (tmp, f),
                    Err(e) if e.kind() == ErrorKind::AlreadyExists && attempt < 1000 => {
                        attempt += 1;
                    }
                    Err(e) => return Err(e),
                }
            }
        };
        let written: std::io::Result<()> = (|| {
            file.write_all(bytes)?;
            file.sync_all()
        })();
        drop(file);
        if let Err(e) = written {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        Ok(tmp)
    }

    /// Durably + atomically publishes `bytes` to `dir/name`, REPLACING any existing file, and returns
    /// its path. The temp is fsync'd, then published with `MoveFileEx(WRITE_THROUGH|REPLACE_EXISTING)`
    /// so the namespace change is durable before returning (the ProgramData ICO, which a re-apply
    /// legitimately rewrites). [WINDOWS-VERIFY] the write-through move on NTFS.
    fn write_durable(dir: &Path, name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
        use windows::core::HSTRING;
        use windows::Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        };
        let tmp = write_temp(dir, name, bytes).map_err(io)?;
        let target = dir.join(name);
        // SAFETY: valid UTF-16 paths; no buffers retained past the call.
        let moved = unsafe {
            MoveFileExW(
                &HSTRING::from(tmp.as_os_str()),
                &HSTRING::from(target.as_os_str()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        if let Err(e) = moved {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.to_string());
        }
        Ok(target)
    }

    /// Durably + atomically publishes the pre-first-apply snapshot to `dir/name`, NON-replacing: if
    /// the target already exists (a concurrent or prior elevated apply already snapshotted the true
    /// original), the write-through move fails-closed and we DISCARD our copy rather than clobber
    /// theirs with a possibly-already-modified value — closing the cross-process snapshot-once race
    /// (handoff §8a #4). WRITE_THROUGH makes the snapshot durable BEFORE the caller modifies HKLM.
    /// [WINDOWS-VERIFY] the write-through + non-replacing move on NTFS.
    fn snapshot_once(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
        use windows::core::HSTRING;
        use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH};
        let tmp = write_temp(dir, name, bytes).map_err(io)?;
        let target = dir.join(name);
        // No MOVEFILE_REPLACE_EXISTING: the move fails if the target exists (another process won).
        // SAFETY: valid UTF-16 paths; no buffers retained past the call.
        let moved = unsafe {
            MoveFileExW(
                &HSTRING::from(tmp.as_os_str()),
                &HSTRING::from(target.as_os_str()),
                MOVEFILE_WRITE_THROUGH,
            )
        };
        let _ = std::fs::remove_file(&tmp); // consumed by the move on success; discarded on collision
        match moved {
            Ok(()) => Ok(()),
            // An existing snapshot means another process satisfied snapshot-once — the desired no-op.
            Err(_) if target.exists() => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn notify_shell() {
        use windows::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};
        // SAFETY: documented global icon-association refresh; takes no ownership.
        unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
    }

    fn io(e: std::io::Error) -> String {
        e.to_string()
    }
}
