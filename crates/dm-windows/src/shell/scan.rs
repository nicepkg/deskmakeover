//! The desktop scan: enumerate the known-folder desktops, classify each entry, and read the
//! shortcut icon source. Ported from `DeskMakeover.Shell/DesktopScanner.cs`
//! (`FileSystemDesktopItemSource`). Runs on the STA thread because the icon read is COM.

use std::path::Path;
use std::sync::Arc;

use dm_domain::{
    DesktopItem, DesktopScanner, IconRef, IconSourceKind, ItemId, ItemKind, ItemState, ItemSurface,
    PortResult,
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

/// One scan root and how it walks (ADR-0024). `surface` labels EVERY item found under the root —
/// a root states its surface explicitly, so a new scan root can never silently masquerade as
/// desktop truth. `recurse` descends into subfolders (the Start menu's app folders); `mandatory`
/// makes an unreadable root fail the whole scan loud (only the user's own desktop is mandatory —
/// 2026-07-19 vanish audit).
struct RootSpec {
    surface: ItemSurface,
    path: std::path::PathBuf,
    recurse: bool,
    mandatory: bool,
}

/// The scan body, executed on the STA thread. [WINDOWS-VERIFY] runtime.
fn scan_blocking() -> PortResult<Vec<DesktopItem>> {
    let mut items = Vec::new();
    // Labeled resolution (codex R-vanish P2): the user desktop is mandatory and identified by
    // NAME, never by vector position — a failed user-desktop lookup errors instead of letting
    // the Public root masquerade as a successful empty scan.
    let (user_root, public_root) = known_folders::desktop_roots_labeled()?;
    let mut roots: Vec<RootSpec> = vec![RootSpec {
        surface: ItemSurface::Desktop,
        path: user_root,
        recurse: false,
        mandatory: true,
    }];
    if let Some(public) = public_root {
        roots.push(RootSpec { surface: ItemSurface::Desktop, path: public, recurse: false, mandatory: false });
    }
    // Taskbar pins + Start menu roots (ADR-0024): best-effort exactly like the Public desktop — a
    // locked-down edition that fails one lookup narrows the SURFACES covered instead of failing
    // the whole scan. The pin root is flat; Start menu roots descend into app folders.
    // [WINDOWS-VERIFY] runtime.
    if let Some(pin) = known_folders::taskbar_pin_root() {
        roots.push(RootSpec { surface: ItemSurface::Taskbar, path: pin, recurse: false, mandatory: false });
    }
    for menu in known_folders::start_menu_roots_labeled() {
        roots.push(RootSpec { surface: ItemSurface::StartMenu, path: menu, recurse: true, mandatory: false });
    }
    for spec in &roots {
        walk_root(spec, &mut items)?;
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
            surface: ItemSurface::Desktop,
            parent: String::new(),
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
                surface: ItemSurface::Desktop,
                parent: String::new(),
                icon: None,
                state: ItemState::Ready,
                requires_explicit_consent: false,
                status_message: None,
            });
        }
    }
    // Store/MSIX (AppsFolder) entries with no walked `.lnk` cover (Task 11, ADR-0024): a virtual
    // Start-menu item per uncovered entry, so its icon becomes styleable through the wrapper
    // recipe. A walked item's display name covering an entry means the classic `.lnk` (or our
    // wrapper) already addresses that app — never synthesize a duplicate. Best-effort per root
    // doctrine: a failed enumeration (locked-down shell) or a missing Programs root NARROWS the
    // surface instead of failing the scan. [WINDOWS-VERIFY] runtime.
    if let Ok(entries) = crate::shell::appsfolder::enumerate() {
        if let Some(programs) = known_folders::user_programs_root() {
            let walked: std::collections::HashSet<String> =
                items.iter().map(|i| i.name.to_lowercase()).collect();
            items.extend(synthesize_appx_items(&walked, &entries, &programs));
        }
    }
    // Explorer folders (ADR-0004 D7, amendment 2026-10-16): every non-excluded folder under EVERY
    // FIXED DRIVE becomes a `Folder` item on the `explorer` surface, styled via `desktop.ini`
    // exactly like a desktop folder. Best-effort per-root doctrine: a missing/locked-down root
    // NARROWS the surface instead of failing the scan. [WINDOWS-VERIFY] runtime.
    let mut explorer_extensions: std::collections::HashSet<String> = std::collections::HashSet::new();
    {
        let mut explorer_items = Vec::new();
        // ONE shared BFS across every root (not a per-root walk): a single queue means a huge
        // `C:\` can never consume the whole folder cap before `D:\` is seen — depth-0 roots are
        // interleaved in enqueue order.
        walk_explorer_folders(
            &known_folders::explorer_folder_roots(),
            &mut explorer_items,
            &mut explorer_extensions,
        );
        // The DESKTOP surface already styles the top-level folders on the desktop, and a drive walk
        // descends into the Desktop folder too — the same folder would otherwise surface twice
        // under one `ItemId` (once desktop, once explorer). Keep the FIRST (the desktop-labeled
        // item) and drop the explorer duplicate so the ledger/commit never address one folder twice.
        let seen: std::collections::HashSet<String> = items.iter().map(|i| i.path.clone()).collect();
        items.extend(explorer_items.into_iter().filter(|i| !seen.contains(&i.path)));
    }
    // Explorer DRIVES (ADR-0004 D7): the surface's other half. [WINDOWS-VERIFY] runtime.
    items.extend(explorer_drive_items());
    // File TYPES (ADR-0004 D7 amendment 2026-10-16): one `FileType` item per ProgId the drive
    // walks SAW a file for — every type except images, exactly the `.ext` set collected by the
    // folder walk above, so a type with no file anywhere visible is never styled blind. Emitted
    // LAST so the shared per-scan preview budget is spent on desktop/taskbar/folder items first.
    items.extend(explorer_file_type_items(&explorer_extensions));
    // Oracle sorts by name, current-culture case-insensitive; ASCII-lowercase is the closest
    // portable approximation and is corrected on the Windows box if a locale diff surfaces.
    // FILE TYPES sort to the END (second key): their `.ext` names would otherwise sort FIRST
    // (ASCII `.` precedes every letter) and consume the shared per-scan preview budget ahead of
    // the desktop/taskbar/folder items — the exact starvation the BFS walk order exists to
    // prevent. Every other item keeps the oracle's pure name order (FileType is a new kind, so no
    // existing position moves), and the emission order above is what survives for equal keys.
    items.sort_by_key(|item| (item.kind == ItemKind::FileType, item.name.to_lowercase()));
    Ok(items)
}

