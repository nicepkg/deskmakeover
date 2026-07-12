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
    if let Some(name) = recycle_bin_display_name() {
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
    // Oracle sorts by name, current-culture case-insensitive; ASCII-lowercase is the closest
    // portable approximation and is corrected on the Windows box if a locale diff surfaces.
    items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(items)
}

/// The shell parsing name for the Recycle Bin (the canonical CLSID form; the oracle's
/// `shell:RecycleBinFolder` resolves to the same folder).
const RECYCLE_BIN_PARSING: &str = "::{645FF040-5081-101B-9F08-00AA002F954E}";

/// The Recycle Bin's localized display name (回收站 / Recycle Bin / …), or `None` when the shell
/// item cannot be resolved — in which case the bin is skipped, exactly like the oracle probe.
fn recycle_bin_display_name() -> Option<String> {
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::{IShellItem, SHCreateItemFromParsingName, SIGDN_NORMALDISPLAY};
    // SAFETY: COM on the STA thread; the returned PWSTR is CoTaskMem-owned and freed here.
    unsafe {
        let item: IShellItem =
            SHCreateItemFromParsingName(&HSTRING::from(RECYCLE_BIN_PARSING), None).ok()?;
        let pw = item.GetDisplayName(SIGDN_NORMALDISPLAY).ok()?;
        let name = pw.to_string().ok();
        windows::Win32::System::Com::CoTaskMemFree(Some(pw.as_ptr() as *const _));
        name.filter(|n| !n.trim().is_empty())
    }
}

/// If `lnk_path` is one of OUR file wrappers — `<file>.lnk` beside a Hidden+System `<file>` that
/// the link targets — returns the reunified original RegularFile item. Any read failure or
/// mismatch means "just an ordinary shortcut" (`None`).
fn reunify_wrapper(lnk_path: &str) -> Option<DesktopItem> {
    let original = lnk_path.strip_suffix(".lnk").or_else(|| lnk_path.strip_suffix(".LNK"))?;
    let meta = std::fs::metadata(original).ok()?;
    if meta.is_dir() {
        return None;
    }
    use std::os::windows::fs::MetadataExt;
    const HIDDEN_SYSTEM: u32 = 0x0000_0002 | 0x0000_0004;
    if meta.file_attributes() & HIDDEN_SYSTEM != HIDDEN_SYSTEM {
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
