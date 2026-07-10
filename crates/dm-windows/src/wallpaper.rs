//! Wallpaper get-source / apply / restore via `IDesktopWallpaper`, ported from
//! `DeskMakeover.Shell/DesktopWallpaperInterop.cs`. Every call runs on the STA thread. Multi-
//! monitor state is captured per device path so restore is byte-for-byte. [WINDOWS-VERIFY] runtime.

use std::sync::Arc;

use dm_domain::{PortError, PortResult};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Shell::{DesktopWallpaper, IDesktopWallpaper};

use crate::com::StaExecutor;

/// One monitor's wallpaper: its device path and the image currently shown (or `None` for a
/// solid-colour / transient-slideshow monitor).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorWallpaper {
    pub monitor_id: String,
    pub image: Option<String>,
}

/// The `IDesktopWallpaper` adapter.
pub struct WindowsWallpaper {
    exec: Arc<StaExecutor>,
}

impl WindowsWallpaper {
    pub fn new(exec: Arc<StaExecutor>) -> Self {
        Self { exec }
    }

    /// Captures every monitor's current wallpaper (the restore snapshot).
    pub fn capture(&self) -> PortResult<Vec<MonitorWallpaper>> {
        self.exec.run(capture_blocking)?
    }

    /// Sets one monitor's wallpaper image.
    pub fn set(&self, monitor_id: String, image_path: String) -> PortResult<()> {
        self.exec.run(move || set_blocking(&monitor_id, &image_path))?
    }

    /// Restores every captured monitor's wallpaper (skipping monitors that had no image).
    pub fn restore(&self, snapshot: Vec<MonitorWallpaper>) -> PortResult<()> {
        self.exec.run(move || restore_blocking(&snapshot))?
    }
}

fn create() -> PortResult<IDesktopWallpaper> {
    // SAFETY: coclass activation on the STA thread.
    unsafe { CoCreateInstance(&DesktopWallpaper, None, CLSCTX_INPROC_SERVER) }.map_err(com)
}

fn capture_blocking() -> PortResult<Vec<MonitorWallpaper>> {
    let dw = create()?;
    let mut monitors = Vec::new();
    // SAFETY: all COM calls confined to this STA thread; each PWSTR is freed with CoTaskMemFree.
    unsafe {
        let count = dw.GetMonitorDevicePathCount().map_err(com)?;
        for i in 0..count {
            let id_pwstr = match dw.GetMonitorDevicePathAt(i) {
                Ok(p) if !p.is_null() => p,
                _ => continue, // detached monitor the engine still remembers
            };
            // Free the buffer BEFORE propagating a decode error (matches read_wallpaper below):
            // the `?` early-return would otherwise leak the PWSTR.
            let id = id_pwstr.to_string();
            CoTaskMemFree(Some(id_pwstr.0 as *const core::ffi::c_void));
            let id = id.map_err(|e| PortError::Com(e.to_string()))?;
            let image = read_wallpaper(&dw, &id);
            monitors.push(MonitorWallpaper { monitor_id: id, image });
        }
    }
    Ok(monitors)
}

/// SAFETY: caller guarantees an STA thread; the returned PWSTR is freed here.
unsafe fn read_wallpaper(dw: &IDesktopWallpaper, monitor_id: &str) -> Option<String> {
    let id = HSTRING::from(monitor_id);
    match dw.GetWallpaper(PCWSTR(id.as_ptr())) {
        Ok(pwstr) if !pwstr.is_null() => {
            let text = pwstr.to_string().ok();
            CoTaskMemFree(Some(pwstr.0 as *const core::ffi::c_void));
            text.filter(|s| !s.is_empty())
        }
        _ => None, // solid-colour desktop or transient slideshow state
    }
}

fn set_blocking(monitor_id: &str, image_path: &str) -> PortResult<()> {
    let dw = create()?;
    let id = HSTRING::from(monitor_id);
    let image = HSTRING::from(image_path);
    // SAFETY: STA thread; the HSTRING buffers outlive the call.
    unsafe { dw.SetWallpaper(PCWSTR(id.as_ptr()), PCWSTR(image.as_ptr())) }.map_err(com)
}

fn restore_blocking(snapshot: &[MonitorWallpaper]) -> PortResult<()> {
    let dw = create()?;
    for monitor in snapshot {
        if let Some(image) = &monitor.image {
            let id = HSTRING::from(monitor.monitor_id.as_str());
            let img = HSTRING::from(image.as_str());
            // SAFETY: STA thread; HSTRING buffers outlive the call.
            unsafe { dw.SetWallpaper(PCWSTR(id.as_ptr()), PCWSTR(img.as_ptr())) }.map_err(com)?;
        }
    }
    Ok(())
}

fn com(e: windows::core::Error) -> PortError {
    PortError::Com(e.to_string())
}