/// Synthesizes virtual Start-menu items for AppsFolder entries NOT already covered by a walked
/// item name (Task 11). Pure — unit-tested on the Mac host; [`crate::shell::appsfolder`]
/// enumerates the real list. Rules:
/// * a display name matching any walked item (case-insensitive) → skip silently (the classic
///   `.lnk` / our wrapper already covers that app);
/// * a duplicate display name within the enumeration → first wins, later ones warn (the on-box
///   probe found none, but the shell is third-party-extensible);
/// * a display name that is not a safe file stem → skip + warn (the wrapper `.lnk` could never be
///   created under it, so the prospective path would be a lie).
/// The item's path is the PROSPECTIVE wrapper location `<user Programs root>\<DisplayName>.lnk` —
/// the addressing anchor (ItemId) apply later writes to.
fn synthesize_appx_items(
    walked_names: &std::collections::HashSet<String>,
    entries: &[crate::shell::appsfolder::AppsfolderEntry],
    programs_root: &Path,
) -> Vec<DesktopItem> {
    let mut out = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for entry in entries {
        let key = entry.display_name.to_lowercase();
        if walked_names.contains(&key) {
            continue;
        }
        if !seen.insert(key) {
            log::warn!(
                "appsfolder: duplicate display name {:?} skipped (first wins)",
                entry.display_name
            );
            continue;
        }
        if !crate::classify::is_safe_file_stem(&entry.display_name) {
            log::warn!(
                "appsfolder: display name {:?} is not a safe file name; skipped",
                entry.display_name
            );
            continue;
        }
        let path = programs_root.join(format!("{}.lnk", entry.display_name));
        let path_str = match path.to_str() {
            Some(s) => s.to_string(),
            None => continue,
        };
        out.push(DesktopItem {
            id: ItemId::from_source_path("filesystem", &path_str),
            name: entry.display_name.clone(),
            icon: None,
            path: path_str,
            kind: ItemKind::AppxShortcut,
            surface: ItemSurface::StartMenu,
            parent: String::new(),
            state: ItemState::Ready,
            requires_explicit_consent: false,
            status_message: None,
        });
    }
    out
}

/// Enumerate one root into `items`, honoring its surface + walk mode. The top-level `read_dir`
/// failure follows `mandatory` (loud error vs. best-effort skip); subfolder failures during a
/// recursive descent are ALWAYS best-effort (one locked app folder must not fail the scan).
fn walk_root(spec: &RootSpec, items: &mut Vec<DesktopItem>) -> PortResult<()> {
    let read_dir = match std::fs::read_dir(&spec.path) {
        Ok(rd) => rd,
        // The USER desktop failing to enumerate must be LOUD (2026-07-19 vanish audit):
        // silently treating it as empty binds the apply to a bogus "empty desktop" snapshot
        // and the UI cannot tell a bare desktop from a broken scan (OneDrive Files-On-Demand
        // hiccup, transient access denial). The PUBLIC root staying best-effort is deliberate
        // — it is often absent/ACL-restricted on managed machines.
        Err(e) if spec.mandatory => {
            return Err(dm_domain::PortError::Io(format!(
                "cannot enumerate the desktop folder {}: {e}",
                spec.path.display()
            )))
        }
        Err(e) => {
            log::warn!("scan skipped unreadable root {}: {e}", spec.path.display());
            return Ok(());
        }
    };
    walk_dir(spec, read_dir, "", 0, items);
    Ok(())
}

/// The recursive walk behind [`walk_root`]. Each entry carries its `parent` — the subfolder path
/// RELATIVE to the root (`""` at the root) — so the mirror can group Start menu tiles; desktop
/// and taskbar roots are flat and always produce `""`. Use the path only if it round-trips as
/// UTF-8: `to_string_lossy` would substitute U+FFFD for invalid units and silently corrupt the
/// item id and every downstream path read, so a non-representable entry is skipped instead.
/// [WINDOWS-VERIFY] runtime.
fn walk_dir(
    spec: &RootSpec,
    read_dir: std::fs::ReadDir,
    parent: &str,
    depth: u32,
    items: &mut Vec<DesktopItem>,
) {
    /// Depth cap for recursive surfaces: deep enough for every real Start menu tree, shallow
    /// enough that a misattributed junction can never make the walk unbounded.
    const START_MENU_MAX_DEPTH: u32 = 8;
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
        if is_dir && spec.recurse {
            // A recursive surface's folders are CONTAINERS (the Start menu's app folders): descend
            // into them instead of styling them. The folder itself never becomes an item — the
            // start-menu surface styles its SHORTCUTS; the spec-06 taxonomy is unchanged.
            if depth >= START_MENU_MAX_DEPTH {
                continue;
            }
            let child = if parent.is_empty() { file_name } else { format!("{parent}\\{file_name}") };
            match std::fs::read_dir(&path) {
                Ok(rd) => walk_dir(spec, rd, &child, depth + 1, items),
                Err(e) => log::warn!("scan skipped unreadable folder {}: {e}", path.display()),
            }
            continue;
        }
        if is_dir && spec.surface != ItemSurface::Desktop {
            // Taskbar pins are a flat `.lnk` set by definition — a directory there is not a pin,
            // and the desktop Folder taxonomy does not apply to it.
            continue;
        }
        let mut kind = classify_entry(&file_name, is_dir);
        let path_str = match path.to_str() {
            Some(s) => s.to_string(),
            None => continue,
        };
        // Wrapper reunification: a `.lnk` this app created around a loose file (the original
        // sits beside it, Hidden+System, and IS the link target) re-presents as the ORIGINAL
        // RegularFile item — same id, same kind — so the ledger row, the CAS surface, and the
        // restore affordance all keep addressing one item across re-scans. Without this, every
        // wrapped file mutates into a foreign Shortcut item after its first apply (the oracle
        // shared this gap). The reunified item inherits the ROOT's surface/parent — the label
        // always comes from the root that found it (ADR-0024). [WINDOWS-VERIFY] runtime.
        if kind == ItemKind::Shortcut {
            if let Some(mut item) = reunify_wrapper(&path_str) {
                item.surface = spec.surface;
                item.parent = parent.to_string();
                // A wrapper around an image/model must ALSO stay original (owner carve-out):
                // the reunified item is a RegularFile, so the same gate applies.
                keep_original_if_listed(&mut item);
                items.push(item);
                continue;
            }
            // A `.lnk` whose ARGUMENTS are the AppsFolder URI is a Store/UWP shortcut (Task 11) —
            // the args are the only reliable signal. One extra COM Load per `.lnk`; a failed or
            // garbage read (test fixtures included) leaves the item an ordinary Shortcut, never a
            // scan error.
            let is_appx = shell_link::read_arguments(&path_str)
                .ok()
                .flatten()
                .is_some_and(|args| crate::classify::parse_appx_aumid(&args).is_some());
            if is_appx {
                kind = ItemKind::AppxShortcut;
            }
        }
        let mut item = DesktopItem {
            id: ItemId::from_source_path("filesystem", &path_str),
            name: display_name(&file_name, kind),
            icon: read_icon_source(kind, &path_str),
            path: path_str,
            kind,
            surface: spec.surface,
            parent: parent.to_string(),
            state: ItemState::Ready,
            requires_explicit_consent: false,
            status_message: None,
        };
        // Owner carve-outs (2026-10-16 / 2026-10-16b): images and 3D/print model files
        // (`NEVER_RESTYLE_EXTENSIONS`) stay exactly as the shell shows them — no companion
        // wrapper, no styling. See `keep_original_if_listed`.
        keep_original_if_listed(&mut item);
        items.push(item);
    }
}

/// Marks a loose-file item that must stay EXACTLY as the shell shows it (owner carve-outs: images
/// and 3D/print models). Non-styleable on purpose: the desktop's loose-file mechanism wraps the
/// original in a styled companion `.lnk` and hides it — the very thing the owner vetoed twice
/// ("que esté como en la versión original"). `PreviewOnly` (not `Ready`) is what makes
/// `can_style()` false; the reason key is localized frontend-side. The item still rides the item
/// list so the RESTORE-FIRST planner reverts any wrapper a previous version created.
fn keep_original_if_listed(item: &mut DesktopItem) {
    if item.kind == ItemKind::RegularFile && never_restyle_extension(&item.name) {
        item.state = ItemState::PreviewOnly;
        item.status_message = Some("Icons_Reason_KeptOriginal".to_string());
    }
}

