//! Desktop icon layout (on-screen positions) — the two spike-gate techniques from ADR-0019.
//!
//! [WINDOWS-VERIFY] — DOCUMENTED STUB. Reading live icon positions is one of the M1 go/no-go
//! spikes and is intricate, untestable COM/inter-process code; rather than ship a
//! blind-written version that compiles but is likely subtly wrong, this records the exact
//! techniques (both fully implemented in the frozen C# oracle) for the owner's Windows session,
//! and returns an empty layout until then. Positions are a preview-mirror nicety, not on the
//! M3/M4 apply/restore critical path.
//!
//! Technique A — `IFolderView2::GetItemPosition` (preferred): walk the live shell chain
//! `IShellWindows::FindWindowSW(SWC_DESKTOP)` → `IServiceProvider::QueryService(SID_STopLevelBrowser,
//! IShellBrowser)` → `IShellBrowser::QueryActiveShellView` → QI `IFolderView2`, then
//! `GetItemCount` + `GetItemPosition(i)`. Oracle: `DeskMakeover.Shell/FolderViewInterop.cs`.
//!
//! Technique B — cross-process `SysListView32` read (fallback): `FindWindow("Progman")` →
//! `SHELLDLL_DefView` (or a `WorkerW` for slideshow desktops) → `SysListView32`; then
//! `OpenProcess` + `VirtualAllocEx` + `LVM_GETITEMPOSITION`/`LVM_GETITEMTEXT` via
//! `ReadProcessMemory`. Oracle: `DeskMakeover.Shell/DesktopLayoutReader.cs`.

/// One desktop icon's on-screen position (top-left in desktop pixels) and z-order. Mirrors the
/// oracle `DesktopIconSlot`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopIconSlot {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub index: i32,
}

/// Reads every desktop icon's live position/order.
///
/// [WINDOWS-VERIFY] STUB: returns an empty layout until technique A/B above is wired on Windows.
/// The oracle degrades to an empty list on any failure, so an empty result is a valid state.
pub fn read_positions() -> Vec<DesktopIconSlot> {
    Vec::new()
}
