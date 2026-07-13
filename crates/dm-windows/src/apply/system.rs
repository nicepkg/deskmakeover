//! System desktop-icon styling (This PC / Network / User Files / Control Panel) via each namespace
//! CLSID's per-user `DefaultIcon` value. Per-user (HKCU) only — no elevation, exactly like the
//! Recycle Bin (`recyclebin.rs`), of which this is the single-value sibling: a desktop CLSID has ONE
//! `(Default)` `DefaultIcon` entry rather than the bin's empty/full pair. Value kinds are preserved
//! for byte-identical restore; a non-string type is refused at read time. [WINDOWS-VERIFY] runtime:
//! the exact per-user key that overrides each CLSID's desktop icon needs box confirmation — this
//! mirrors the Recycle Bin's proven `Explorer\CLSID\{clsid}\DefaultIcon` location.

use dm_domain::{PortError, PortResult, SystemIconAnchor};
use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER};
use winreg::RegKey;

use crate::apply::reg_icon::{io, read_value, write_or_delete};

/// Extracts the `{GUID}` from a desktop namespace parsing path (`::{GUID}`) or accepts a bare
/// `{GUID}`. A path with no recognisable CLSID is rejected rather than styling the wrong key.
pub fn parse_clsid(path: &str) -> PortResult<String> {
    let trimmed = path.trim();
    let candidate = trimmed.strip_prefix("::").unwrap_or(trimmed);
    if candidate.starts_with('{') && candidate.ends_with('}') && candidate.len() >= 3 {
        Ok(candidate.to_string())
    } else {
        Err(PortError::Unsupported(format!("not a System CLSID path: {path:?}")))
    }
}

/// The per-user override key that styles a desktop CLSID's icon (mirrors the Recycle Bin's).
fn user_key(clsid: &str) -> String {
    format!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\CLSID\{clsid}\DefaultIcon")
}

/// The machine class key whose `DefaultIcon` is the effective icon when no per-user override exists.
fn machine_key(clsid: &str) -> String {
    format!(r"CLSID\{clsid}\DefaultIcon")
}

/// Reads the current effective `DefaultIcon` (the restore anchor) for `clsid`. Prefers the per-user
/// override; falls back to the machine class default; else records "no key" with no value. A missing
/// key/value is a benign absence; any OTHER error propagates rather than collapsing to "no key"
/// (P2-#3). [WINDOWS-VERIFY] runtime.
pub fn read_current(clsid: &str) -> PortResult<SystemIconAnchor> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    match hkcu.open_subkey(user_key(clsid)) {
        Ok(key) => {
            return Ok(SystemIconAnchor { clsid: clsid.to_string(), key_existed: true, value: read_value(&key, "")? })
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {} // fall through to the machine key
        Err(e) => return Err(PortError::Io(format!("open HKCU System DefaultIcon {clsid}: {e}"))),
    }
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    match hkcr.open_subkey(machine_key(clsid)) {
        // No per-user override, but capture the machine value for source extraction (the machine
        // fallback — legitimate `key_existed:false` + `Some(value)`, mirroring the Recycle Bin).
        Ok(key) => Ok(SystemIconAnchor { clsid: clsid.to_string(), key_existed: false, value: read_value(&key, "")? }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Ok(SystemIconAnchor { clsid: clsid.to_string(), key_existed: false, value: None })
        }
        Err(e) => Err(PortError::Io(format!("open machine System DefaultIcon {clsid}: {e}"))),
    }
}

/// Points `clsid`'s per-user `DefaultIcon` at the styled ICO. Mirrors `recyclebin::apply`.
pub fn apply(clsid: &str, icon: &str) -> PortResult<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(user_key(clsid)).map_err(io)?;
    key.set_value("", &format!("{icon},0")).map_err(io)?;
    Ok(())
}

/// Restores the captured `DefaultIcon` (or removes the per-user value if the key did not exist).
/// Mirrors `recyclebin::restore`, preserving the value's original kind.
pub fn restore(anchor: &SystemIconAnchor) -> PortResult<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if !anchor.key_existed {
        // We created the per-user override; undo by removing ONLY the value we wrote (never
        // delete_subkey_all, which would destroy unrelated values another program added to the same
        // key — codex B5-🔴). A non-NotFound failure PROPAGATES. The now-empty key is left in place.
        match hkcu.open_subkey_with_flags(user_key(&anchor.clsid), winreg::enums::KEY_SET_VALUE) {
            Ok(key) => match key.delete_value("") {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(io(e)),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {} // key already gone — idempotent
            Err(e) => return Err(io(e)),
        }
        return Ok(());
    }
    let (key, _) = hkcu.create_subkey(user_key(&anchor.clsid)).map_err(io)?;
    write_or_delete(&key, "", anchor.value.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_clsid_accepts_parsing_path_and_bare_guid() {
        assert_eq!(
            parse_clsid("::{20D04FE0-3AEA-1069-A2D8-08002B30309D}").unwrap(),
            "{20D04FE0-3AEA-1069-A2D8-08002B30309D}"
        );
        assert_eq!(parse_clsid("{ABC}").unwrap(), "{ABC}");
        assert!(parse_clsid("C:/Desktop/thing.lnk").is_err());
        assert!(parse_clsid("::not-a-clsid").is_err());
        assert!(parse_clsid("").is_err());
    }

    #[test]
    fn key_paths_are_per_clsid() {
        let clsid = "{F02C1A0D-BE21-4350-88B0-7367FC96EF3C}";
        assert!(user_key(clsid).contains(clsid));
        assert!(user_key(clsid).ends_with(r"\DefaultIcon"));
        assert_eq!(machine_key(clsid), r"CLSID\{F02C1A0D-BE21-4350-88B0-7367FC96EF3C}\DefaultIcon");
    }
}