/// Caps for the `explorer` folder walk (ADR-0004 D7, rebased for the ALL-DRIVES walk in the
/// 2026-10-16 amendment). The depth cap is measured **from a drive root**, so the old
/// profile-relative depth 8 re-bases to 10 (the profile itself sits at depth 2 under `C:\`); the
/// count cap rises with it (5000) because drive-wide content replaces profile content. Both stay
/// the hard backstop against a freak tree stalling the scan or emitting tens of thousands of
/// items — the denylist below still cuts `target`/`node_modules` first, so real browsable content
/// is what reaches the cap. Tunable.
const EXPLORER_MAX_DEPTH: u32 = 10;
const EXPLORER_MAX_FOLDERS: usize = 5000;

/// Drive-root children the walk never descends into (ADR-0004 D7 amendment 2026-10-16): trees the
/// current user cannot write `desktop.ini` into without elevation. Emitting them would only
/// produce doomed per-item apply failures (access denied), so they are cut whole — styling them
/// needs a HOT-tier UAC decision that was explicitly NOT taken. Case-insensitive; the attribute
/// gate already cuts the Hidden/System ones (`Windows`, `$Recycle.Bin`, …) — they are listed too
/// as belt-and-suspenders for boxes that lost those attributes.
const EXPLORER_PROTECTED_ROOTS: &[&str] = &[
    "Program Files",
    "Program Files (x86)",
    "ProgramData",
    "Windows",
    "$Recycle.Bin",
    "System Volume Information",
    "Recovery",
    "MSOCache",
    "PerfLogs",
    "Documents and Settings",
];

/// Container-only directory names: descended into but NEVER styled as items. `Users` is the case
/// that matters — `C:\Users` itself is ACL-protected (writing `desktop.ini` there is admin-only),
/// yet the profile lives beneath it, so the walk must pass through. Same name deeper on a data
/// drive is skipped as an item too (one unstyled folder name beats a doomed write).
const EXPLORER_CONTAINER_ONLY: &[&str] = &["Users"];

/// Directory names that are build/VCS/cache/package artifacts — never styled. Writing
/// `desktop.ini` into them is catastrophic for perf + restore and invisible to the user anyway.
/// Case-insensitive. The DOT-prefixed entries are additionally caught wholesale by the
/// dot-prefix gate in the walk (belt-and-suspenders); the explosive build trees (`node_modules`,
/// `target`, `bin`, `obj`, …) are cut by name so they never fill the count cap. The DEPTH cap is
/// the backstop for arbitrary tooling internals: the owner box's
/// `Documents\godot\…\android-sdk\…\res\values-*` explosion lives at depth 9 — under this cap it
/// is never reached, so no budget is spent there and no `desktop.ini` lands in the SDK.
const EXPLORER_DENYLIST: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    "bin",
    "obj",
    "__pycache__",
    "venv",
    "packages",
    // Tooling SDKs — a user never browses these for icons and they hold deep explosions
    // (`android-sdk\cmdline-tools\latest\lib\zipflinger`, NDK python stdlib, …). Cutting the root
    // keeps every `desktop.ini` out of the SDK (owner box: `Documents\godot\…\android-sdk`).
    "android-sdk",
    "cmdline-tools",
    "platform-tools",
    "build-tools",
    "system-images",
    ".git",
    ".svn",
    ".hg",
    ".venv",
    ".cache",
    ".cargo",
    ".rustup",
    ".npm",
    ".gradle",
    "AppData",
];

/// Emits the FOLDER items for the `explorer` surface across `roots` — every fixed-drive root, one
/// shared breadth-first queue (ADR-0004 D7; amendment 2026-10-16). Unlike the Start menu (whose
/// folders are containers only), a folder here is BOTH an item (its `desktop.ini` gets styled)
/// AND a container (we descend to its subfolders). Files are never emitted as items — but their
/// EXTENSIONS are collected into `extensions` (lowercase, no dot), the `.ext` universe the
/// file-type pass later resolves to ProgIds: a type is only ever styled when the user actually
/// HAS files of it on a walked drive. Best-effort: an unreadable subfolder is skipped, never a
/// scan failure.
///
/// **Breadth-first, not depth-first.** The per-scan icon-preview budget (`SCAN_SOURCE_BUDGET` in
/// the host) is SHARED across every surface and, once exhausted, turns the remaining items
/// `Icons_Reason_TooMany` → not-styleable. A DFS would dive into the first huge subtree — the
/// owner box: `Documents\godot\…\android-sdk\…\res\values-*`, thousands of dirs — and spend the
/// whole budget there, starving the profile's LATER content folders (Pictures, Music, Videos, …)
/// of a preview. BFS emits the shallow, browsable content FIRST, so the budget is spent on what
/// the user actually browses and only ever overflows onto deep tooling junk (fine to leave
/// not-styleable). The depth cap additionally cuts a depth-11+ explosion outright. Every root is
/// seeded at depth 0 into the ONE queue, so a giant `C:\` never eats the whole count cap before
/// `D:\` gets seen (the roots would otherwise be walked sequentially, last one starved).
fn walk_explorer_folders(
    roots: &[std::path::PathBuf],
    items: &mut Vec<DesktopItem>,
    extensions: &mut std::collections::HashSet<String>,
) {
    let mut queue: std::collections::VecDeque<(std::path::PathBuf, u32)> =
        std::collections::VecDeque::new();
    for root in roots {
        queue.push_back((root.clone(), 0));
    }
    let mut count = 0usize;
    while let Some((dir, depth)) = queue.pop_front() {
        if count >= EXPLORER_MAX_FOLDERS {
            break;
        }
        // At the depth ceiling: don't even read this dir's children (they'd be depth+1 > cap).
        if depth >= EXPLORER_MAX_DEPTH {
            continue;
        }
        let rd = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(e) => {
                log::warn!("explorer scan skipped unreadable folder {}: {e}", dir.display());
                continue;
            }
        };
        for entry in rd.flatten() {
            if count >= EXPLORER_MAX_FOLDERS {
                break;
            }
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if is_ignored_entry(&file_name) {
                continue;
            }
            // The explorer surface styles FOLDERS (+ drives and file TYPES, both enumerated
            // separately); a loose file is never itself an item, but its EXTENSION feeds the
            // file-type pass (ADR-0004 D7 amendment 2026-10-16).
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                if let Some(ext) = std::path::Path::new(&file_name).extension() {
                    extensions.insert(ext.to_string_lossy().to_lowercase());
                }
                continue;
            }
            // Dot-prefixed dirs (`.git`, `.bun`, `.vscode`, `.cargo`, `.venv`, `.cache`, `.local`, …)
            // are the Windows convention for tooling/package caches — dot-prefix is NOT an attribute
            // hidden bit, so the attribute gate below misses them. They hold thousands of store
            // subfolders that blow the preview budget and starve every REAL folder past them into
            // `Icons_Reason_TooMany` → not-styleable (owner-box `.bun\install\cache` report
            // 2026-10-13). Skip them wholesale — a user never browses them for icons.
            if file_name.starts_with('.')
                || EXPLORER_DENYLIST.iter().any(|d| file_name.eq_ignore_ascii_case(d))
            {
                continue;
            }
            // Admin-only trees: never styled, never descended (ADR-0004 D7 amendment 2026-10-16).
            if EXPLORER_PROTECTED_ROOTS.iter().any(|d| file_name.eq_ignore_ascii_case(d)) {
                continue;
            }
            let path = entry.path();
            let path_str = match path.to_str() {
                Some(s) => s.to_string(),
                None => continue,
            };
            // Attribute exclusions: hidden, system, or a reparse point. Hidden/System covers
            // `AppData` and app-managed trees (what Explorer hides by default — the owner never
            // sees them). Reparse is skipped so we never write `desktop.ini` *through* a
            // junction/symlink and never cycle (OneDrive, redirected Documents). An unreadable
            // attribute read fails closed.
            match crate::shell::attrs::get(&path_str) {
                Ok(a) => {
                    if a & (crate::shell::attrs::HIDDEN | crate::shell::attrs::SYSTEM) != 0 {
                        continue;
                    }
                    if a & crate::shell::attrs::REPARSE_POINT != 0 {
                        continue;
                    }
                }
                Err(_) => continue,
            }
            // Container-only roots (the profile's `Users` parent): descended, never emitted.
            if !EXPLORER_CONTAINER_ONLY.iter().any(|d| file_name.eq_ignore_ascii_case(d)) {
                count += 1;
                items.push(DesktopItem {
                    id: ItemId::from_source_path("filesystem", &path_str),
                    name: display_name(&file_name, ItemKind::Folder),
                    icon: read_icon_source(ItemKind::Folder, &path_str),
                    path: path_str,
                    kind: ItemKind::Folder,
                    surface: ItemSurface::Explorer,
                    parent: String::new(),
                    state: ItemState::Ready,
                    requires_explicit_consent: false,
                    status_message: None,
                });
            }
            // Enqueue for descent (breadth-first): processed AFTER every already-queued shallower
            // folder, so a deep subtree never preempts shallow content for the count cap / budget.
            queue.push_back((path, depth + 1));
        }
    }
}

