//! Pure text formatting for the `.url` and folder writers, split out of the `cfg(windows)` apply
//! modules so it compiles and is unit-tested on the Mac host. Ported from
//! `UrlShortcutIconWriter.UpsertInInternetShortcutSection` and `FolderIconWriter`'s `desktop.ini`
//! body.

const INTERNET_SHORTCUT_SECTION: &str = "[InternetShortcut]";

/// Strips a leading UTF-8 BOM. Windows Explorer writes `.url` (and `desktop.ini`) files
/// with a BOM whenever their content has non-ASCII characters — routine for Chinese site
/// names and paths, which is squarely this product's userbase. `str::trim` does NOT remove
/// U+FEFF (it is Unicode category `Cf`, not `White_Space`), so a section header behind a BOM
/// otherwise fails every `trim().eq_ignore_ascii_case(...)` match and the whole file reads as
/// sectionless. The BOM only ever appears at byte 0, i.e. the start of the first line. (APPLY-1)
fn strip_bom(s: &str) -> &str {
    s.strip_prefix('\u{feff}').unwrap_or(s)
}

#[derive(Clone, Copy)]
enum Utf16Endian {
    Little,
    Big,
}

/// Decodes the raw bytes of a Windows INI-style text file (`.url` internet shortcut, `desktop.ini`)
/// into a `String`, honouring the byte-order mark that names its encoding. This is the ONE
/// file-text decoder the `.url` reader/writer, the folder `desktop.ini` reader, and the source
/// extractor all share, so no read path can regress into assuming UTF-8.
///
/// Windows tools disagree wildly on how they encode these files: **Steam (and other game
/// launchers) write `.url` shortcuts as UTF-16 LE**; Explorer writes UTF-8 (with or without a BOM);
/// legacy tools wrote the system ANSI code page. `std::fs::read_to_string` assumes UTF-8 and ERRORS
/// on any UTF-16 form — the leading `0xFF` of a UTF-16 LE BOM is not a legal UTF-8 byte — which
/// made every Steam-created `.url` read as unreadable and therefore non-styleable (owner report
/// 2026-07-15, three Steam `.url` icons ignoring every config change).
///
/// Detection order: UTF-8 BOM → UTF-16 LE BOM → UTF-16 BE BOM → BOM-less UTF-16 (interleaved-NUL
/// heuristic — real INI text has no NUL bytes, so a slice riddled with them on one byte parity is
/// UTF-16 of that endianness) → strict UTF-8 → lossy UTF-8. The returned string never carries a
/// leading U+FEFF, so the existing per-line [`strip_bom`] stays defensive rather than required.
pub fn decode_ini_text_bytes(bytes: &[u8]) -> String {
    decode_ini_text(bytes).0
}

/// Like [`decode_ini_text_bytes`] but yields `None` when the bytes could only be decoded LOSSILY
/// (a legacy ANSI code-page file carrying non-ASCII bytes). A WRITER must use this and refuse,
/// rather than rewrite the file as UTF-8 with those bytes replaced by U+FFFD — that corrupts the
/// shortcut's URL/data (codex R2-#3). READERS/fingerprinters keep using the lossy
/// [`decode_ini_text_bytes`]: a lossy read only risks a benign fingerprint miss, never on-disk loss.
pub fn decode_ini_text_lossless(bytes: &[u8]) -> Option<String> {
    let (text, lossy) = decode_ini_text(bytes);
    if lossy {
        None
    } else {
        Some(text)
    }
}

/// The shared decoder. The bool is `true` when the ANSI fallback had to replace undecodable bytes
/// with U+FFFD. UTF-8 (BOM or bare) reports loss precisely; UTF-16 is treated as lossless (the shell
/// never writes the unpaired surrogates that are the only thing `from_utf16_lossy` would drop).
fn decode_ini_text(bytes: &[u8]) -> (String, bool) {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return utf8_checked(rest);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return (decode_utf16(rest, Utf16Endian::Little), false);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return (decode_utf16(rest, Utf16Endian::Big), false);
    }
    // No BOM. A slice riddled with NULs is BOM-less UTF-16 (a NUL is legal UTF-8, so `from_utf8`
    // would otherwise silently accept UTF-16 LE ASCII as a string full of NUL chars) — sniff that
    // FIRST, then trust UTF-8, then decode lossily so a genuinely mixed-encoding file still yields
    // its ASCII keys and paths.
    if let Some(endian) = sniff_bomless_utf16(bytes) {
        return (decode_utf16(bytes, endian), false);
    }
    utf8_checked(bytes)
}

