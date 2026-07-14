//! Multi-monitor wallpaper topology via `IDesktopWallpaper` (M6-WIRE A4): every
//! present monitor's device path, virtual-desktop bounds, current source path and
//! slideshow flag, plus the global position. Pure read — no mutation. All COM on the
//! STA thread. [WINDOWS-VERIFY] runtime (blind-written on Mac, msvc-cross-checked).
//!
//! Blind-write simplifications recorded for the Windows handoff:
//! * `name` is "Display N" — DisplayConfig friendly names (e.g. the panel model) are
//!   a Windows-batch upgrade; the frontend treats the name as opaque display text.
//! * `slideshow_active` is the GLOBAL `DSS_SLIDESHOW` flag mirrored onto every
//!   monitor — `IDesktopWallpaper` has no per-monitor slideshow granularity.
//! * `has_readable_source` = "GetWallpaper returned a non-empty path". Distinguishing
//!   a solid-colour desktop (readable, no image) from a third-party dynamic wallpaper
//!   (unreadable) needs a real desktop — refined on the owner's box.

use std::sync::Arc;

use dm_domain::{
    MonitorInfo, MonitorRect, MonitorTopology, PortError, PortResult, WallpaperPosition,
    WallpaperTopology,
};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL};
use windows::Win32::UI::Shell::{
    DesktopWallpaper, IDesktopWallpaper, DSS_SLIDESHOW, DWPOS_CENTER, DWPOS_FIT, DWPOS_SPAN,
    DWPOS_STRETCH, DWPOS_TILE,
};

use crate::com::StaExecutor;

/// The `IDesktopWallpaper` topology reader.
pub struct WindowsMonitorTopology {
    exec: Arc<StaExecutor>,
}

impl WindowsMonitorTopology {
    pub fn new(exec: Arc<StaExecutor>) -> Self {
        Self { exec }
    }
}

impl MonitorTopology for WindowsMonitorTopology {
    fn enumerate(&self) -> PortResult<WallpaperTopology> {
        self.exec.run(enumerate_blocking)?
    }
}

fn enumerate_blocking() -> PortResult<WallpaperTopology> {
    // SAFETY: coclass activation + all calls confined to this STA thread; every PWSTR
    // is freed with CoTaskMemFree (same discipline as wallpaper.rs).
    unsafe {
        // CLSCTX_ALL, not CLSCTX_INPROC_SERVER: CLSID_DesktopWallpaper has no
        // InprocServer32 — it is registered as a local-server surrogate (AppId
        // {8B30085D-…}), so an inproc-only activation returns REGDB_E_CLASSNOTREG
        // (0x80040154). Verified on a real Windows box; matches Microsoft's own
        // IDesktopWallpaper sample and layout.rs's IShellWindows activation.
        let dw: IDesktopWallpaper =
            CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL).map_err(com)?;

        let position = map_position(dw.GetPosition().map_err(com)?.0);
        // A running slideshow reports DSS_SLIDESHOW; any other/failed status = not-a-slideshow.
        let slideshow_active = matches!(dw.GetStatus(), Ok(s) if s.0 & DSS_SLIDESHOW.0 != 0);

        let count = dw.GetMonitorDevicePathCount().map_err(com)?;
        let mut monitors = Vec::new();
        for i in 0..count {
            let id_pwstr = match dw.GetMonitorDevicePathAt(i) {
                Ok(p) if !p.is_null() => p,
                // Detached-but-remembered monitor: no screen to report. Omit.
                _ => continue,
            };
            let id = id_pwstr.to_string();
            CoTaskMemFree(Some(id_pwstr.0 as *const core::ffi::c_void));
            let id = id.map_err(|e| PortError::Com(e.to_string()))?;

            let hid = HSTRING::from(id.as_str());
            // GetMonitorRECT fails for a detached monitor the engine still remembers —
            // skip it (parity with the capture path's omit rule). [WINDOWS-VERIFY]
            let Ok(rect) = dw.GetMonitorRECT(PCWSTR(hid.as_ptr())) else { continue };
            let bounds = MonitorRect {
                x: rect.left,
                y: rect.top,
                w: rect.right - rect.left,
                h: rect.bottom - rect.top,
            };

            let source_path = crate::wallpaper::read_wallpaper(&dw, &id);
            let has_readable_source = source_path.is_some();
            monitors.push(MonitorInfo {
                monitor_id: id,
                name: format!("Display {}", monitors.len() + 1),
                bounds,
                source_path,
                slideshow_active,
                has_readable_source,
            });
        }
        Ok(WallpaperTopology { monitors, position })
    }
}

fn map_position(raw: i32) -> WallpaperPosition {
    match raw {
        v if v == DWPOS_CENTER.0 => WallpaperPosition::Center,
        v if v == DWPOS_TILE.0 => WallpaperPosition::Tile,
        v if v == DWPOS_STRETCH.0 => WallpaperPosition::Stretch,
        v if v == DWPOS_FIT.0 => WallpaperPosition::Fit,
        v if v == DWPOS_SPAN.0 => WallpaperPosition::Span,
        // DWPOS_FILL and any future/unknown value: Fill is Windows' own default.
        _ => WallpaperPosition::Fill,
    }
}

fn com(e: windows::core::Error) -> PortError {
    PortError::Com(e.to_string())
}
