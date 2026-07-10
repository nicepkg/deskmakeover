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

/// The `desktop.ini` body pointing a folder at `icon_path` (oracle content, CRLF-terminated).
pub fn desktop_ini_content(icon_path: &str) -> String {
    format!("[.ShellClassInfo]\r\nIconResource={icon_path},0\r\nConfirmFileOp=0\r\n")
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
}