fn utf8_checked(bytes: &[u8]) -> (String, bool) {
    match std::str::from_utf8(bytes) {
        Ok(s) => (s.to_string(), false),
        Err(_) => (String::from_utf8_lossy(bytes).into_owned(), true),
    }
}

fn decode_utf16(bytes: &[u8], endian: Utf16Endian) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| match endian {
            Utf16Endian::Little => u16::from_le_bytes([c[0], c[1]]),
            Utf16Endian::Big => u16::from_be_bytes([c[0], c[1]]),
        })
        .collect();
    let s = String::from_utf16_lossy(&units);
    // A BOM-less slice can still open with a U+FEFF unit; strip a single leading one so the result
    // is clean text regardless of how it was detected.
    s.strip_prefix('\u{feff}').map(str::to_string).unwrap_or(s)
}

/// Detects BOM-less UTF-16 by its interleaved-NUL signature: ASCII/CJK INI text is essentially
/// NUL-free as UTF-8, but ASCII-heavy UTF-16 is ~half NUL, clustered on one byte parity (the
/// zero high byte). Requires ≥ a quarter of the bytes to be NUL before committing, and names the
/// endianness from which parity holds them (LE → high byte at ODD indices, BE → EVEN).
fn sniff_bomless_utf16(bytes: &[u8]) -> Option<Utf16Endian> {
    if bytes.len() < 2 {
        return None;
    }
    let (mut even_nul, mut odd_nul) = (0usize, 0usize);
    for (i, &b) in bytes.iter().enumerate() {
        if b == 0 {
            if i % 2 == 0 {
                even_nul += 1;
            } else {
                odd_nul += 1;
            }
        }
    }
    if (even_nul + odd_nul) * 4 < bytes.len() {
        return None; // < 25% NUL → ordinary single-byte text, not UTF-16-of-ASCII
    }
    match odd_nul.cmp(&even_nul) {
        std::cmp::Ordering::Greater => Some(Utf16Endian::Little),
        std::cmp::Ordering::Less => Some(Utf16Endian::Big),
        std::cmp::Ordering::Equal => None,
    }
}

