//! Recycle Bin styling via the per-user `DefaultIcon` registry values. Ported from
//! `DeskMakeover.Shell/RecycleBinIconWriter.cs`. Per-user (HKCU) only — no elevation. Value kinds
//! (`REG_SZ` vs `REG_EXPAND_SZ`) and unexpanded `%SystemRoot%` text are preserved for
//! byte-identical restore; a `DefaultIcon` of any OTHER (non-string) registry type is refused at
//! read time rather than silently collapsed to `REG_SZ` (APPLY-3). [WINDOWS-VERIFY] runtime.

use dm_domain::{PortError, PortResult, RecycleBinAnchor};
use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER};
use winreg::RegKey;

use crate::apply::reg_icon::{io, read_value, write_or_delete};

const USER_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\CLSID\{645FF040-5081-101B-9F08-00AA002F954E}\DefaultIcon";
const MACHINE_KEY: &str = r"CLSID\{645FF040-5081-101B-9F08-00AA002F954E}\DefaultIcon";

/// Reads the current effective `DefaultIcon` state (the restore anchor). Prefers the per-user
/// override; falls back to the machine CLSID; else records "no key".
///
/// A registry key/value that does NOT exist is a benign absence (`None`/`key_existed:false`); any
/// OTHER error (access denied, a corrupt hive) PROPAGATES as `Io` rather than collapsing to "no
/// key" (P2-#3) — a `read_current` that silently reported absence for an unreadable key would let
/// the restore anchor forget the user's real state. [WINDOWS-VERIFY] runtime.
pub fn read_current() -> PortResult<RecycleBinAnchor> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    match hkcu.open_subkey(USER_KEY) {
        Ok(key) => return state_from(&key, true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {} // fall through to the machine key
        Err(e) => return Err(PortError::Io(format!("open HKCU recycle-bin DefaultIcon: {e}"))),
    }
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    match hkcr.open_subkey(MACHINE_KEY) {
        Ok(key) => state_from(&key, false),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Ok(RecycleBinAnchor { key_existed: false, default: None, empty: None, full: None })
        }
        Err(e) => Err(PortError::Io(format!("open machine recycle-bin DefaultIcon: {e}"))),
    }
}

/// Points the per-user `DefaultIcon` at the styled empty/full ICOs. Mirrors `Apply`.
pub fn apply(empty_ico: &str, full_ico: &str) -> PortResult<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(USER_KEY).map_err(io)?;
    key.set_value("", &format!("{full_ico},0")).map_err(io)?;
    key.set_value("empty", &format!("{empty_ico},0")).map_err(io)?;
    key.set_value("full", &format!("{full_ico},0")).map_err(io)?;
    Ok(())
}

/// Restores the captured registry values (or removes the per-user key if it did not exist).
/// Mirrors `Restore`, preserving each value's original kind.
pub fn restore(anchor: &RecycleBinAnchor) -> PortResult<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if !anchor.key_existed {
        // We created this key; undo by removing ONLY the three values we wrote — NOT delete_subkey_all,
        // which recursively destroys unrelated values/subkeys another program may have added to the
        // same DefaultIcon key (codex B5-🔴). A non-NotFound failure PROPAGATES (a swallowed one is a
        // false success that then discards the anchor). The now-empty key is left in place: removing it
        // safely needs an emptiness check a concurrent writer could race, not worth the added surface.
        match hkcu.open_subkey_with_flags(USER_KEY, winreg::enums::KEY_SET_VALUE) {
            Ok(key) => {
                for name in ["", "empty", "full"] {
                    match key.delete_value(name) {
                        Ok(()) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                        Err(e) => return Err(io(e)),
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {} // key already gone — idempotent
            Err(e) => return Err(io(e)),
        }
        return Ok(());
    }
    let (key, _) = hkcu.create_subkey(USER_KEY).map_err(io)?;
    write_or_delete(&key, "", anchor.default.as_ref())?;
    write_or_delete(&key, "empty", anchor.empty.as_ref())?;
    write_or_delete(&key, "full", anchor.full.as_ref())?;
    Ok(())
}

fn state_from(key: &RegKey, key_existed: bool) -> PortResult<RecycleBinAnchor> {
    Ok(RecycleBinAnchor {
        key_existed,
        default: read_value(key, "")?,
        empty: read_value(key, "empty")?,
        full: read_value(key, "full")?,
    })
}
