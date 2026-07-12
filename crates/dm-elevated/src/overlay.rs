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

        // Snapshot the pre-DeskMakeover value exactly once. The state file lives in the hardened,
        // admin-only data dir, so a standard user cannot pre-plant it to defeat this guard.
        let state = dir.join(STATE_FILE);
        if !state.exists() {
            let original: String =
                key.get_value(OVERLAY_VALUE).unwrap_or_else(|_| ABSENT_MARKER.to_string());
            std::fs::write(&state, original).map_err(io)?;
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
        let dst = dir.join(format!("{}-overlay.ico", style.as_str()));
        // Atomic temp+fsync+rename, matching the durability discipline the .lnk/.url/desktop.ini
        // writers use: this ICO is referenced live by HKLM Shell Icons and read by Explorer's icon
        // cache, so a crash mid-write must not leave a torn/corrupt icon. (ELEV-2)
        let tmp = dir.join(format!(".{}-overlay.ico.tmp", style.as_str()));
        let write_tmp = || -> std::io::Result<()> {
            let mut f = std::fs::File::create(&tmp)?;
            std::io::Write::write_all(&mut f, &bytes)?;
            f.sync_all()
        };
        if let Err(e) = write_tmp() {
            let _ = std::fs::remove_file(&tmp);
            return Err(io(e));
        }
        if let Err(e) = std::fs::rename(&tmp, &dst) {
            let _ = std::fs::remove_file(&tmp);
            return Err(io(e));
        }
        Ok(dst)
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
