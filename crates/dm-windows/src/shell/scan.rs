//! The desktop scan: enumerate the known-folder desktops, classify each entry, and read the
//! shortcut icon source. Ported from `DeskMakeover.Shell/DesktopScanner.cs`
//! (`FileSystemDesktopItemSource`). Runs on the STA thread because the icon read is COM.

use std::path::Path;
use std::sync::Arc;

use dm_domain::{
    DesktopItem, DesktopScanner, IconRef, IconSourceKind, ItemId, ItemKind, ItemState, PortResult,
};

use crate::classify::{classify_entry, display_name, is_ignored_entry};
use crate::com::StaExecutor;
use crate::shell::{known_folders, shell_link};

/// Scans the desktop into classified items. All enumeration + COM icon reads are marshalled onto
/// the STA thread.
pub struct WindowsScanner {
    exec: Arc<StaExecutor>,
}

impl WindowsScanner {
    pub fn new(exec: Arc<StaExecutor>) -> Self {
        Self { exec }
    }
}

impl DesktopScanner for WindowsScanner {
    fn scan(&self) -> PortResult<Vec<DesktopItem>> {
        self.exec.run(scan_blocking)?
    }
}

/// The scan body, executed on the STA thread. [WINDOWS-VERIFY] runtime.
fn scan_blocking() -> PortResult<Vec<DesktopItem>> {
    let mut items = Vec::new();
    for root in known_folders::desktop_roots()? {
        let read_dir = match std::fs::read_dir(&root) {
            Ok(rd) => rd,
            Err(_) => continue, // an unreadable root is skipped, not fatal
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if is_ignored_entry(&file_name) {
                continue;
            }
            // System-attributed items are invisible on the real desktop (oracle: Test-VisibleDesktopItem).
            if has_system_attribute(&path) {
                continue;
            }
            // A reparse point (symlink/junction) is not a safe styling target: writing desktop.ini
            // or a .lnk through it would follow the link elsewhere. Skip it defensively.
            if has_reparse_point(&path) {
                continue;
            }
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let kind = classify_entry(&file_name, is_dir);
            // Use the path only if it round-trips as UTF-8. to_string_lossy would substitute U+FFFD
            // for invalid units and silently corrupt the item id and every downstream path read, so
            // skip a non-representable entry instead.
            let path_str = match path.to_str() {
                Some(s) => s.to_string(),
                None => continue,
            };
            // Wrapper reunification: a `.lnk` this app created around a loose file (the original
            // sits beside it, Hidden+System, and IS the link target) re-presents as the ORIGINAL
            // RegularFile item — same id, same kind — so the ledger row, the CAS surface, and the
            // restore affordance all keep addressing one item across re-scans. Without this, every
            // wrapped file mutates into a foreign Shortcut item after its first apply (the oracle
            // shared this gap). [WINDOWS-VERIFY] runtime.
            if kind == ItemKind::Shortcut {
                if let Some(item) = reunify_wrapper(&path_str) {
                    items.push(item);
                    continue;
                }
            }
            items.push(DesktopItem {
                id: ItemId::from_source_path("filesystem", &path_str),
                name: display_name(&file_name, kind),
                icon: read_icon_source(kind, &path_str),
                path: path_str,
                kind,
                state: ItemState::Ready,
                requires_explicit_consent: false,
                status_message: None,
            });
        }
    }
    // The Recycle Bin is a VIRTUAL shell folder the filesystem enumeration can never see — inject
    // it explicitly so the mirror matches the real desktop 1:1 (oracle:
    // `DesktopPreviewService.AddRecycleBin`; identity via `RecycleBinProbe`). No display name →
    // skipped silently rather than failing the scan. [WINDOWS-VERIFY] runtime.
    if let Some(name) = shell_display_name(RECYCLE_BIN_PARSING) {
        items.push(DesktopItem {
            id: ItemId::from_raw("recyclebin"),
            name,
            path: RECYCLE_BIN_PARSING.to_string(),
            kind: ItemKind::RecycleBin,
            icon: None,
            state: ItemState::Ready,
            requires_explicit_consent: false,
            status_message: None,
        });
    }
    // The System desktop-namespace icons (This PC / User Files / Network / Control Panel) are also
    // VIRTUAL shell items the filesystem walk never sees — inject each ENABLED one, matching the real
    // desktop 1:1 (spec 06 §6). Enablement is the per-CLSID DWORD under `HideDesktopIcons`; a CLSID
    // whose display name cannot be resolved is skipped silently, like the bin. The CLSID rides the
    // item's parsing path (`::{GUID}`), which the System applier/reader parse back out.
    // [WINDOWS-VERIFY] runtime.
    for (id, clsid) in SYSTEM_DESKTOP_CLSIDS {
        if !system_icon_enabled(clsid) {
            continue;
        }
        let parsing = format!("::{clsid}");
        if let Some(name) = shell_display_name(&parsing) {
            items.push(DesktopItem {
                id: ItemId::from_raw(*id),
                name,
                path: parsing,
                kind: ItemKind::System,
                icon: None,
                state: ItemState::Ready,
                requires_explicit_consent: false,
                status_message: None,
            });
        }
    }
    // Oracle sorts by name, current-culture case-insensitive; ASCII-lowercase is the closest
    // portable approximation and is corrected on the Windows box if a locale diff surfaces.
    items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(items)
}