/// The explorer surface's DRIVE half (ADR-0004 D7): one `Drive` item per visible FIXED root, each
/// styled via its per-user registry `DefaultIcon` (routed by `kind`, distinct from the folder
/// `desktop.ini` writer). A drive's display name comes from the shell exactly like the bin /
/// namespace items ("SYSTEM (C:)"); an unresolvable shell name falls back to `(C:)` rather than
/// dropping the drive. `icon` stays `None` by design — the effective icon is a LIVE shell read at
/// extract time (`shell_item_image`), never a scan-time snapshot. Best-effort: an empty fixed-drive
/// enumeration just narrows the surface (the `visible_fixed_drives` test owns the on-box proof).
/// [WINDOWS-VERIFY] runtime.
fn explorer_drive_items() -> Vec<DesktopItem> {
    known_folders::visible_fixed_drives()
        .into_iter()
        .map(|drive_path| {
            let letter = drive_path.chars().next().unwrap_or('?');
            let name = shell_display_name(&drive_path)
                .filter(|n| !n.trim().is_empty())
                .unwrap_or_else(|| format!("({letter}:)"));
            DesktopItem {
                id: ItemId::from_source_path("filesystem", &drive_path),
                name,
                path: drive_path,
                kind: ItemKind::Drive,
                surface: ItemSurface::Explorer,
                parent: String::new(),
                icon: None,
                state: ItemState::Ready,
                requires_explicit_consent: false,
                status_message: None,
            }
        })
        .collect()
}

/// The shell parsing name for the Recycle Bin (the canonical CLSID form; the oracle's
/// `shell:RecycleBinFolder` resolves to the same folder).
const RECYCLE_BIN_PARSING: &str = "::{645FF040-5081-101B-9F08-00AA002F954E}";

/// The `explorer` surface's FILE-TYPE half (ADR-0004 D7 amendment 2026-10-16): one `FileType`
/// item per ProgId the drive walk actually SAW files of, filtered to the types whose icon can be
/// restyled. The type set comes from `extensions` (collected by [`walk_explorer_folders`]), never
/// from enumerating the registry wholesale — a type with no file anywhere the user browses is
/// never styled blind, and the item count stays proportional to real content.
///
/// Gates, in order (each one is part of the amendment's contract, not an optimization):
/// 1. **Images are never restyled** — owner carve-out: `PerceivedType = image` OR membership in
///    [`NEVER_RESTYLE_EXTENSIONS`], evaluated **per ProgId**: one `DefaultIcon` serves every extension of
///    an association, so a ProgId reachable from even ONE image extension is untouchable. A
///    thumbnail/preview type keeps its native identity.
/// 2. **Extension → ProgId** (`HKCR\<ext>\(Default)`), and the ProgId must pass the same
///    single-key-component validation apply uses (never a `\`-bearing association name).
/// 3. **One item per ProgId** — extensions sharing an association (`.htm`/`.html`) share ONE
///    registry value, so they must be ONE item; two writers of one value would CAS-conflict
///    forever.
/// 4. **The type must HAVE a `DefaultIcon`** — nothing to style otherwise (`.lnk`-class types
///    resolve through their target instead, so they simply fall out here).
/// 5. **Per-file types are skipped**: a `DefaultIcon` carrying an unexpanded `%1`/`%L`/`%l`
///    resolves to *the file itself* (`exefile` and friends) — a type-level override there would
///    collapse every executable's individual icon into ONE glyph. This is the honest floor of
///    "all program icons": per-program identity lives inside each `.exe` (ADR-0024's running-
///    window limit, same platform fact) and there is no per-file lever.
///
/// `icon` carries the type's effective `DefaultIcon` resource, captured at scan so extraction and
/// the styled-residue provenance guard work off the same location the shell shows.
/// Best-effort per-type doctrine: any failure skips that type, never the scan. [WINDOWS-VERIFY]
/// runtime (the merged HKCR reads + `PerceivedType`).
fn explorer_file_type_items(extensions: &std::collections::HashSet<String>) -> Vec<DesktopItem> {
    use winreg::enums::HKEY_CLASSES_ROOT;
    use winreg::RegKey;
    if extensions.is_empty() {
        return Vec::new();
    }
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    let mut sorted: Vec<&String> = extensions.iter().collect();
    sorted.sort(); // deterministic emission order (the item list is name-sorted later anyway)

    // Pass 1 — resolve every seen extension to its ProgId and mark the image-reachable ones.
    // The image carve-out runs PROGID-LEVEL on purpose: one `DefaultIcon` value serves every
    // extension of a ProgId, so if a SINGLE image extension reaches it (however odd a mapping),
    // styling it would restyle image files too — the carve-out must win over the per-extension
    // view. Extensions without a valid association never enter the candidate list.
    let mut candidates: Vec<(String, String, bool)> = Vec::new(); // (ext, prog_id, is_image_reachable)
    for ext in sorted {
        let static_excluded = NEVER_RESTYLE_EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e));
        let assoc = match hkcr.open_subkey(format!(".{ext}")) {
            Ok(k) => k,
            Err(_) => continue, // no association at all → nothing to style
        };
        // The systematic image filter: `PerceivedType` covers image types the static list misses.
        let perceived_image = assoc
            .get_value::<String, _>("PerceivedType")
            .map(|p| p.eq_ignore_ascii_case("image"))
            .unwrap_or(false);
        let prog_id = match assoc.get_value::<String, _>("") {
            Ok(p) if !p.trim().is_empty() => p,
            _ => continue,
        };
        if crate::apply::filetype::prog_id_from_path(&prog_id).is_err() {
            continue;
        }
        candidates.push((ext.to_string(), prog_id, static_excluded || perceived_image));
    }

    // Pass 2 — one item per non-image ProgId, with the `DefaultIcon` gates.
    let image_progs: std::collections::HashSet<String> = candidates
        .iter()
        .filter(|c| c.2)
        .map(|c| c.1.to_lowercase())
        .collect();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (ext, prog_id, _) in candidates {
        let prog_key = prog_id.to_lowercase();
        if image_progs.contains(&prog_key) {
            continue; // shares its icon value with an image type — never restyled
        }
        if !seen.insert(prog_key) {
            continue; // another extension of the SAME ProgId — one registry target, one item
        }
        let icon_key = match hkcr.open_subkey(format!(r"{prog_id}\DefaultIcon")) {
            Ok(k) => k,
            Err(_) => continue,
        };
        let value = match icon_key.get_value::<String, _>("") {
            Ok(v) if !v.trim().is_empty() => v,
            _ => continue,
        };
        if value.contains("%1") || value.contains("%L") || value.contains("%l") {
            continue; // per-file type — styling it would flatten every file of the type (gate 5)
        }
        let (location, index) = crate::classify::parse_icon_location(&value);
        if location.trim().is_empty() {
            continue;
        }
        out.push(DesktopItem {
            id: ItemId::from_source_path("filetype", &prog_id),
            name: format!(".{ext}"),
            path: prog_id,
            kind: ItemKind::FileType,
            surface: ItemSurface::Explorer,
            parent: String::new(),
            icon: Some(IconRef { kind: icon_ref_kind(&location), location, index }),
            state: ItemState::Ready,
            requires_explicit_consent: false,
            status_message: None,
        });
    }
    out
}

