//! Pure text formatting for the `.url` and folder writers, split out of the `cfg(windows)` apply
//! modules so it compiles and is unit-tested on the Mac host. Ported from
//! `UrlShortcutIconWriter.UpsertInInternetShortcutSection` and `FolderIconWriter`'s `desktop.ini`
//! body.

const INTERNET_SHORTCUT_SECTION: &str = "[InternetShortcut]";

/// Upserts `key=value` inside the `[InternetShortcut]` section (case-insensitive), replacing any
/// existing occurrences in place and inserting at the section end otherwise. Exact port of the
/// oracle upsert.
pub fn internet_shortcut_upsert(lines: &mut Vec<String>, key: &str, value: &str) -> Result<(), String> {
    let section_start = lines
        .iter()
        .position(|l| l.trim().eq_ignore_ascii_case(INTERNET_SHORTCUT_SECTION))
        .ok_or_else(|| "URL shortcut is missing the [InternetShortcut] section".to_string())?;
    let section_end = section_end(lines, section_start);

    // Remove existing matches scanning backward, remembering the smallest matched index.
    let mut first_match: Option<usize> = None;
    let mut idx = section_end;
    while idx > section_start + 1 {
        idx -= 1;
        if let Some(sep) = lines[idx].find('=') {
            if lines[idx][..sep].trim().eq_ignore_ascii_case(key) {
                first_match = Some(idx);
                lines.remove(idx);
            }
        }
    }

    let insert_at = first_match.unwrap_or(section_end);
    lines.insert(insert_at, format!("{key}={value}"));
    Ok(())
}

/// Reads the current `IconFile`/`IconIndex` back out of a `.url`'s `[InternetShortcut]` section
/// — the icon reference the reader fingerprints so a read-back can be compared to the asset the
/// applier was asked to point at (P1-1). Returns `None` when the section or key is absent.
pub fn parse_internet_shortcut_icon(text: &str) -> Option<(String, i32)> {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.trim().eq_ignore_ascii_case(INTERNET_SHORTCUT_SECTION))?;
    let mut icon_file: Option<String> = None;
    let mut icon_index: i32 = 0;
    for line in &lines[start + 1..] {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            break; // next section
        }
        if let Some(sep) = line.find('=') {
            let key = line[..sep].trim();
            let value = &line[sep + 1..];
            if key.eq_ignore_ascii_case("IconFile") {
                icon_file = Some(value.to_string());
            } else if key.eq_ignore_ascii_case("IconIndex") {
                icon_index = value.trim().parse().unwrap_or(0);
            }
        }
    }
    icon_file.map(|f| (f, icon_index))
}

/// The `desktop.ini` body pointing a folder at `icon_path` (oracle content, CRLF-terminated).
pub fn desktop_ini_content(icon_path: &str) -> String {
    format!("[.ShellClassInfo]\r\nIconResource={icon_path},0\r\nConfirmFileOp=0\r\n")
}

/// Reads the `IconResource=path,index` back out of a `desktop.ini`'s bytes (BOM-tolerant) — the
/// folder icon reference the reader fingerprints (P1-1). Returns `None` when absent.
pub fn parse_desktop_ini_icon(bytes: &[u8]) -> Option<(String, i32)> {
    let text = std::str::from_utf8(bytes).ok()?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for line in text.lines() {
        if let Some(sep) = line.find('=') {
            if line[..sep].trim().eq_ignore_ascii_case("IconResource") {
                let value = line[sep + 1..].trim();
                // Split the LAST comma into path,index (paths can contain commas).
                return Some(match value.rfind(',') {
                    Some(comma) => {
                        let (path, idx) = value.split_at(comma);
                        (path.to_string(), idx[1..].trim().parse().unwrap_or(0))
                    }
                    None => (value.to_string(), 0),
                });
            }
        }
    }
    None
}

/// `desktop.ini` bytes with a leading UTF-8 BOM so non-ASCII icon paths are honoured.
pub fn desktop_ini_bytes(icon_path: &str) -> Vec<u8> {
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(desktop_ini_content(icon_path).as_bytes());
    bytes
}

