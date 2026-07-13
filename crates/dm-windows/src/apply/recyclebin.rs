//! Recycle Bin styling via the per-user `DefaultIcon` registry values. Ported from
//! `DeskMakeover.Shell/RecycleBinIconWriter.cs`. Per-user (HKCU) only — no elevation. Value kinds
//! (`REG_SZ` vs `REG_EXPAND_SZ`) and unexpanded `%SystemRoot%` text are preserved for
//! byte-identical restore; a `DefaultIcon` of any OTHER (non-string) registry type is refused at
//! read time rather than silently collapsed to `REG_SZ` (APPLY-3). [WINDOWS-VERIFY] runtime.

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
        let _ = hkcu.delete_subkey_all(USER_KEY); // idempotent if already gone
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

/// A missing value is a benign `None`; any other read error propagates (P2-#3).
fn read_value(key: &RegKey, name: &str) -> PortResult<Option<RegistryValue>> {
    match key.get_raw_value(name) {
        Ok(raw) => {
            // APPLY-3: preserve the value's real string kind faithfully. `DefaultIcon` is a path
            // string (`REG_SZ` / `REG_EXPAND_SZ`) by definition; ANY other type is an unexpected or
            // corrupt state. The old code collapsed every non-EXPAND type to `REG_SZ` and decoded
            // its bytes as UTF-16 — a `REG_DWORD`/`REG_BINARY`/`REG_MULTI_SZ` value would be read as
            // garbled text and RESTORED as a `REG_SZ`, silently rewriting the user's real value with
            // a different type. Refuse rather than corrupt: fail closed so the caller keeps the real
            // state (no lossy round-trip). [WINDOWS-VERIFY] runtime.
            let kind = match raw.vtype {
                RegType::REG_SZ => REG_SZ_KIND,
                RegType::REG_EXPAND_SZ => REG_EXPAND_SZ_KIND,
                other => {
                    return Err(PortError::Io(format!(
                        "recycle-bin value {name:?} has an unexpected registry type {other:?}; \
                         refusing to style to avoid a lossy, type-changing restore"
                    )))
                }
            };
            let text = decode_wide(&raw.bytes);
            Ok(Some(RegistryValue { raw: text, kind }))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(PortError::Io(format!("read recycle-bin value {name:?}: {e}"))),
    }
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
        Some(v) if v.kind == REG_SZ_KIND => key.set_value(name, &v.raw).map_err(io),
        // APPLY-3 (codex 🟠): read_value fails closed on non-string types, so a FRESHLY-captured
        // anchor's kind is always REG_SZ/REG_EXPAND_SZ. A DESERIALIZED anchor, though, carries an
        // unrestricted serialized `u32` (RegistryValue derives Serialize) — a corrupt, foreign, or
        // pre-fix-lossy persisted anchor could hold any value. Silently writing it as REG_SZ (the
        // old fall-through) would type-change the user's value on restore. Refuse instead: a restore
        // that cannot faithfully reproduce the captured kind must not rewrite it as a different type.
        Some(v) => Err(PortError::Io(format!(
            "recycle-bin restore anchor for {name:?} has an unrecognised registry kind {}; \
             refusing to write a type-changed value",
            v.kind
        ))),
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