/// The shell parsing name for the Recycle Bin (the canonical CLSID form; the oracle's
/// `shell:RecycleBinFolder` resolves to the same folder).
const RECYCLE_BIN_PARSING: &str = "::{645FF040-5081-101B-9F08-00AA002F954E}";

/// The stylable System desktop-namespace CLSIDs (stable ItemId, CLSID GUID). The Recycle Bin is
/// injected separately above (it has its own empty/full pair). The GUIDs are the canonical Windows
/// desktop namespace class ids. [WINDOWS-VERIFY] runtime.
const SYSTEM_DESKTOP_CLSIDS: &[(&str, &str)] = &[
    ("thispc", "{20D04FE0-3AEA-1069-A2D8-08002B30309D}"),
    ("userfiles", "{59031a47-3f72-44a7-89c5-5595fe6b30ee}"),
    ("network", "{F02C1A0D-BE21-4350-88B0-7367FC96EF3C}"),
    ("controlpanel", "{5399E694-6CE5-4D6C-8FCE-1D8870FDCBA0}"),
];

/// Whether a desktop-namespace CLSID icon is currently SHOWN on the desktop. The per-CLSID DWORD
/// under `HideDesktopIcons\NewStartPanel` is 1 when HIDDEN; an ABSENT value or key (NotFound) means
/// shown — the default. Any OTHER read error (access denied, wrong type) does NOT prove the icon is
/// shown, so it FAILS CLOSED (treated as hidden → the item is not emitted) rather than conflating an
/// unreadable value with absence and styling an icon the user actually hid (codex System-review 🟡).
/// [WINDOWS-VERIFY] runtime.
fn system_icon_enabled(clsid: &str) -> bool {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    match hkcu
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\HideDesktopIcons\NewStartPanel")
    {
        Ok(key) => match key.get_value::<u32, _>(clsid) {
            Ok(hidden) => hidden == 0,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => true, // no override → shown
            Err(_) => false, // unreadable → fail closed (do not emit an unverifiable item)
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => true, // no policy key → all shown
        Err(_) => false, // policy key unreadable → fail closed
    }
}

/// A virtual shell item's localized display name (回收站 / 此电脑 / …), or `None` when the shell item
/// cannot be resolved — in which case it is skipped, exactly like the oracle's Recycle Bin probe.
fn shell_display_name(parsing: &str) -> Option<String> {
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::{IShellItem, SHCreateItemFromParsingName, SIGDN_NORMALDISPLAY};
    // SAFETY: COM on the STA thread; the returned PWSTR is CoTaskMem-owned and freed here.
    unsafe {
        let item: IShellItem =
            SHCreateItemFromParsingName(&HSTRING::from(parsing), None).ok()?;
        let pw = item.GetDisplayName(SIGDN_NORMALDISPLAY).ok()?;
        let name = pw.to_string().ok();
        windows::Win32::System::Com::CoTaskMemFree(Some(pw.as_ptr() as *const _));
        name.filter(|n| !n.trim().is_empty())
    }
}

/// If `lnk_path` is one of OUR file wrappers, returns the reunified original RegularFile item.
/// Ownership is proven by the DURABLE Description marker our wrapper writes (codex icons2-🟠6),
/// NOT by structural shape — a user's own Hidden+System file beside a same-named `.lnk` is never
/// mistaken for ours. Any read failure / non-marker means "just an ordinary shortcut" (`None`).
fn reunify_wrapper(lnk_path: &str) -> Option<DesktopItem> {
    // The ownership gate FIRST: no marker → not ours, regardless of structure.
    match shell_link::read_description(lnk_path).ok().flatten() {
        Some(desc) if desc == shell_link::WRAPPER_MARKER => {}
        _ => return None,
    }
    // Case-correct extension strip (handles `.LnK`, unicode-safe): the companion file is the
    // `.lnk` path minus its extension.
    let lnk = std::path::Path::new(lnk_path);
    if !lnk.extension().map(|e| e.eq_ignore_ascii_case("lnk")).unwrap_or(false) {
        return None;
    }
    let original = &lnk_path[..lnk_path.len() - 4]; // strip ".lnk"/".LNK"/… (4 bytes, ASCII dot+ext)
    let meta = std::fs::metadata(original).ok()?;
    if meta.is_dir() {
        return None;
    }
    use std::os::windows::fs::MetadataExt;
    const HIDDEN_SYSTEM: u32 = 0x0000_0002 | 0x0000_0004;
    if meta.file_attributes() & HIDDEN_SYSTEM != HIDDEN_SYSTEM {
        return None;
    }
    // Never derive an original that is itself a reparse point (symlink/junction) — a styled
    // reparse original would resolve elsewhere (codex icons2-🟠6).
    if has_reparse_point(std::path::Path::new(original)) {
        return None;
    }
    // The wrapper must actually POINT at the sibling (case-insensitive: NTFS paths).
    let target = shell_link::read_target(lnk_path).ok().flatten()?;
    if !target.eq_ignore_ascii_case(original) {
        return None;
    }
    let file_name = std::path::Path::new(original).file_name()?.to_str()?;
    Some(DesktopItem {
        id: ItemId::from_source_path("filesystem", original),
        name: display_name(file_name, ItemKind::RegularFile),
        icon: None,
        path: original.to_string(),
        kind: ItemKind::RegularFile,
        state: ItemState::Ready,
        requires_explicit_consent: false,
        status_message: None,
    })
}

fn read_icon_source(kind: ItemKind, path: &str) -> Option<IconRef> {
    match kind {
        ItemKind::Shortcut => shell_link::read_icon_location(path).ok().flatten().map(|(loc, idx)| {
            let source_kind = if loc.to_lowercase().ends_with(".ico") {
                IconSourceKind::File
            } else {
                IconSourceKind::ExecutableResource
            };
            IconRef { kind: source_kind, location: loc, index: idx }
        }),
        _ => None,
    }
}

fn has_system_attribute(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_SYSTEM: u32 = 0x0000_0004;
    std::fs::metadata(path)
        .map(|m| m.file_attributes() & FILE_ATTRIBUTE_SYSTEM != 0)
        .unwrap_or(false)
}

fn has_reparse_point(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    // symlink_metadata (lstat) does NOT follow the link, so the reparse flag is visible on the
    // entry itself rather than resolving to the target's attributes.
    std::fs::symlink_metadata(path)
        .map(|m| m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
        .unwrap_or(false)
}
