//! Recycle Bin styling via the per-user `DefaultIcon` registry values. Ported from
//! `DeskMakeover.Shell/RecycleBinIconWriter.cs`. Per-user (HKCU) only — no elevation. Value kinds
//! (`REG_SZ` vs `REG_EXPAND_SZ`) and unexpanded `%SystemRoot%` text are preserved for
//! byte-identical restore. [WINDOWS-VERIFY] runtime.

use dm_domain::{PortError, PortResult, RecycleBinAnchor, RegistryValue};
use winreg::enums::{RegType, HKEY_CLASSES_ROOT, HKEY_CURRENT_USER};
use winreg::{RegKey, RegValue};

const USER_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\CLSID\{645FF040-5081-101B-9F08-00AA002F954E}\DefaultIcon";
const MACHINE_KEY: &str = r"CLSID\{645FF040-5081-101B-9F08-00AA002F954E}\DefaultIcon";

const REG_SZ_KIND: u32 = 1;
const REG_EXPAND_SZ_KIND: u32 = 2;

/// Reads the current effective `DefaultIcon` state (the restore anchor). Prefers the per-user
/// override; falls back to the machine CLSID; else records "no key".
pub fn read_current() -> RecycleBinAnchor {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey(USER_KEY) {
        return state_from(&key, true);
    }
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    match hkcr.open_subkey(MACHINE_KEY) {
        Ok(key) => state_from(&key, false),
        Err(_) => RecycleBinAnchor { key_existed: false, default: None, empty: None, full: None },
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
        let _ = hkcu.delete_subkey_all(USER_KEY); // idempotent if already gone
        return Ok(());
    }
    let (key, _) = hkcu.create_subkey(USER_KEY).map_err(io)?;
    write_or_delete(&key, "", anchor.default.as_ref())?;
    write_or_delete(&key, "empty", anchor.empty.as_ref())?;
    write_or_delete(&key, "full", anchor.full.as_ref())?;
    Ok(())
}

fn state_from(key: &RegKey, key_existed: bool) -> RecycleBinAnchor {
    RecycleBinAnchor {
        key_existed,
        default: read_value(key, ""),
        empty: read_value(key, "empty"),
        full: read_value(key, "full"),
    }
}

fn read_value(key: &RegKey, name: &str) -> Option<RegistryValue> {
    let raw = key.get_raw_value(name).ok()?;
    let text = decode_wide(&raw.bytes);
    Some(RegistryValue { raw: text, kind: reg_type_num(&raw.vtype) })
}

fn write_or_delete(key: &RegKey, name: &str, value: Option<&RegistryValue>) -> PortResult<()> {
    match value {
        None => {
            let _ = key.delete_value(name);
            Ok(())
        }
        Some(v) if v.kind == REG_EXPAND_SZ_KIND => {
            // Preserve REG_EXPAND_SZ with its unexpanded text.
            let raw = RegValue { bytes: encode_wide(&v.raw), vtype: RegType::REG_EXPAND_SZ };
            key.set_raw_value(name, &raw).map_err(io)
        }
        Some(v) => key.set_value(name, &v.raw).map_err(io),
    }
}

fn reg_type_num(t: &RegType) -> u32 {
    match t {
        RegType::REG_EXPAND_SZ => REG_EXPAND_SZ_KIND,
        _ => REG_SZ_KIND,
    }
}

/// Decodes a registry UTF-16LE string blob, trimming the terminating NUL.
fn decode_wide(bytes: &[u8]) -> String {
    let wide: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..end])
}

/// Encodes a string as UTF-16LE with a terminating NUL, for `set_raw_value`.
fn encode_wide(text: &str) -> Vec<u8> {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    wide.iter().flat_map(|c| c.to_le_bytes()).collect()
}

fn io(e: std::io::Error) -> PortError {
    PortError::Io(e.to_string())
}