/// Extensions whose icons are NEVER restyled — the owner's explicit carve-outs (2026-10-16:
/// "todos los iconos de archivo **menos los de imágenes**"; 2026-10-16b: "también los `.3mf`").
/// Belt and suspenders over the systematic `PerceivedType = image` check: a thumbnail/preview type
/// keeps its native identity, and some image types carry no perceived type at all. Case-insensitive.
/// Applied to BOTH halves: the per-ProgId file TYPE (via [`explorer_file_type_items`]) and a loose
/// FILE on a styled surface (the desktop's companion-wrapper mechanism must never wrap these).
const NEVER_RESTYLE_EXTENSIONS: &[&str] = &[
    // Images (owner carve-out).
    "png", "jpg", "jpeg", "jpe", "jfif", "gif", "bmp", "dib", "ico", "cur", "ani", "apng",
    "tif", "tiff", "wmf", "emf", "svg", "svgz", "webp", "heic", "heif", "avif", "avifs", "tga",
    "psd", "psb", "xcf", "dng", "cr2", "cr3", "nef", "orf", "rw2", "arw", "mrw", "raf", "pef",
    "jxl", "jp2", "j2k", "dds", "exr", "hdr", "qoi",
    // 3D / print models (owner 2026-10-16b: `.3mf` "que esté como en la versión original").
    "3mf",
];

/// Whether a file name's extension is on the never-restyle list (case-insensitive). The
/// loose-file gate: an image or a 3D/print model is left exactly as the shell shows it.
fn never_restyle_extension(file_name: &str) -> bool {
    match crate::classify::extension_lower(file_name) {
        Some(ext) => NEVER_RESTYLE_EXTENSIONS.iter().any(|e| e.eq_ignore_ascii_case(&ext)),
        None => false,
    }
}

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
/// under `HideDesktopIcons\NewStartPanel` is 1 when HIDDEN, 0 when SHOWN. These four namespace icons
/// (This PC / User Files / Network / Control Panel) are OPT-IN: a clean Windows 10/11 profile shows
/// only the Recycle Bin (handled separately), and the Desktop Icon Settings dialog WRITES an explicit
/// `0` when the user enables one. So an ABSENT value or key is the default-HIDDEN state, NOT shown —
/// treating absence as shown injected phantom desktop tiles + style targets on a normal profile
/// (codex R2 D-3). We therefore require an explicit `0` to emit the item; absence and any read error
/// (access denied, wrong type) alike FAIL CLOSED (hidden → not emitted). [WINDOWS-VERIFY] runtime: the
/// per-CLSID default and the `NewStartPanel` vs `ClassicStartMenu` distinction need box confirmation;
/// the fully-correct form enumerates the live desktop shell namespace (a larger [WV] follow-up).
fn system_icon_enabled(clsid: &str) -> bool {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    match hkcu
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\HideDesktopIcons\NewStartPanel")
    {
        Ok(key) => match key.get_value::<u32, _>(clsid) {
            Ok(shown_flag) => shown_flag == 0, // explicit 0 = shown; explicit 1 = hidden
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false, // opt-in, no explicit 0 → hidden
            Err(_) => false, // unreadable → fail closed (do not emit an unverifiable item)
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false, // no policy key → opt-in icons hidden
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
        // Placeholder: the walk that FOUND the wrapper overwrites surface/parent with its root's
        // labels (ADR-0024) — wrappers are desktop-born, so desktop is the honest default.
        surface: ItemSurface::Desktop,
        parent: String::new(),
        state: ItemState::Ready,
        requires_explicit_consent: false,
        status_message: None,
    })
}

fn read_icon_source(kind: ItemKind, path: &str) -> Option<IconRef> {
    match kind {
        ItemKind::Shortcut => shell_link::read_icon_location(path).ok().flatten().map(|(loc, idx)| {
            IconRef { kind: icon_ref_kind(&loc), location: loc, index: idx }
        }),
        // A `.url`'s icon is its `[InternetShortcut]` `IconFile` (a `.ico`/`.exe` on disk) — read it
        // straight so extraction honours the REAL game/app icon via `icon_resource_image`, instead
        // of falling through to the shell's generic internet-shortcut render (owner report
        // 2026-07-15: Steam `.url` icons showed a tiny generic glyph). Encoding-aware because Steam
        // writes these files as UTF-16 LE. An absent/unreadable IconFile → None → shell fallback.
        ItemKind::UrlShortcut => crate::durable::read_capped(path, crate::durable::SHORTCUT_READ_CAP)
            .ok()
            .and_then(|bytes| {
                crate::textfmt::parse_internet_shortcut_icon(&crate::textfmt::decode_ini_text_bytes(&bytes))
            })
            .filter(|(loc, _)| !loc.trim().is_empty())
            .map(|(loc, idx)| IconRef { kind: icon_ref_kind(&loc), location: loc, index: idx }),
        _ => None,
    }
}

