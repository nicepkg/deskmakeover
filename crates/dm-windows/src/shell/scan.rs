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
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let kind = classify_entry(&file_name, is_dir);
            let path_str = path.to_string_lossy().into_owned();
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
    // Oracle sorts by name, current-culture case-insensitive; ASCII-lowercase is the closest
    // portable approximation and is corrected on the Windows box if a locale diff surfaces.
    items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(items)
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
