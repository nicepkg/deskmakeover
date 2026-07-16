//! The batch manifest the unelevated app hands the elevated helper for `apply|restore-desktop-items`.
//!
//! The manifest lives in a USER-WRITABLE location (the app's own data dir) — the unelevated app
//! cannot write into the helper's locked `%ProgramData%` dir — so it is FULLY UNTRUSTED input to
//! the privileged helper. This parser only turns bytes into typed rows and rejects malformed shapes;
//! the SECURITY decision (is this a real desktop `.lnk`/folder on a local fixed disk, under a
//! privileged root, pointing at a valid ICO) is made per row by [`crate::guards`] + the scope check
//! before any write. A hostile manifest can therefore never do more than name candidate targets the
//! guards will refuse.
//!
//! Format (strict, line-oriented, no serde — a smaller parse surface for a privileged binary):
//! * line 1 is the header `dm-desktop-items\t1` (magic + format version);
//! * every later non-empty line is one TAB-separated row (Windows filenames cannot contain a TAB,
//!   so a path can never smuggle a field separator).
//!   * apply row:   `kind \t target \t icon \t index \t expect_icon \t expect_index`
//!   * restore row: `kind \t target \t original \t expect_icon \t expect_index`
//! `expect_icon`/`expect_index` are the icon location the helper must find CURRENTLY on the target
//! (a compare-and-swap: never clobber an item an external process changed since the app read it).

/// The file-backed item kinds that can need elevation. Registry-backed kinds (System / Recycle Bin)
/// are per-user HKCU and never reach the helper, so they are deliberately absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Shortcut,
    UrlShortcut,
    Folder,
    RegularFile,
}

impl Kind {
    pub fn parse(value: &str) -> Option<Kind> {
        match value {
            "shortcut" => Some(Kind::Shortcut),
            "url" => Some(Kind::UrlShortcut),
            "folder" => Some(Kind::Folder),
            "file" => Some(Kind::RegularFile),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Shortcut => "shortcut",
            Kind::UrlShortcut => "url",
            Kind::Folder => "folder",
            Kind::RegularFile => "file",
        }
    }
}

/// One target the helper should restyle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyItem {
    pub kind: Kind,
    /// The `.lnk` / folder to restyle (validated as a real local target under a privileged root).
    pub target: String,
    /// The baked ICO the target's icon should point at (validated as a real, capped ICO).
    pub icon: String,
    pub index: i32,
    /// Compare-and-swap: the icon location the helper must find on the target NOW, or it skips it.
    pub expect_icon: String,
    pub expect_index: i32,
}

/// One target the helper should return to its captured original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreItem {
    pub kind: Kind,
    pub target: String,
    /// A file holding the original bytes to write back (the app staged it; helper reads it capped).
    pub original: String,
    /// Compare-and-swap: only restore when the live icon location is still the one WE applied.
    pub expect_icon: String,
    pub expect_index: i32,
}

const HEADER: &str = "dm-desktop-items\t1";
/// A desktop cannot realistically hold this many styleable items; the cap bounds a hostile manifest.
pub const MAX_ITEMS: usize = 4096;

fn check_header(text: &str) -> Result<&str, String> {
    let mut lines = text.splitn(2, '\n');
    let header = lines.next().unwrap_or("").trim_end_matches('\r');
    if header != HEADER {
        return Err(format!("bad manifest header {header:?}, expected {HEADER:?}"));
    }
    Ok(lines.next().unwrap_or(""))
}

/// A path field must be non-empty and, being a filename, must carry no control characters (a NUL or
/// newline in an argument to a privileged file op is never legitimate).
fn field(parts: &[&str], i: usize, row: usize, name: &str) -> Result<String, String> {
    let raw = parts
        .get(i)
        .ok_or_else(|| format!("manifest row {row}: missing {name}"))?;
    if raw.is_empty() {
        return Err(format!("manifest row {row}: empty {name}"));
    }
    if raw.chars().any(|c| c.is_control()) {
        return Err(format!("manifest row {row}: {name} contains a control character"));
    }
    Ok(raw.to_string())
}

fn index(parts: &[&str], i: usize, row: usize) -> Result<i32, String> {
    parts
        .get(i)
        .and_then(|v| v.parse::<i32>().ok())
        .ok_or_else(|| format!("manifest row {row}: index is not an integer"))
}

fn kind(parts: &[&str], row: usize) -> Result<Kind, String> {
    Kind::parse(parts.first().copied().unwrap_or(""))
        .ok_or_else(|| format!("manifest row {row}: unknown kind {:?}", parts.first().unwrap_or(&"")))
}

fn rows(body: &str) -> impl Iterator<Item = (usize, &str)> {
    body.lines()
        .map(|l| l.trim_end_matches('\r'))
        .enumerate()
        .filter(|(_, l)| !l.is_empty())
        .map(|(i, l)| (i + 2, l)) // 1-based line number (header is line 1)
}