/// Classifies an icon location by extension: a `.ico` is a plain image File, anything else
/// (`.exe`/`.dll`) an executable resource whose `index` selects the frame.
fn icon_ref_kind(location: &str) -> IconSourceKind {
    if location.to_lowercase().ends_with(".ico") {
        IconSourceKind::File
    } else {
        IconSourceKind::ExecutableResource
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

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(surface: ItemSurface, root: &Path, recurse: bool, mandatory: bool) -> RootSpec {
        RootSpec { surface, path: root.to_path_buf(), recurse, mandatory }
    }

    /// Creates a plain byte file at `rel` under `root` (mkdir -p). The walk treats `.lnk` names
    /// as ordinary shortcut candidates: reunify's ownership gate rejects these foreign bytes
    /// (no COM marker) exactly like a user's own shortcut.
    fn touch(root: &Path, rel: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"fixture").unwrap();
    }

    fn walk(root: &Path, surface: ItemSurface, recurse: bool) -> Vec<DesktopItem> {
        let mut items = Vec::new();
        walk_root(&spec(surface, root, recurse, false), &mut items).unwrap();
        items
    }

    #[test]
    fn the_explorer_walk_emits_folders_recurses_and_honors_exclusions() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // Visible folders: emitted AND descended into (a folder is both item + container).
        touch(root, "Docs/a.txt");
        touch(root, "Docs/Sub/b.txt");
        // A denylist dir: skipped entirely, including its descent (the repo's target/node_modules).
        touch(root, "node_modules/dep/c.txt");
        // A loose file at the root: never an explorer ITEM (its `.txt` extension, however, feeds
        // the file-type pass — ADR-0004 D7 amendment 2026-10-16).
        touch(root, "loose.txt");
        // A hidden folder (attribute-gated, like AppData): skipped.
        let hidden = root.join("Secret");
        std::fs::create_dir_all(&hidden).unwrap();
        crate::shell::attrs::set(&hidden.to_string_lossy(), crate::shell::attrs::HIDDEN).unwrap();

        let mut items = Vec::new();
        let mut exts = std::collections::HashSet::new();
        walk_explorer_folders(&[root.to_path_buf()], &mut items, &mut exts);

        let paths: std::collections::HashSet<String> = items.iter().map(|i| i.path.clone()).collect();
        let p = |s: &str| root.join(s).to_string_lossy().into_owned();
        // Emitted: the visible child folder AND the descended subfolder.
        assert!(paths.contains(&p("Docs")), "visible folder emitted");
        assert!(paths.contains(&p("Docs\\Sub")), "subfolder emitted via descent");
        // Skipped: the denylist dir + its descent, the hidden folder, and the loose file.
        assert!(!paths.contains(&p("node_modules")), "denylist dir skipped");
        assert!(!paths.contains(&p("node_modules\\dep")), "denylist descent skipped");
        assert!(!paths.contains(&p("Secret")), "hidden folder skipped");
        assert!(!paths.contains(&p("loose.txt")), "files are never explorer items");
        // ...but their extensions ARE collected for the file-type pass (amendment 2026-10-16):
        // seen files feed it, denylisted/hidden ones are never reached (same budget posture).
        assert!(exts.contains("txt"), "seen extension collected for file types");
        assert!(!exts.contains("c"), "files under a denylisted tree never widen the type set");
        // Every emitted item is a Folder on the explorer surface.
        assert!(items
            .iter()
            .all(|i| i.kind == ItemKind::Folder && i.surface == ItemSurface::Explorer));
    }

    #[test]
    fn the_never_restyle_list_covers_images_and_3d_models() {
        // Owner carve-outs 2026-10-16 (images) + 2026-10-16b (`.3mf`): the list is the ONE source
        // the loose-file gate and the file-TYPE pass share, so a gap here is a gap in both.
        for name in [
            "gato.png", "foto.JPEG", "x.webp", "a.jxl", "b.psb", "c.svgz", "d.dds", "e.exr",
            "modelo.3mf", "MODELO.3MF",
        ] {
            assert!(never_restyle_extension(name), "{name} must never be restyled");
        }
        for name in ["notas.txt", "app.exe", "lib.dll", "archivo.zip", "sin_extension", "punto."] {
            assert!(!never_restyle_extension(name), "{name} stays styleable");
        }
    }

    #[test]
    fn loose_images_and_model_files_are_kept_original() {
        // The desktop's loose-file mechanism wraps the original in a styled companion `.lnk` and
        // hides it — exactly what the owner vetoed ("que esté como en la versión original"). An
        // image or a 3D/print model must arrive NON-styleable, with its stable reason key.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(root, "gato.png");
        touch(root, "foto.JPEG"); // case-insensitive
        touch(root, "modelo.3mf");
        touch(root, "notas.txt"); // a normal loose file keeps its Ready state
        touch(root, "Docs/readme.txt");

        let items = walk(root, ItemSurface::Desktop, false);
        let by_name = |n: &str| {
            items.iter().find(|i| i.name == n).unwrap_or_else(|| panic!("missing {n}"))
        };
        for n in ["gato.png", "foto.JPEG", "modelo.3mf"] {
            let it = by_name(n);
            assert_eq!(it.kind, ItemKind::RegularFile, "{n} stays a loose file (restore identity)");
            assert_eq!(it.state, ItemState::PreviewOnly, "{n} must not be styleable");
            assert!(!it.can_style(), "{n} must not be styleable");
            assert_eq!(
                it.status_message.as_deref(),
                Some("Icons_Reason_KeptOriginal"),
                "{n} carries the stable reason key"
            );
        }
        let txt = by_name("notas.txt");
        assert_eq!(txt.state, ItemState::Ready);
        assert!(txt.can_style(), "an ordinary loose file is untouched by the gate");
        assert!(txt.status_message.is_none());
        assert_eq!(by_name("Docs").state, ItemState::Ready, "folders are unaffected");
    }

    #[test]
    fn the_scan_emits_one_drive_item_per_fixed_root() {
        // The drive half of the explorer surface: one item per `visible_fixed_drives` root, all
        // carrying the shape the apply pipeline keys off — `kind=Drive` (routes to the per-user
        // registry writer, not the `desktop.ini` one), `surface=Explorer` (governed by the
        // explorer toggle, never rendered in the canvas), a stable filesystem id, and NO scan-time
        // icon snapshot (the drive's effective icon is read live at extract time).
        let drives = explorer_drive_items();
        let roots = known_folders::visible_fixed_drives();
        assert_eq!(drives.len(), roots.len(), "one item per fixed root");
        assert!(drives.iter().any(|d| d.path == "C:\\"), "the system drive is styled");
        for item in &drives {
            assert_eq!(item.kind, ItemKind::Drive);
            assert_eq!(item.surface, ItemSurface::Explorer);
            assert!(item.path.ends_with(":\\"), "bare drive root, got {}", item.path);
            assert_eq!(item.id, ItemId::from_source_path("filesystem", &item.path));
            assert!(!item.name.trim().is_empty(), "every drive shows a name");
            assert_eq!(item.state, ItemState::Ready);
            assert!(item.icon.is_none(), "drive icon source is live, not scan-time");
        }
    }

    #[test]
    fn protected_roots_never_emit_or_descend_and_users_is_container_only() {
        // The all-drives walk (ADR-0004 D7 amendment 2026-10-16) must NOT offer trees the current
        // user cannot write `desktop.ini` into (each emitted one would only become a doomed
        // per-item apply failure), while still walking THROUGH the ACL-protected `Users` parent to
        // reach the profile beneath it.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(root, "Program Files/App/readme.txt"); // protected → no emit, no descent
        touch(root, "Windows/System32/x.txt"); // protected (also Hidden/System-attribute gated)
        touch(root, "Users/Melu/Docs/report.pjy"); // container-only → descended, never an item
        touch(root, "Music/note.txt"); // ordinary folder → styled as usual

        let mut items = Vec::new();
        let mut exts = std::collections::HashSet::new();
        walk_explorer_folders(&[root.to_path_buf()], &mut items, &mut exts);

        let paths: std::collections::HashSet<String> = items.iter().map(|i| i.path.clone()).collect();
        let p = |s: &str| root.join(s).to_string_lossy().into_owned();
        assert!(!paths.contains(&p("Program Files")), "protected root not styled");
        assert!(!paths.contains(&p("Program Files\\App")), "protected root not descended");
        assert!(!paths.contains(&p("Windows")), "protected root not styled");
        assert!(!paths.contains(&p("Windows\\System32")), "protected root not descended");
        assert!(!paths.contains(&p("Users")), "`Users` itself is never styled");
        assert!(paths.contains(&p("Users\\Melu")), "`Users` is still descended to reach the profile");
        assert!(paths.contains(&p("Users\\Melu\\Docs")), "profile content is styled");
        assert!(paths.contains(&p("Music")), "ordinary folders are styled");
        // Files behind a protected root never widen the type set either (never visited).
        assert!(!exts.contains("x"), "protected-descent files are unseen");
        assert!(exts.contains("pjy"), "profile files behind `Users` feed the type set");
        assert!(exts.contains("txt"), "ordinary folders feed the type set");
    }

    /// Builds (and later scrubs) a per-user association fixture under `HKCU\Software\Classes` —
    /// the exact override layer the file-type walk reads through the merged `HKCR` view, so the
    /// gates can be proven WITHOUT touching any real association or needing elevation.
    struct ProgIdFixture {
        keys: Vec<String>,
    }

    impl ProgIdFixture {
        fn new() -> Self {
            use winreg::enums::HKEY_CURRENT_USER;
            use winreg::RegKey;
            let hkcu = RegKey::predef(HKEY_CURRENT_USER);
            let mut fx = Self { keys: Vec::new() };
            for k in [
                r"Software\Classes\.dmktest9f3ca",
                r"Software\Classes\.dmktest9f3cb",
                r"Software\Classes\.dmktest9f3cc",
                r"Software\Classes\.dmktest9f3cd",
                r"Software\Classes\DeskMakeoverTest.Type9f3c",
                r"Software\Classes\DeskMakeoverTest.PerFile9f3c",
                r"Software\Classes\DeskMakeoverTest.Image9f3c",
            ] {
                let _ = hkcu.delete_subkey_all(k);
                fx.keys.push(k.to_string());
            }
            fx
        }

        /// `HKCU\<ext>` → ProgId, plus an optional `PerceivedType` on the extension key.
        fn ext(&self, ext: &str, prog_id: &str, perceived: Option<&str>) {
            use winreg::enums::HKEY_CURRENT_USER;
            use winreg::RegKey;
            let hkcu = RegKey::predef(HKEY_CURRENT_USER);
            let (key, _) = hkcu.create_subkey(format!(r"Software\Classes\.{ext}")).unwrap();
            key.set_value("", &prog_id).unwrap();
            if let Some(p) = perceived {
                key.set_value("PerceivedType", &p).unwrap();
            }
        }

        /// `HKCU\<prog>\DefaultIcon` → raw value.
        fn icon(&self, prog_id: &str, raw: &str) {
            use winreg::enums::HKEY_CURRENT_USER;
            use winreg::RegKey;
            let hkcu = RegKey::predef(HKEY_CURRENT_USER);
            let (key, _) = hkcu.create_subkey(format!(r"Software\Classes\{prog_id}\DefaultIcon")).unwrap();
            key.set_value("", &raw).unwrap();
        }
    }

    impl Drop for ProgIdFixture {
        fn drop(&mut self) {
            use winreg::enums::HKEY_CURRENT_USER;
            use winreg::RegKey;
            let hkcu = RegKey::predef(HKEY_CURRENT_USER);
            for k in &self.keys {
                let _ = hkcu.delete_subkey_all(k);
            }
        }
    }

    #[test]
    fn file_type_items_are_one_per_progid_and_never_images_or_per_file_types() {
        // The D7 amendment's contract, gate by gate, against a LIVE merged-HKCR read:
        //   a normal type → emitted once for two extensions sharing its ProgId (case-insensitive),
        //   a `%1` per-file type → never (it would flatten every executable into one glyph),
        //   `PerceivedType=image` → never (the systematic image carve-out),
        //   `.exe`/`.png` (real machine associations) → never (static list + the `%1` gate),
        //   an unassociated extension → never.
        // [WINDOWS-VERIFY] runtime (merged HKCR through HKCU `Software\Classes`).
        let fx = ProgIdFixture::new();
        fx.ext("dmktest9f3ca", "DeskMakeoverTest.Type9f3c", None);
        fx.ext("dmktest9f3cb", "deskmakovertest.type9f3c", None); // same ProgId, other case
        fx.icon("DeskMakeoverTest.Type9f3c", r"C:\Windows\System32\shell32.dll,3");
        fx.ext("dmktest9f3cc", "DeskMakeoverTest.PerFile9f3c", None);
        fx.icon("DeskMakeoverTest.PerFile9f3c", "%1");
        fx.ext("dmktest9f3cd", "DeskMakeoverTest.Image9f3c", Some("image"));
        fx.icon("DeskMakeoverTest.Image9f3c", r"C:\Windows\System32\imageres.dll,-109");

        let exts: std::collections::HashSet<String> = ["dmktest9f3ca", "dmktest9f3cb", "dmktest9f3cc", "dmktest9f3cd", "dmktest9f3cz", "exe", "png"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        // An empty set emits nothing (no files seen → no type is ever styled blind).
        assert!(explorer_file_type_items(&std::collections::HashSet::new()).is_empty());

        let items = explorer_file_type_items(&exts);
        assert_eq!(items.len(), 1, "exactly the one styleable type");
        let item = &items[0];
        assert_eq!(item.kind, ItemKind::FileType);
        assert_eq!(item.surface, ItemSurface::Explorer);
        assert_eq!(item.path, "DeskMakeoverTest.Type9f3c", "path carries the ProgId");
        assert!(!item.path.contains('\\'), "a ProgId is one key component");
        assert_eq!(item.name, ".dmktest9f3ca", "first extension seen labels the type");
        assert_eq!(item.id, ItemId::from_source_path("filetype", &item.path), "filetype id namespace");
        assert_eq!(item.state, ItemState::Ready);
        assert!(item.parent.is_empty());
        assert!(!item.requires_explicit_consent, "explorer items ride the surface toggle");
        let icon = item.icon.as_ref().expect("the type's DefaultIcon is captured at scan");
        assert_eq!(icon.location, r"C:\Windows\System32\shell32.dll");
        assert_eq!(icon.index, 3);
        assert_eq!(icon.kind, IconSourceKind::ExecutableResource, "dll icon is a resource");

        // Belt-and-suspenders: no emitted item may name the per-file or the image ProgId.
        for i in &items {
            assert!(!i.path.eq_ignore_ascii_case("DeskMakeoverTest.PerFile9f3c"));
            assert!(!i.path.eq_ignore_ascii_case("DeskMakeoverTest.Image9f3c"));
        }
    }

    #[test]
    fn a_flat_pin_root_yields_taskbar_items_with_an_empty_parent() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "Brave.lnk");
        touch(dir.path(), "File Explorer.lnk");
        let items = walk(dir.path(), ItemSurface::Taskbar, false);
        assert_eq!(items.len(), 2);
        for item in &items {
            assert_eq!(item.surface, ItemSurface::Taskbar);
            assert_eq!(item.parent, "");
            assert_eq!(item.kind, ItemKind::Shortcut);
        }
        // Display names drop the .lnk (the strip / Start panel label).
        let mut names: Vec<String> = items.iter().map(|i| i.name.clone()).collect();
        names.sort();
        assert_eq!(names, vec!["Brave".to_string(), "File Explorer".to_string()]);
    }

    #[test]
    fn a_flat_desktop_root_keeps_folders_as_items_and_never_descends() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "mine.lnk");
        touch(dir.path(), "Nested/child.lnk");
        let items = walk(dir.path(), ItemSurface::Desktop, false);
        // Desktop taxonomy unchanged: the folder is ONE Folder item; its child is not scanned.
        assert!(items.iter().any(|i| i.name == "Nested" && i.kind == ItemKind::Folder));
        assert!(!items.iter().any(|i| i.name == "child"));
        assert!(items.iter().all(|i| i.surface == ItemSurface::Desktop && i.parent.is_empty()));
    }

    #[test]
    fn a_recursive_start_menu_root_groups_items_by_relative_parent() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "Top.lnk");
        touch(dir.path(), "WinRAR/WinRAR.lnk");
        touch(dir.path(), "WinRAR/Tools/Archiver.lnk");
        let items = walk(dir.path(), ItemSurface::StartMenu, true);
        let find = |label: &str| items.iter().find(|i| i.name == label).unwrap_or_else(|| panic!("no {label}"));
        assert_eq!(find("Top").parent, "");
        assert_eq!(find("WinRAR").parent, "WinRAR");
        assert_eq!(find("Archiver").parent, r"WinRAR\Tools");
        // Start menu folders are CONTAINERS — descended into, never styled as items.
        assert!(items.iter().all(|i| i.kind == ItemKind::Shortcut));
        assert!(items.iter().all(|i| i.surface == ItemSurface::StartMenu));
    }

    #[test]
    fn desktop_ini_stays_skipped_on_every_surface_root() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "desktop.ini");
        touch(dir.path(), "app.lnk");
        for (surface, recurse) in [(ItemSurface::Desktop, false), (ItemSurface::StartMenu, true)] {
            let items = walk(dir.path(), surface, recurse);
            assert_eq!(items.len(), 1, "{surface:?} must skip its own desktop.ini");
            assert_eq!(items[0].name, "app");
        }
    }

    #[test]
    fn an_unreadable_root_fails_loud_when_mandatory_and_narrows_when_best_effort() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("never-created");
        let mut items = Vec::new();
        // Best-effort roots (taskbar / start menu / public desktop) narrow the surfaces covered.
        walk_root(&spec(ItemSurface::Taskbar, &missing, false, false), &mut items).unwrap();
        assert!(items.is_empty());
        // The user's own desktop stays LOUD (the 2026-07-19 vanish audit).
        assert!(walk_root(&spec(ItemSurface::Desktop, &missing, false, true), &mut items).is_err());
    }

    #[test]
    fn a_recursive_walk_hits_the_depth_cap_instead_of_running_away() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "0/a.lnk");
        let mut deep = String::from("0");
        for level in 1..12 {
            deep = format!("{deep}/d{level}");
            touch(dir.path(), &format!("{deep}/f.lnk"));
        }
        let items = walk(dir.path(), ItemSurface::StartMenu, true);
        for item in &items {
            let comps = if item.parent.is_empty() { 0 } else { item.parent.split('\\').count() };
            assert!(comps <= 8, "parent {:?} exceeded the depth cap", item.parent);
        }
        // The level-11 file (parent of 12 components) is beyond the cap: never scanned.
        assert!(!items.iter().any(|i| i.parent.ends_with("d11")));
        // In-range entries DID scan (7 f.lnk within cap + the root-level a.lnk).
        assert_eq!(items.iter().filter(|i| i.name == "f").count(), 7);
    }

    #[test]
    fn a_reparse_point_directory_is_never_descended() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real");
        std::fs::create_dir(&target).unwrap();
        touch(&target, "inside.lnk");
        let junction = dir.path().join("link");
        // Junctions need no elevation on Windows; skip (never fail) if the box refuses one.
        let created = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&target)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !created {
            eprintln!("skipping: mklink /J unavailable on this box");
            return;
        }
        assert!(has_reparse_point(&junction));
        let items = walk(dir.path(), ItemSurface::StartMenu, true);
        // The real directory still yields its file; the JUNCTION path never does.
        assert!(items.iter().any(|i| i.name == "inside"));
        assert!(
            !items.iter().any(|i| Path::new(&i.path).starts_with(&junction)),
            "a junction's foreign contents must never become style targets"
        );
    }

    // ---- Task 11: AppsFolder (Store/MSIX) synthesis + args classification ----

    use crate::shell::appsfolder::AppsfolderEntry;

    fn entry(display_name: &str, aumid: &str) -> AppsfolderEntry {
        AppsfolderEntry { display_name: display_name.into(), aumid: aumid.into() }
    }

    #[test]
    fn synthesize_appx_items_covers_only_entries_without_a_walked_item() {
        let dir = tempfile::tempdir().unwrap();
        // The walked-name set is LOWERCASED names across every surface: an entry whose name any
        // real item already carries is addressed by that item — never a duplicate.
        let walked: std::collections::HashSet<String> =
            ["brave", "whatsapp"].into_iter().map(String::from).collect();
        let entries = vec![
            entry("Brave", "Brave"),
            entry("whatsapp", "X!App"),
            entry("Trello", "45273LiamForsyth.PawsforTrello_7pb5ddty8z1pa!trello"),
            entry("Reloj", "Microsoft.WindowsAlarms_8wekyb3d8bbwe!App"),
        ];
        let items = synthesize_appx_items(&walked, &entries, dir.path());
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["Trello", "Reloj"], "walked names (any case) are never synthesized");

        let trello = &items[0];
        assert_eq!(trello.kind, ItemKind::AppxShortcut);
        assert_eq!(trello.surface, ItemSurface::StartMenu);
        assert!(trello.parent.is_empty(), "wrappers live at the Programs root");
        assert_eq!(trello.icon, None, "the icon is extracted live from the package glyph");
        let expected_path =
            dir.path().join("Trello.lnk").to_string_lossy().into_owned();
        assert_eq!(trello.path, expected_path, "path is the PROSPECTIVE wrapper location");
        assert_eq!(
            trello.id,
            ItemId::from_source_path("filesystem", &expected_path),
            "the id anchors on the path apply will write to"
        );
        assert!(trello.can_style());
    }

    #[test]
    fn synthesize_appx_items_dedupes_display_names_and_rejects_unsafe_stems() {
        let dir = tempfile::tempdir().unwrap();
        let walked = std::collections::HashSet::new();
        let entries = vec![
            entry("Dup", "A!App"),
            entry("dup", "B!App"), // duplicate display name → first wins
            entry("Bad:Name", "C!App"), // illegal filename char → skipped
            entry("Trailing.", "D!App"), // trailing dot normalizes → skipped
            entry("With Space", "E!App"), // legal → kept
        ];
        let items = synthesize_appx_items(&walked, &entries, dir.path());
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["Dup", "With Space"]);
    }

    #[test]
    fn a_garbage_fixture_lnk_stays_an_ordinary_shortcut_after_the_args_read() {
        // The args-driven AppxShortcut reclassification must be best-effort: a `.lnk` that cannot
        // be COM-loaded (every byte fixture here) is an ordinary Shortcut, never a scan error.
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "Broken.lnk");
        let items = walk(dir.path(), ItemSurface::Taskbar, false);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, ItemKind::Shortcut);
    }

    /// On-box (COM): a REAL appx wrapper (the box-verified launcher recipe — args
    /// `--activate-aumid <AUMID>`) walks as an AppxShortcut with a clean display name — the
    /// classification the whole Task 11 pipeline keys on, and the guard against a present
    /// wrapper re-reading as an ordinary shortcut (which would compound re-styles).
    #[test]
    fn a_real_appsfolder_wrapper_lnk_walks_as_an_appx_shortcut() {
        // The icon lives OUTSIDE the walked root — it is an asset, not a scanned item.
        let assets = tempfile::tempdir().unwrap();
        let icon = assets.path().join("i.ico");
        std::fs::write(&icon, b"icon").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let lnk = dir.path().join("WhatsApp.lnk").to_string_lossy().into_owned();
        let icon = icon.to_string_lossy().into_owned();
        let exec = crate::com::StaExecutor::spawn().unwrap();
        exec.run(move || shell_link::create_appx_shortcut(&lnk, "Fake.App_abc!App", &icon))
            .unwrap()
            .unwrap();
        let items = walk(dir.path(), ItemSurface::StartMenu, false);
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].kind,
            ItemKind::AppxShortcut,
            "our wrapper arguments (--activate-aumid) must reclassify the .lnk"
        );
        assert_eq!(items[0].name, "WhatsApp", "the label drops .lnk like any shortcut");
    }
}
