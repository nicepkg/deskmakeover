//! Desktop icon layout + geometry — `DesktopGeometryReader` for the real desktop.
//!
//! Technique A from ADR-0019 (oracle `DeskMakeover.Shell/FolderViewInterop.cs`), BLIND-WRITTEN
//! against the windows-rs projections (which lay the COM vtables out for us — the hand-ordered
//! placeholder-slot hazard the C# interop had does not exist here): walk the live shell chain
//! `IShellWindows::FindWindowSW(SWC_DESKTOP)` → `IServiceProvider::QueryService(SID_STopLevelBrowser)`
//! → `IShellBrowser::QueryActiveShellView` → QI `IFolderView2`, then `ItemCount` +
//! `Item(i)`/`GetItemPosition` + `IShellFolder::GetDisplayNameOf` for the name each slot matches
//! on. Every step can legitimately fail (session 0, MTA caller, denied QI) — any failure degrades
//! to an Err the host replaces with its synthetic layout, exactly the oracle's silent-false.
//!
//! Technique B (cross-process `SysListView32` + `ReadProcessMemory`, oracle
//! `DesktopLayoutReader.cs`) stays UNWRITTEN as the documented fallback if A proves unreliable on
//! the box — positions are a preview-mirror nicety, never on the apply/restore critical path.
//!
//! [WINDOWS-VERIFY] runtime: the whole chain, plus geometry (`SPI_GETWORKAREA` vs a left/right
//! taskbar — the reserved height collapses to 0 there, which the grid tolerates).

use std::sync::Arc;

use dm_domain::{DesktopGeometry, DesktopGeometryReader, DesktopIconSlot, PortResult};

use crate::com::StaExecutor;

/// The real desktop's geometry + live icon positions, marshalled onto the STA thread.
pub struct WindowsDesktopGeometry {
    exec: Arc<StaExecutor>,
}

impl WindowsDesktopGeometry {
    pub fn new(exec: Arc<StaExecutor>) -> Self {
        Self { exec }
    }
}

impl DesktopGeometryReader for WindowsDesktopGeometry {
    fn geometry(&self) -> PortResult<DesktopGeometry> {
        self.exec.run(win::geometry_blocking)?
    }

    fn positions(&self) -> PortResult<Vec<DesktopIconSlot>> {
        self.exec.run(win::positions_blocking)?
    }
}