/// Upserts `key=value` inside the `[InternetShortcut]` section (case-insensitive), replacing any
/// existing occurrences in place and inserting at the section end otherwise. Exact port of the
/// oracle upsert.
pub fn internet_shortcut_upsert(lines: &mut Vec<String>, key: &str, value: &str) -> Result<(), String> {
    let section_start = lines
        .iter()
        .position(|l| strip_bom(l).trim().eq_ignore_ascii_case(INTERNET_SHORTCUT_SECTION))
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
    let text = strip_bom(text); // BOM-tolerant, mirroring parse_desktop_ini_icon (APPLY-1)
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
    // Encoding-aware: a folder styled by another tool may carry a UTF-16 `desktop.ini`, which the
    // old `from_utf8` read as "no icon" (a silent unstyled reading, one class softer than the `.url`
    // fingerprint failure but the same root cause).
    let text = decode_ini_text_bytes(bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    // Section-scoped (codex R2-#4): `IconResource` is a `[.ShellClassInfo]` key, so scan ONLY that
    // section — a decoy `IconResource` in another section (e.g. `[ViewState]`) before it must never
    // be read as the folder icon, or the fingerprint mismatches and a real restore reads a false CAS
    // conflict. Mirrors `parse_internet_shortcut_icon`'s section discipline.
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.trim().eq_ignore_ascii_case("[.ShellClassInfo]"))?;
    for line in &lines[start + 1..] {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            break; // next section — IconResource past here belongs to a different section
        }
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

/// `desktop.ini` bytes as **UTF-16 LE with a BOM** — the encoding Windows Explorer actually
/// honours for a folder's custom icon.
///
/// A UTF-8-BOM `desktop.ini` (what this wrote before) is SILENTLY IGNORED by Explorer: the shell's
/// INI parser reads the leading `EF BB BF` as three garbage leading characters on the first line,
/// so `[.ShellClassInfo]` never matches its section header and the whole file is discarded — the
/// folder falls back to the default manila icon while the `.lnk` items around it (which never touch
/// desktop.ini) style fine. This was the owner-visible "the folder never gets styled" bug
/// (2026-07-17), root-caused by a clean A/B on the live desktop: the SAME icon + SAME asset + SAME
/// `SHChangeNotify` shows the custom icon when the file is UTF-16 LE and the default icon when it is
/// UTF-8-BOM. The proven-good reference (`D:\shells\...\Set-FolderCustomIcon`) writes
/// `Set-Content -Encoding Unicode`, i.e. UTF-16 LE — matched here.
///
/// The reader ([`parse_desktop_ini_icon`] → [`decode_ini_text_bytes`]) already decodes UTF-16 LE,
/// so the read-back fingerprint still matches; restore replays the captured original bytes, so the
/// original encoding is never lost.
pub fn desktop_ini_bytes(icon_path: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xFE]; // UTF-16 LE BOM
    for unit in desktop_ini_content(icon_path).encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
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
    fn desktop_ini_bytes_are_utf16_le_which_explorer_honours() {
        // Explorer SILENTLY IGNORES a UTF-8-BOM desktop.ini (the `EF BB BF` breaks the
        // `[.ShellClassInfo]` section match) → the folder shows the default icon. It parses UTF-16
        // LE, which the proven-good reference writes. Regression for the 2026-07-17 "folder never
        // styles" bug.
        let bytes = desktop_ini_bytes("x.ico");
        assert_eq!(&bytes[..2], &[0xFF, 0xFE], "UTF-16 LE BOM, not UTF-8");
        // The section header is present as UTF-16 LE (each ASCII char is <byte>,0x00).
        assert_eq!(&bytes[2..6], &[0x5B, 0x00, 0x2E, 0x00], "'[.' encoded UTF-16 LE");
        // And the reader round-trips it (encoding-aware decode).
        assert_eq!(parse_desktop_ini_icon(&bytes), Some(("x.ico".to_string(), 0)));
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

    #[test]
    fn parse_desktop_ini_icon_ignores_iconresource_outside_shellclassinfo() {
        // A decoy IconResource in a PRIOR section must not be read as the folder icon — only the
        // one under [.ShellClassInfo] counts (codex R2-#4).
        let content =
            "[ViewState]\r\nIconResource=C:\\decoy.ico,9\r\n[.ShellClassInfo]\r\nIconResource=C:\\real.ico,0\r\nConfirmFileOp=0\r\n";
        assert_eq!(parse_desktop_ini_icon(content.as_bytes()), Some((r"C:\real.ico".to_string(), 0)));
    }

    #[test]
    fn parse_desktop_ini_icon_iconresource_only_outside_section_is_none() {
        // IconResource that never appears under [.ShellClassInfo] is not the folder icon.
        let content = "[ViewState]\r\nIconResource=C:\\decoy.ico,9\r\n[.ShellClassInfo]\r\nConfirmFileOp=0\r\n";
        assert_eq!(parse_desktop_ini_icon(content.as_bytes()), None);
    }

    #[test]
    fn upsert_finds_the_section_behind_a_utf8_bom_and_preserves_it() {
        // APPLY-1: Explorer BOM-prefixes .url files whose content has non-ASCII chars
        // (Chinese sites). The section must be found despite the BOM, and the BOM must
        // survive the rewrite so the file's encoding marker is not silently dropped.
        let mut l = lines("\u{feff}[InternetShortcut]\nURL=https://例子.test\nIconFile=old.ico");
        internet_shortcut_upsert(&mut l, "IconFile", r"C:\gen\a.ico").unwrap();
        assert!(l[0].starts_with('\u{feff}'), "the file's leading BOM is preserved");
        assert!(l.contains(&r"IconFile=C:\gen\a.ico".to_string()));
        assert!(!l.contains(&"IconFile=old.ico".to_string()), "the old key was replaced");
    }

    #[test]
    fn parse_internet_shortcut_icon_tolerates_a_leading_bom() {
        let text = "\u{feff}[InternetShortcut]\r\nURL=https://例子.test\r\nIconFile=C:\\gen\\a.ico\r\nIconIndex=0";
        assert_eq!(parse_internet_shortcut_icon(text), Some((r"C:\gen\a.ico".to_string(), 0)));
    }

    /// Encodes `text` as UTF-16 with a leading BOM, exactly how Steam writes a `.url`.
    fn utf16_with_bom(text: &str, big_endian: bool) -> Vec<u8> {
        let mut bytes = if big_endian { vec![0xFE, 0xFF] } else { vec![0xFF, 0xFE] };
        for unit in text.encode_utf16() {
            let pair = if big_endian { unit.to_be_bytes() } else { unit.to_le_bytes() };
            bytes.extend_from_slice(&pair);
        }
        bytes
    }

    #[test]
    fn decodes_a_real_steam_url_shortcut_which_is_utf16_le() {
        // The exact shape of a Steam desktop `.url` (owner box, 2026-07-15): a `[{GUID}]` prop
        // section, then `[InternetShortcut]` with a `steam://rungameid/...` URL and an `IconFile`
        // pointing at Steam's icon cache — the whole file written as UTF-16 LE with a BOM. Read as
        // UTF-8 this failed outright, marking the icon non-styleable.
        let content = "[{000214A0-0000-0000-C000-000000000046}]\r\nProp3=19,0\r\n[InternetShortcut]\r\nIDList=\r\nIconIndex=0\r\nURL=steam://rungameid/1868140\r\nIconFile=D:\\apps\\steam\\steam\\games\\ac24c6922f55c7dd7ab535f871c9adff300e6feb.ico\r\n";
        let bytes = utf16_with_bom(content, false);
        // The first byte is 0xFF — never legal UTF-8, which is exactly why read_to_string errored.
        assert_eq!(bytes[0], 0xFF);
        let text = decode_ini_text_bytes(&bytes);
        assert_eq!(
            parse_internet_shortcut_icon(&text),
            Some((r"D:\apps\steam\steam\games\ac24c6922f55c7dd7ab535f871c9adff300e6feb.ico".to_string(), 0))
        );
    }

    #[test]
    fn decode_ini_text_bytes_covers_every_encoding_the_shell_emits() {
        let body = "[InternetShortcut]\r\nURL=https://例子.test\r\nIconFile=C:\\图标\\a.ico\r\nIconIndex=3\r\n";
        let expected = Some((r"C:\图标\a.ico".to_string(), 3));
        // UTF-16 LE + BOM (Steam), UTF-16 BE + BOM, UTF-8 + BOM, and plain UTF-8 all decode equal.
        for bytes in [
            utf16_with_bom(body, false),
            utf16_with_bom(body, true),
            {
                let mut b = vec![0xEF, 0xBB, 0xBF];
                b.extend_from_slice(body.as_bytes());
                b
            },
            body.as_bytes().to_vec(),
        ] {
            assert_eq!(parse_internet_shortcut_icon(&decode_ini_text_bytes(&bytes)), expected);
        }
    }

    #[test]
    fn decode_ini_text_bytes_sniffs_bomless_utf16_le() {
        // Some writers omit the BOM. ASCII-in-UTF-16LE is half NUL on odd bytes — a NUL is legal
        // UTF-8, so `from_utf8` would otherwise accept it as a NUL-riddled string and the parse
        // would miss the section. The sniffer catches it.
        let body = "[InternetShortcut]\r\nURL=x\r\nIconFile=C:\\a.ico\r\nIconIndex=0\r\n";
        let mut bytes = Vec::new();
        for unit in body.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        assert_ne!(bytes[1], 0xFE, "no BOM in this fixture");
        assert_eq!(
            parse_internet_shortcut_icon(&decode_ini_text_bytes(&bytes)),
            Some((r"C:\a.ico".to_string(), 0))
        );
    }

    #[test]
    fn decode_ini_text_lossless_accepts_utf8_and_utf16_but_rejects_ansi() {
        // UTF-8 (bare + BOM) and UTF-16 (BOM) decode losslessly → a writer may safely rewrite.
        assert!(decode_ini_text_lossless(b"[InternetShortcut]\r\nURL=https://x").is_some());
        let utf16 = utf16_with_bom("[InternetShortcut]\r\nURL=https://例子.test\r\n", false);
        assert!(decode_ini_text_lossless(&utf16).is_some());
        // A GBK byte (0xC4 0xE3 = 你) is invalid UTF-8 with no NULs, so it falls through to the
        // lossy ANSI path → None: the writer must refuse rather than corrupt it.
        let ansi = b"[InternetShortcut]\r\nURL=https://x/\xC4\xE3\r\n";
        assert!(decode_ini_text_lossless(ansi).is_none());
        // The lossy READER still yields a (U+FFFD-bearing) string for the same bytes — reads tolerate it.
        assert!(decode_ini_text_bytes(ansi).contains('\u{fffd}'));
    }

    #[test]
    fn decode_ini_text_bytes_leaves_ordinary_utf8_untouched() {
        // Plain ASCII/UTF-8 (the common case) must not be misfired into the UTF-16 path — it has no
        // NUL bytes, so the sniffer declines and the strict UTF-8 read wins.
        let plain = "[InternetShortcut]\r\nURL=https://x\r\n";
        assert_eq!(decode_ini_text_bytes(plain.as_bytes()), plain);
    }

    #[test]
    fn parse_desktop_ini_icon_decodes_a_utf16_folder_ini() {
        // A folder another tool styled can carry a UTF-16 `desktop.ini`; the old from_utf8 read it
        // as "no icon". Now it decodes.
        let content = "[.ShellClassInfo]\r\nIconResource=C:\\图标\\folder.ico,2\r\nConfirmFileOp=0\r\n";
        let bytes = utf16_with_bom(content, false);
        assert_eq!(parse_desktop_ini_icon(&bytes), Some((r"C:\图标\folder.ico".to_string(), 2)));
    }
}
