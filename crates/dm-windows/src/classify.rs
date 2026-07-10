//! Pure desktop-item classification, split out from the COM scan so it is unit-tested on the
//! Mac host. Ported from `DeskMakeover.Shell/DesktopScanner.cs` (`CreateItem`) and the icon
//! location parsing in `DesktopBakeService.TryExtractIconLocation` (reference `Split-IconLocation`).

use dm_domain::ItemKind;

/// The desktop-scan skips its own `desktop.ini` marker file (oracle: `DesktopScanner`).
pub fn is_ignored_entry(file_name: &str) -> bool {
    file_name.eq_ignore_ascii_case("desktop.ini")
}

/// Classifies a filesystem entry by directory-ness and extension, exactly as the oracle scanner
/// did: directories are folders, `.url`/`.lnk` are their shortcut kinds, everything else is a
/// loose file (which the unified look wraps).
pub fn classify_entry(file_name: &str, is_dir: bool) -> ItemKind {
    if is_dir {
        return ItemKind::Folder;
    }
    match extension_lower(file_name).as_deref() {
        Some("url") => ItemKind::UrlShortcut,
        Some("lnk") => ItemKind::Shortcut,
        _ => ItemKind::RegularFile,
    }
}

/// The display name shown for an item: folders and loose files keep their full file name;
/// shortcuts drop the extension (oracle: `Path.GetFileName` vs `GetFileNameWithoutExtension`).
pub fn display_name(file_name: &str, kind: ItemKind) -> String {
    match kind {
        ItemKind::Shortcut | ItemKind::UrlShortcut => file_stem(file_name).to_string(),
        _ => file_name.to_string(),
    }
}

/// Splits an icon location string `"path,index"` into its path and (possibly negative) resource
/// index. Surrounding quotes are stripped; the index is taken only when the text after the LAST
/// comma parses as an integer, so paths that themselves contain commas are preserved.
pub fn parse_icon_location(raw: &str) -> (String, i32) {
    let trimmed = raw.trim().trim_matches('"');
    if let Some(comma) = trimmed.rfind(',') {
        if let Ok(index) = trimmed[comma + 1..].trim().parse::<i32>() {
            return (trimmed[..comma].trim().to_string(), index);
        }
    }
    (trimmed.to_string(), 0)
}

fn extension_lower(file_name: &str) -> Option<String> {
    let dot = file_name.rfind('.')?;
    // A leading dot (dotfile) or trailing dot is not an extension.
    if dot == 0 || dot + 1 >= file_name.len() {
        return None;
    }
    Some(file_name[dot + 1..].to_ascii_lowercase())
}

fn file_stem(file_name: &str) -> &str {
    match file_name.rfind('.') {
        Some(dot) if dot > 0 => &file_name[..dot],
        _ => file_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_by_extension_and_directoryness() {
        assert_eq!(classify_entry("App.lnk", false), ItemKind::Shortcut);
        assert_eq!(classify_entry("Site.URL", false), ItemKind::UrlShortcut);
        assert_eq!(classify_entry("Reports", true), ItemKind::Folder);
        assert_eq!(classify_entry("notes.txt", false), ItemKind::RegularFile);
        assert_eq!(classify_entry("archive", false), ItemKind::RegularFile);
    }

    #[test]
    fn desktop_ini_is_ignored_case_insensitively() {
        assert!(is_ignored_entry("desktop.ini"));
        assert!(is_ignored_entry("Desktop.INI"));
        assert!(!is_ignored_entry("desktop.txt"));
    }

    #[test]
    fn display_name_drops_extension_only_for_shortcuts() {
        assert_eq!(display_name("App.lnk", ItemKind::Shortcut), "App");
        assert_eq!(display_name("Site.url", ItemKind::UrlShortcut), "Site");
        assert_eq!(display_name("notes.txt", ItemKind::RegularFile), "notes.txt");
        assert_eq!(display_name("Reports", ItemKind::Folder), "Reports");
    }

    #[test]
    fn parses_icon_location_with_index() {
        assert_eq!(parse_icon_location(r"C:\Windows\System32\shell32.dll,3"), (r"C:\Windows\System32\shell32.dll".into(), 3));
        assert_eq!(parse_icon_location(r"%SystemRoot%\imageres.dll,-54"), (r"%SystemRoot%\imageres.dll".into(), -54));
    }

    #[test]
    fn parses_icon_location_without_index_and_strips_quotes() {
        assert_eq!(parse_icon_location(r#""C:\Program Files\App\app.ico""#), (r"C:\Program Files\App\app.ico".into(), 0));
        assert_eq!(parse_icon_location(r"C:\App\icon.ico"), (r"C:\App\icon.ico".into(), 0));
    }

    #[test]
    fn preserves_paths_that_contain_commas() {
        // Only a trailing integer segment is treated as the index.
        assert_eq!(parse_icon_location(r"C:\a,b\icon.ico"), (r"C:\a,b\icon.ico".into(), 0));
        assert_eq!(parse_icon_location(r"C:\a,b\lib.dll,7"), (r"C:\a,b\lib.dll".into(), 7));
    }
}