mod win {
    use dm_domain::{DesktopGeometry, DesktopIconSlot, PortError, PortResult};
    use windows::core::Interface;
    use windows::Win32::Foundation::RECT;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoTaskMemFree, IServiceProvider, CLSCTX_ALL,
    };
    use windows::Win32::System::Variant::VARIANT;
    use windows::Win32::UI::Shell::Common::{ITEMIDLIST, STRRET};
    use windows::Win32::UI::Shell::{
        IFolderView2, IShellBrowser, IShellFolder, IShellWindows, ShellWindows, StrRetToBufW,
        SHGDN_NORMAL, SID_STopLevelBrowser, SVGIO_ALLVIEW, SWC_DESKTOP, SWFO_NEEDDISPATCH,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SystemParametersInfoW, SM_CXSCREEN, SM_CYSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN, SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };

    /// Full screen dims + the taskbar's reserved height (screen − work area). A side-docked
    /// taskbar reserves width, not height — the height then reads 0, which the grid tolerates.
    pub(super) fn geometry_blocking() -> PortResult<DesktopGeometry> {
        // SAFETY: plain user32 metric reads.
        unsafe {
            let width = GetSystemMetrics(SM_CXSCREEN);
            let height = GetSystemMetrics(SM_CYSCREEN);
            if width <= 0 || height <= 0 {
                return Err(PortError::Io("GetSystemMetrics returned no screen".into()));
            }
            let mut work = RECT::default();
            SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some(&mut work as *mut RECT as *mut _),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
            .map_err(|e| PortError::Io(format!("SPI_GETWORKAREA: {e}")))?;
            let taskbar = (height - (work.bottom - work.top)).max(0);
            Ok(DesktopGeometry {
                screen_width: width as u32,
                screen_height: height as u32,
                taskbar_height: taskbar as u32,
            })
        }
    }

    /// Technique A: the live `IFolderView2` walk. Any COM failure surfaces as one Err — the host
    /// degrades to its synthetic layout, matching the oracle's silent-false contract.
    pub(super) fn positions_blocking() -> PortResult<Vec<DesktopIconSlot>> {
        // SAFETY: COM on the STA thread; PIDLs are CoTaskMem-owned and freed per iteration.
        unsafe {
            let shell: IShellWindows = CoCreateInstance(&ShellWindows, None, CLSCTX_ALL)
                .map_err(|e| PortError::Io(format!("ShellWindows: {e}")))?;
            let mut hwnd = 0i32;
            let dispatch = shell
                .FindWindowSW(
                    &VARIANT::default(),
                    &VARIANT::default(),
                    SWC_DESKTOP,
                    &mut hwnd,
                    SWFO_NEEDDISPATCH,
                )
                .map_err(|e| PortError::Io(format!("FindWindowSW(desktop): {e}")))?;
            let provider: IServiceProvider = dispatch
                .cast()
                .map_err(|e| PortError::Io(format!("desktop IServiceProvider: {e}")))?;
            let browser: IShellBrowser = provider
                .QueryService(&SID_STopLevelBrowser)
                .map_err(|e| PortError::Io(format!("SID_STopLevelBrowser: {e}")))?;
            let view = browser
                .QueryActiveShellView()
                .map_err(|e| PortError::Io(format!("QueryActiveShellView: {e}")))?;
            let folder_view: IFolderView2 = view
                .cast()
                .map_err(|e| PortError::Io(format!("IFolderView2 QI: {e}")))?;
            let folder: IShellFolder = folder_view
                .GetFolder()
                .map_err(|e| PortError::Io(format!("IFolderView2::GetFolder: {e}")))?;

            let count = folder_view
                .ItemCount(SVGIO_ALLVIEW)
                .map_err(|e| PortError::Io(format!("ItemCount: {e}")))?;
            // GetItemPosition returns coordinates in the desktop ListView's client space, whose
            // origin (0,0) is the VIRTUAL-desktop top-left, not the primary monitor's. When another
            // monitor sits above/left of the primary the virtual origin is negative, so a primary
            // icon at real (13,2) reads as (13, 2 - SM_YVIRTUALSCREEN). Shift back to primary-relative
            // coords (what the mirror + geometry use) by adding the virtual origin; both are 0 on a
            // single-monitor desktop, so this is a no-op there. [WINDOWS-VERIFY closed: real 2nd monitor]
            let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let mut slots = Vec::with_capacity(count as usize);
            for i in 0..count {
                let pidl: *mut ITEMIDLIST = match folder_view.Item(i) {
                    Ok(p) if !p.is_null() => p,
                    _ => continue, // an item that vanished mid-walk is skipped, not fatal
                };
                let slot = (|| -> Option<DesktopIconSlot> {
                    let pos = folder_view.GetItemPosition(pidl).ok()?;
                    let mut ret = STRRET::default();
                    folder.GetDisplayNameOf(pidl, SHGDN_NORMAL, &mut ret).ok()?;
                    let mut buf = [0u16; 520];
                    StrRetToBufW(&mut ret, Some(pidl), &mut buf).ok()?;
                    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                    let name = String::from_utf16_lossy(&buf[..len]);
                    if name.is_empty() {
                        return None;
                    }
                    Some(DesktopIconSlot { name, x: pos.x + vx, y: pos.y + vy })
                })();
                CoTaskMemFree(Some(pidl as *const _));
                if let Some(s) = slot {
                    slots.push(s);
                }
            }
            Ok(slots)
        }
    }
}
