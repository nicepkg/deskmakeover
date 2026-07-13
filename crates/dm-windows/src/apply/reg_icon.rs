//! Shared HKCU `DefaultIcon` registry-value helpers for the two registry-icon writers (the Recycle
//! Bin and the System desktop CLSIDs). Both style by pointing a per-user CLSID `DefaultIcon` value at
//! a generated ICO and restore by writing back the captured original — so the read/write/decode
//! primitives are single-sourced here (DRY) rather than duplicated per writer. [WINDOWS-VERIFY] runtime.

use dm_domain::{PortError, PortResult, RegistryValue};
use winreg::enums::RegType;
use winreg::{RegKey, RegValue};

pub const REG_SZ_KIND: u32 = 1;
pub const REG_EXPAND_SZ_KIND: u32 = 2;

/// Reads one string `DefaultIcon` value, preserving its exact kind. A missing value is a benign
/// `None`; any OTHER read error propagates (P2-#3). A non-string registry type (`REG_DWORD`,
/// `REG_BINARY`, …) is REFUSED rather than collapsed to `REG_SZ` — the old lossy path would read its
/// bytes as garbled UTF-16 and restore a type-changed value, silently rewriting the user's real
/// state (APPLY-3). [WINDOWS-VERIFY] runtime.
pub fn read_value(key: &RegKey, name: &str) -> PortResult<Option<RegistryValue>> {
    match key.get_raw_value(name) {
        Ok(raw) => {
            let kind = match raw.vtype {
                RegType::REG_SZ => REG_SZ_KIND,
                RegType::REG_EXPAND_SZ => REG_EXPAND_SZ_KIND,
                other => {
                    return Err(PortError::Io(format!(
                        "registry icon value {name:?} has an unexpected registry type {other:?}; \
                         refusing to style to avoid a lossy, type-changing restore"
                    )))
                }
            };
            Ok(Some(RegistryValue { raw: decode_wide(&raw.bytes), kind }))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(PortError::Io(format!("read registry icon value {name:?}: {e}"))),
    }
}

/// Writes `value` back under `name`, or deletes the value when `None`. Preserves the captured kind
/// (`REG_EXPAND_SZ` keeps its unexpanded `%SystemRoot%` text); a non-NotFound delete failure and an
/// unrecognised deserialized kind both PROPAGATE rather than silently type-changing the value (B5).
pub fn write_or_delete(key: &RegKey, name: &str, value: Option<&RegistryValue>) -> PortResult<()> {
    match value {
        None => match key.delete_value(name) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io(e)),
        },
        Some(v) if v.kind == REG_EXPAND_SZ_KIND => {
            let raw = RegValue { bytes: encode_wide(&v.raw), vtype: RegType::REG_EXPAND_SZ };
            key.set_raw_value(name, &raw).map_err(io)
        }
        Some(v) if v.kind == REG_SZ_KIND => key.set_value(name, &v.raw).map_err(io),
        Some(v) => Err(PortError::Io(format!(
            "registry icon restore anchor for {name:?} has an unrecognised registry kind {}; \
             refusing to write a type-changed value",
            v.kind
        ))),
    }
}

/// Decodes a registry UTF-16LE string blob, trimming the terminating NUL.
pub fn decode_wide(bytes: &[u8]) -> String {
    let wide: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..end])
}

/// Encodes a string as UTF-16LE with a terminating NUL, for `set_raw_value`.
pub fn encode_wide(text: &str) -> Vec<u8> {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    wide.iter().flat_map(|c| c.to_le_bytes()).collect()
}

pub fn io(e: std::io::Error) -> PortError {
    PortError::Io(e.to_string())
}
