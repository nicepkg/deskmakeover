//! `.url` (internet shortcut) apply + restore. Ported from
//! `DeskMakeover.Shell/UrlShortcutIconWriter.cs`. The `[InternetShortcut]` INI upsert lives in the
//! host-tested [`crate::textfmt`]; restore replays the captured original bytes (same as a `.lnk`).

use dm_domain::{PortError, PortResult};

use crate::textfmt::{decode_ini_text_lossless, internet_shortcut_upsert};

/// Points a `.url` at `icon_path`/`index` by upserting `IconFile`/`IconIndex` in its
/// `[InternetShortcut]` section, preserving the rest of the file. Mirrors `Apply`.
///
/// The read is encoding-aware (Steam writes `.url` as UTF-16 LE, which `read_to_string` rejected —
/// the same defect that made these shortcuts non-styleable). The rewrite normalizes to UTF-8: the
/// shell reads any encoding, the reader now decodes any encoding so the read-back fingerprint still
/// matches, and restore replays the captured original bytes verbatim, so the original encoding is
/// never lost.
///
/// A `.url` we cannot decode LOSSLESSLY (a legacy ANSI code-page file with non-ASCII bytes) is
/// refused rather than restyled (codex R2-#3): rewriting it as UTF-8 after a lossy decode would
/// replace its non-ASCII URL/data with U+FFFD, corrupting the shortcut. UTF-8 and UTF-16 (what
/// Explorer and Steam write) decode losslessly, so this only skips genuinely legacy files.
///
/// [WINDOWS-VERIFY] runtime (filesystem semantics).
pub fn apply(url_path: &str, icon_path: &str, index: i32) -> PortResult<()> {
    let bytes = crate::durable::read_capped(url_path, crate::durable::SHORTCUT_READ_CAP)
        .map_err(|e| io(url_path, e))?;
    let text = decode_ini_text_lossless(&bytes).ok_or_else(|| {
        PortError::Io(format!(
            "{url_path}: refusing to restyle a .url in a legacy ANSI encoding — rewriting it as UTF-8 would corrupt its non-ASCII contents"
        ))
    })?;
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