/// The index of the next `[section]` header after `start`, or the line count.
fn section_end(lines: &[String], start: usize) -> usize {
    for (offset, line) in lines.iter().enumerate().skip(start + 1) {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            return offset;
        }
    }
    lines.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &str) -> Vec<String> {
        s.lines().map(str::to_string).collect()
    }

    #[test]
    fn inserts_new_keys_at_section_end() {
        let mut l = lines("[InternetShortcut]\nURL=https://example.test");
        internet_shortcut_upsert(&mut l, "IconFile", r"C:\gen\app.ico").unwrap();
        internet_shortcut_upsert(&mut l, "IconIndex", "0").unwrap();
        assert_eq!(
            l,
            vec![
                "[InternetShortcut]".to_string(),
                "URL=https://example.test".to_string(),
                r"IconFile=C:\gen\app.ico".to_string(),
                "IconIndex=0".to_string(),
            ]
        );
    }

    #[test]
    fn replaces_existing_key_in_place() {
        let mut l = lines("[InternetShortcut]\nURL=https://x\nIconFile=old.ico\nIconIndex=5");
        internet_shortcut_upsert(&mut l, "IconFile", "new.ico").unwrap();
        assert_eq!(l[2], "IconFile=new.ico");
        assert_eq!(l[3], "IconIndex=5"); // untouched
        assert_eq!(l.len(), 4);
    }

    #[test]
    fn respects_section_boundaries_and_case() {
        let mut l = lines("[internetshortcut]\nURL=https://x\n\n[Other]\nIconFile=keep.ico");
        internet_shortcut_upsert(&mut l, "IconFile", "styled.ico").unwrap();
        assert!(l.contains(&"IconFile=keep.ico".to_string()));
        assert!(l.contains(&"IconFile=styled.ico".to_string()));
    }

    #[test]
    fn missing_section_is_an_error() {
        let mut l = lines("URL=https://x");
        assert!(internet_shortcut_upsert(&mut l, "IconFile", "x.ico").is_err());
    }

    #[test]
    fn value_with_equals_and_query_string_is_preserved() {
        // A URL value containing `=` must survive intact (only the FIRST `=` splits key/value).
        let mut l = lines("[InternetShortcut]\nURL=https://x?a=1&b=2\nIconFile=old.ico");
        internet_shortcut_upsert(&mut l, "IconFile", "styled.ico").unwrap();
        assert!(l.contains(&"URL=https://x?a=1&b=2".to_string()));
        assert!(l.contains(&"IconFile=styled.ico".to_string()));
    }

    #[test]
    fn empty_section_inserts_at_end() {
        let mut l = lines("[InternetShortcut]");
        internet_shortcut_upsert(&mut l, "IconFile", "a.ico").unwrap();
        assert_eq!(l, vec!["[InternetShortcut]".to_string(), "IconFile=a.ico".to_string()]);
    }

    #[test]
    fn first_internet_shortcut_section_wins() {
        // A malformed file with two sections: the upsert targets the first, not the decoy.
        let mut l = lines("[InternetShortcut]\nURL=https://a\n[InternetShortcut]\nURL=https://b");
        internet_shortcut_upsert(&mut l, "IconFile", "a.ico").unwrap();
        // Inserted inside the first section (before the second header at index 2).
        assert_eq!(l[2], "IconFile=a.ico");
    }

    #[test]
    fn desktop_ini_preserves_special_chars_in_icon_path() {
        let content = desktop_ini_content(r"C:\图标\my icon,v2.ico");
        assert!(content.contains(r"IconResource=C:\图标\my icon,v2.ico,0"));
        assert!(content.ends_with("ConfirmFileOp=0\r\n"));
        assert!(content.contains("\r\n")); // CRLF line endings
    }

    #[test]
    fn desktop_ini_content_matches_oracle_shape() {
        assert_eq!(
            desktop_ini_content(r"C:\gen\folder.ico"),
            "[.ShellClassInfo]\r\nIconResource=C:\\gen\\folder.ico,0\r\nConfirmFileOp=0\r\n"
        );
    }

    #[test]
    fn desktop_ini_bytes_lead_with_utf8_bom() {
        assert_eq!(&desktop_ini_bytes("x.ico")[..3], &[0xEF, 0xBB, 0xBF]);
    }

    #[test]
    fn parse_internet_shortcut_icon_round_trips_the_writer() {
        // What the writer upserts, the reader reads back — so a genuine apply's read-back matches
        // the asset-derived expected (P1-1).
        let mut l = lines("[InternetShortcut]\nURL=https://x?a=1&b=2");
        internet_shortcut_upsert(&mut l, "IconFile", r"C:\gen\a.ico").unwrap();
        internet_shortcut_upsert(&mut l, "IconIndex", "0").unwrap();
        let text = l.join("\r\n");
        assert_eq!(parse_internet_shortcut_icon(&text), Some((r"C:\gen\a.ico".to_string(), 0)));
    }

    #[test]
    fn parse_internet_shortcut_icon_absent_is_none() {
        assert_eq!(parse_internet_shortcut_icon("[InternetShortcut]\nURL=https://x"), None);
        assert_eq!(parse_internet_shortcut_icon("URL=https://x"), None); // no section
    }

    #[test]
    fn parse_desktop_ini_icon_round_trips_the_writer_with_bom_and_comma_paths() {
        let bytes = desktop_ini_bytes(r"C:\图标\my icon,v2.ico");
        // A path containing a comma must survive: only the LAST comma splits path,index.
        assert_eq!(
            parse_desktop_ini_icon(&bytes),
            Some((r"C:\图标\my icon,v2.ico".to_string(), 0))
        );
    }

    #[test]
    fn parse_desktop_ini_icon_absent_is_none() {
        assert_eq!(parse_desktop_ini_icon(b"[.ShellClassInfo]\r\nConfirmFileOp=0\r\n"), None);
    }
}
