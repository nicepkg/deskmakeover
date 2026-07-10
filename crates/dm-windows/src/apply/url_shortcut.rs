//! `.url` (internet shortcut) apply + restore. Ported from
//! `DeskMakeover.Shell/UrlShortcutIconWriter.cs`. The `[InternetShortcut]` INI upsert lives in the
//! host-tested [`crate::textfmt`]; restore replays the captured original bytes (same as a `.lnk`).

use dm_domain::{PortError, PortResult};

use crate::textfmt::internet_shortcut_upsert;

/// Points a `.url` at `icon_path`/`index` by upserting `IconFile`/`IconIndex` in its
/// `[InternetShortcut]` section, preserving the rest of the file. Mirrors `Apply`.
///
/// [WINDOWS-VERIFY] runtime (filesystem semantics).
pub fn apply(url_path: &str, icon_path: &str, index: i32) -> PortResult<()> {
    let text = std::fs::read_to_string(url_path).map_err(|e| io(url_path, e))?;
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    internet_shortcut_upsert(&mut lines, "IconFile", icon_path).map_err(PortError::Io)?;
    internet_shortcut_upsert(&mut lines, "IconIndex", &index.to_string()).map_err(PortError::Io)?;
    // Durable + atomic so a crash mid-write can't tear the `.url` (P1-9).
    crate::durable::write_atomic(url_path, lines.join("\r\n").as_bytes())
}

fn io(path: &str, e: std::io::Error) -> PortError {
    if e.kind() == std::io::ErrorKind::NotFound {
        PortError::NotFound(path.to_string())
    } else {
        PortError::Io(e.to_string())
    }
}