/// Parse an `apply-desktop-items` manifest into typed rows (still fully untrusted).
pub fn parse_apply(text: &str) -> Result<Vec<ApplyItem>, String> {
    let body = check_header(text)?;
    let mut items = Vec::new();
    for (row, line) in rows(body) {
        if items.len() >= MAX_ITEMS {
            return Err(format!("manifest exceeds the {MAX_ITEMS}-item cap"));
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() != 6 {
            return Err(format!("manifest row {row}: expected 6 apply fields, got {}", parts.len()));
        }
        items.push(ApplyItem {
            kind: kind(&parts, row)?,
            target: field(&parts, 1, row, "target")?,
            icon: field(&parts, 2, row, "icon")?,
            index: index(&parts, 3, row)?,
            expect_icon: field(&parts, 4, row, "expect_icon")?,
            expect_index: index(&parts, 5, row)?,
        });
    }
    Ok(items)
}

/// Parse a `restore-desktop-items` manifest into typed rows.
pub fn parse_restore(text: &str) -> Result<Vec<RestoreItem>, String> {
    let body = check_header(text)?;
    let mut items = Vec::new();
    for (row, line) in rows(body) {
        if items.len() >= MAX_ITEMS {
            return Err(format!("manifest exceeds the {MAX_ITEMS}-item cap"));
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() != 5 {
            return Err(format!("manifest row {row}: expected 5 restore fields, got {}", parts.len()));
        }
        items.push(RestoreItem {
            kind: kind(&parts, row)?,
            target: field(&parts, 1, row, "target")?,
            original: field(&parts, 2, row, "original")?,
            expect_icon: field(&parts, 3, row, "expect_icon")?,
            expect_index: index(&parts, 4, row)?,
        });
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply_manifest(rows: &[&str]) -> String {
        std::iter::once(HEADER).chain(rows.iter().copied()).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn parses_a_well_formed_apply_manifest() {
        let text = apply_manifest(&[
            "shortcut\tC:\\Users\\Public\\Desktop\\Chrome.lnk\tC:\\assets\\a.ico\t0\tC:\\old.ico\t0",
            "folder\tC:\\Users\\Public\\Desktop\\Tools\tC:\\assets\\b.ico\t0\t%SystemRoot%\\shell32.dll\t3",
        ]);
        let items = parse_apply(&text).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].kind, Kind::Shortcut);
        assert_eq!(items[0].target, r"C:\Users\Public\Desktop\Chrome.lnk");
        assert_eq!(items[0].icon, r"C:\assets\a.ico");
        assert_eq!(items[0].expect_icon, r"C:\old.ico");
        assert_eq!(items[1].kind, Kind::Folder);
        assert_eq!(items[1].expect_index, 3);
    }

    #[test]
    fn parses_a_restore_manifest_and_round_trips_kinds() {
        let text = [HEADER, "shortcut\tC:\\a.lnk\tC:\\tmp\\orig.bin\tC:\\assets\\a.ico\t0"].join("\n");
        let items = parse_restore(&text).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].original, r"C:\tmp\orig.bin");
        for k in [Kind::Shortcut, Kind::UrlShortcut, Kind::Folder, Kind::RegularFile] {
            assert_eq!(Kind::parse(k.as_str()), Some(k));
        }
    }

    #[test]
    fn an_empty_body_is_a_valid_empty_batch() {
        assert_eq!(parse_apply(HEADER).unwrap(), vec![]);
        assert_eq!(parse_apply(&format!("{HEADER}\n")).unwrap(), vec![]);
        // Blank lines between rows are skipped, not treated as malformed rows.
        let text = apply_manifest(&["", "shortcut\tC:\\a.lnk\tC:\\a.ico\t0\tC:\\o.ico\t0", ""]);
        assert_eq!(parse_apply(&text).unwrap().len(), 1);
    }

    #[test]
    fn a_missing_or_wrong_header_is_refused() {
        assert!(parse_apply("").is_err());
        assert!(parse_apply("shortcut\tC:\\a.lnk\tC:\\a.ico\t0\tC:\\o.ico\t0").is_err());
        assert!(parse_apply("dm-desktop-items\t2\n").is_err(), "a future version must not be silently accepted");
    }

    #[test]
    fn wrong_field_counts_are_refused() {
        assert!(parse_apply(&apply_manifest(&["shortcut\tC:\\a.lnk\tC:\\a.ico"])).is_err(), "too few");
        assert!(
            parse_apply(&apply_manifest(&["shortcut\tC:\\a.lnk\tC:\\a.ico\t0\tC:\\o.ico\t0\textra"])).is_err(),
            "too many"
        );
        assert!(parse_restore(&[HEADER, "shortcut\tC:\\a.lnk\tC:\\o.bin\tC:\\i.ico"].join("\n")).is_err());
    }

    #[test]
    fn an_unknown_kind_or_non_integer_index_is_refused() {
        assert!(parse_apply(&apply_manifest(&["registry\tC:\\a.lnk\tC:\\a.ico\t0\tC:\\o.ico\t0"])).is_err());
        // System / RecycleBin are per-user HKCU and must never appear in an elevated batch.
        assert!(parse_apply(&apply_manifest(&["system\tC:\\a.lnk\tC:\\a.ico\t0\tC:\\o.ico\t0"])).is_err());
        assert!(parse_apply(&apply_manifest(&["shortcut\tC:\\a.lnk\tC:\\a.ico\tNaN\tC:\\o.ico\t0"])).is_err());
    }

    #[test]
    fn empty_or_control_character_fields_are_refused() {
        assert!(parse_apply(&apply_manifest(&["shortcut\t\tC:\\a.ico\t0\tC:\\o.ico\t0"])).is_err(), "empty target");
        // A NUL or newline in a path field is never legitimate for a privileged file op.
        assert!(parse_apply(&apply_manifest(&["shortcut\tC:\\a\0.lnk\tC:\\a.ico\t0\tC:\\o.ico\t0"])).is_err());
    }

    #[test]
    fn the_item_cap_bounds_a_hostile_manifest() {
        let row = "shortcut\tC:\\a.lnk\tC:\\a.ico\t0\tC:\\o.ico\t0";
        let rows: Vec<&str> = std::iter::repeat(row).take(MAX_ITEMS + 1).collect();
        assert!(parse_apply(&apply_manifest(&rows)).is_err());
    }
}
