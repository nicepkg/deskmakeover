//! Fixed-drive styling via the per-user `DefaultIcon` registry value (ADR-0004 D7: drives style
//! via the registry `DriveIcons` mechanism — never `autorun.inf`). Per-user (HKCU) only — no
//! elevation, matching the app's HKCU-only, no-UAC discipline (a current-user override is honoured
//! by Explorer without local-machine permissions). Value kinds are preserved for byte-identical
//! restore; a non-string registry type is refused at read time rather than collapsed to `REG_SZ`
//! (the same APPLY-3 discipline as the Recycle Bin). [WINDOWS-VERIFY] runtime — the exact per-user
//! `Software\Classes\Applications\Explorer.exe\Drives\<L>` key.

use dm_domain::{DriveAnchor, PortError, PortResult};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;

use crate::apply::reg_icon::{io, read_value, write_or_delete};

/// The per-user override apply writes (and restore tears down). `HKCU` — current user only. The
/// `(Default)` value of the per-drive `DefaultIcon` subkey carries `"<path>,<index>"`, mirroring the
/// machine `DriveIcons` shape exactly (so a machine→per-user handoff is byte-compatible).
fn user_key(letter: &str) -> String {
    format!(r"Software\Classes\Applications\Explorer.exe\Drives\{letter}\DefaultIcon")
}

/// The machine `DriveIcons` `DefaultIcon` — the EFFECTIVE icon when NO per-user override exists.
/// Styling never writes HKLM, so it remains the true original for source extraction (the same
/// live-machine-fallback posture as `recyclebin::machine_state`).
fn machine_key(letter: &str) -> String {
    format!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\DriveIcons\{letter}\DefaultIcon")
}

/// Reads the current effective `DefaultIcon` state for `letter` — both the restore anchor and the
/// source-extraction original. Prefers the per-user override; falls back to the machine `DriveIcons`
/// (so a drive whose icon is set machine-wide still recovers its true original and restores the
/// per-user key we created without touching the machine value).
///
/// A missing key/value is a benign absence (`None` / `key_existed:false`); any OTHER error
/// PROPAGATES as `Io` rather than collapsing to "absent" — a `read_current` that silently reported
/// absence for an unreadable key would let the restore anchor forget the user's real state
/// (P2-#3). [WINDOWS-VERIFY] runtime.
pub fn read_current(letter: &str) -> PortResult<DriveAnchor> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    match hkcu.open_subkey(&user_key(letter)) {
        Ok(key) => Ok(DriveAnchor {
            letter: letter.to_string(),
            key_existed: true,
            value: read_value(&key, "")?,
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => machine_state(letter),
        Err(e) => Err(PortError::Io(format!("open HKCU drive DefaultIcon: {e}"))),
    }
}

/// The machine (HKLM) `DriveIcons` state — the effective original when NO per-user override
/// exists. `key_existed:false` always (apply only ever writes the per-user key, so restore removes
/// it, never the machine value). All-`None` when the machine key is absent (drive uses the shell
/// stock icon). [WINDOWS-VERIFY] runtime (the exact HKLM value name).
pub(crate) fn machine_state(letter: &str) -> PortResult<DriveAnchor> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    match hklm.open_subkey(&machine_key(letter)) {
        Ok(key) => Ok(DriveAnchor {
            letter: letter.to_string(),
            key_existed: false,
            value: read_value(&key, "")?,
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(DriveAnchor {
            letter: letter.to_string(),
            key_existed: false,
            value: None,
        }),
        Err(e) => Err(PortError::Io(format!("open machine drive DefaultIcon: {e}"))),
    }
}

/// Points the per-user `DefaultIcon` at the styled ICO (`"<path>,0"`).
pub fn apply(letter: &str, icon: &str) -> PortResult<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(&user_key(letter)).map_err(io)?;
    key.set_value("", &format!("{icon},0")).map_err(io)?;
    Ok(())
}

/// Restores the captured per-user value (or removes the value we created when none existed).
pub fn restore(anchor: &DriveAnchor) -> PortResult<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if !anchor.key_existed {
        // We created this value; undo by removing ONLY the `(Default)` value we wrote — never
        // `delete_subkey_all`, which would destroy unrelated state another program added to the
        // same key. A non-NotFound failure PROPAGATES (a swallowed one is a false success that
        // then discards the anchor). Idempotent when the value is already gone.
        match hkcu.open_subkey_with_flags(&user_key(&anchor.letter), winreg::enums::KEY_SET_VALUE) {
            Ok(key) => match key.delete_value("") {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(io(e)),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {} // key already gone
            Err(e) => return Err(io(e)),
        }
        return Ok(());
    }
    let (key, _) = hkcu.create_subkey(&user_key(&anchor.letter)).map_err(io)?;
    write_or_delete(&key, "", anchor.value.as_ref())?;
    Ok(())
}

/// Derives the bare drive letter (`"C"`, uppercase, no colon) from a drive-root path (`"C:\"` /
/// `"C:"`). Anything that is not a drive spec — a subfolder (`C:\Users\me`) or a UNC path — is
/// `Unsupported`, never silently styled against the wrong per-drive key.
pub fn letter_from_path(path: &str) -> PortResult<String> {
    let mut it = path.trim().chars();
    let c = it
        .next()
        .filter(|c| c.is_ascii_alphabetic())
        .ok_or_else(|| PortError::Unsupported(format!("path is not a drive root: {path}")))?;
    // A drive spec is `<letter>:` optionally followed by a single root separator (`\`).
    let rest = match it.as_str().strip_prefix(':') {
        Some(r) => r,
        None => return Err(PortError::Unsupported(format!("path is not a drive root: {path}"))),
    };
    if rest.is_empty() || rest == "\\" {
        Ok(c.to_ascii_uppercase().to_string())
    } else {
        Err(PortError::Unsupported(format!("path is not a drive root: {path}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letter_from_path_accepts_a_drive_root_and_uppercases() {
        assert_eq!(letter_from_path("C:\\").unwrap(), "C");
        assert_eq!(letter_from_path("d:").unwrap(), "D");
        assert_eq!(letter_from_path("  e:\\").unwrap(), "E");
    }

    #[test]
    fn letter_from_path_rejects_a_non_drive_path() {
        assert!(letter_from_path(r"C:\Users\me").is_err());
        assert!(letter_from_path("").is_err());
        assert!(letter_from_path(r"\\server\share").is_err());
    }
}
