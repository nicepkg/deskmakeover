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
    use std::io::Read;
    use std::path::PathBuf;

    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    use crate::args::Style;
    use crate::guards::validate_ico;

    const SHELL_ICONS_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Shell Icons";
    const OVERLAY_VALUE: &str = "29";
    const ABSENT_MARKER: &str = "__absent__";

    fn data_dir() -> PathBuf {
        let base = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".to_string());
        PathBuf::from(base).join("DeskMakeover")
    }

    fn state_path() -> PathBuf {
        data_dir().join("overlay-state.txt")
    }

    pub fn apply(style: Style, source_file: Option<&str>) -> Result<(), String> {
        std::fs::create_dir_all(data_dir()).map_err(io)?;
        let ico = materialize_ico(style, source_file)?;

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let key = hklm.create_subkey(SHELL_ICONS_KEY).map_err(io)?.0;

        // Snapshot the pre-DeskMakeover value exactly once (oracle: write state only when absent).
        if !state_path().exists() {
            let original: String =
                key.get_value(OVERLAY_VALUE).unwrap_or_else(|_| ABSENT_MARKER.to_string());
            std::fs::write(state_path(), original).map_err(io)?;
        }

        // Point the registry at the ProgramData copy — NEVER the caller path (LPE guard).
        key.set_value(OVERLAY_VALUE, &format!("{},0", ico.display())).map_err(io)?;
        notify_shell();
        Ok(())
    }

    pub fn restore() -> Result<(), String> {
        if !state_path().exists() {
            return Ok(()); // untouched — nothing to restore
        }
        let original = std::fs::read_to_string(state_path()).map_err(io)?;
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let key = hklm.create_subkey(SHELL_ICONS_KEY).map_err(io)?.0;
        if original == ABSENT_MARKER {
            let _ = key.delete_value(OVERLAY_VALUE); // idempotent if already gone
        } else {
            key.set_value(OVERLAY_VALUE, &original).map_err(io)?;
        }
        std::fs::remove_file(state_path()).map_err(io)?;
        notify_shell();
        Ok(())
    }

    /// Validates the rendered ICO and copies it into ProgramData, returning the ProgramData path.
    ///
    /// NOTE (blind-write divergence): the oracle generated the built-in refined/transparent ICOs
    /// in-process (`OverlayBadgeIconFactory`). Here EVERY style's ICO arrives as a validated
    /// `--file` from the client, which owns the icon core. The LPE guard (validate + copy into
    /// ProgramData) is applied uniformly. [icon-core-need] the client renders the
    /// transparent/refined overlay ICOs. [WINDOWS-VERIFY] registry + refresh behaviour.
    fn materialize_ico(style: Style, source_file: Option<&str>) -> Result<PathBuf, String> {
        let src = source_file
            .ok_or_else(|| format!("--file (a rendered .ico) is required for the {} overlay", style.as_str()))?;
        let meta = std::fs::metadata(src).map_err(io)?;
        let mut header = [0u8; 6];
        let read = std::fs::File::open(src).map_err(io)?.read(&mut header).map_err(io)?;
        validate_ico(meta.len(), &header[..read])?;
        let dst = data_dir().join(format!("{}-overlay.ico", style.as_str()));
        std::fs::copy(src, &dst).map_err(io)?;
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
