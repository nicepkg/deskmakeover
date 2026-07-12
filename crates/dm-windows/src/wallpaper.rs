//! Wallpaper capture / apply / restore via `IDesktopWallpaper`, ported from
//! `DeskMakeover.Shell/DesktopWallpaperInterop.cs`. Every call runs on the STA thread.
//!
//! Restore is byte-level (P2-1): the snapshot records the global background COLOR and POSITION in
//! addition to each monitor's image, so a solid-colour or repositioned desktop returns to exactly
//! its prior state instead of leaving a residual DeskMakeover image. A present monitor showing the
//! solid background is captured as `image: None` (distinct from a *detached* monitor, which is
//! omitted from the snapshot entirely) and restored by re-applying the background colour and
//! clearing our image.
//!
//! Slideshow is a documented limitation: a running slideshow is captured as `slideshow_active`
//! plus each monitor's current frame and restored as static images; re-arming the rotation
//! (`IDesktopWallpaper::SetSlideshow` with an `IShellItemArray`) is out of scope for this pass.
//! [WINDOWS-VERIFY] runtime.

use std::sync::Arc;

use dm_domain::{
    MonitorWallpaper, PortError, PortResult, WallpaperApplier, WallpaperSnapshot,
};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::COLORREF;
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Shell::{
    DesktopWallpaper, IDesktopWallpaper, DESKTOP_WALLPAPER_POSITION, DSS_SLIDESHOW,
};

use crate::com::StaExecutor;

/// The `IDesktopWallpaper` adapter. Snapshot types live in `dm-domain::wallpaper`
/// (M6-WIRE A2) so the operations layer's snapshot-once policy and the Mac fakes
/// speak the exact same shapes; this adapter implements the [`WallpaperApplier`]
/// port over them.
pub struct WindowsWallpaper {
    exec: Arc<StaExecutor>,
}

impl WindowsWallpaper {
    pub fn new(exec: Arc<StaExecutor>) -> Self {
        Self { exec }
    }
}

impl WallpaperApplier for WindowsWallpaper {
    /// Captures the full restore snapshot (background colour, position, per-monitor images).
    fn capture(&self) -> PortResult<WallpaperSnapshot> {
        self.exec.run(capture_blocking)?
    }

    /// Sets one monitor's wallpaper image.
    fn set(&self, monitor_id: &str, image_path: &str) -> PortResult<()> {
        let (monitor_id, image_path) = (monitor_id.to_owned(), image_path.to_owned());
        self.exec.run(move || set_blocking(&monitor_id, &image_path))?
    }

    /// Restores the captured snapshot: global colour + position, then each monitor's image (or a
    /// cleared image so the background colour shows for a solid-colour monitor).
    fn restore(&self, snapshot: &WallpaperSnapshot) -> PortResult<()> {
        let snapshot = snapshot.clone();
        self.exec.run(move || restore_blocking(&snapshot))?
    }
}

fn create() -> PortResult<IDesktopWallpaper> {
    // SAFETY: coclass activation on the STA thread.
    unsafe { CoCreateInstance(&DesktopWallpaper, None, CLSCTX_INPROC_SERVER) }.map_err(com)
}

fn capture_blocking() -> PortResult<WallpaperSnapshot> {
    let dw = create()?;
    // SAFETY: all COM calls confined to this STA thread; each PWSTR is freed with CoTaskMemFree.
    unsafe {
        let background_color = dw.GetBackgroundColor().map_err(com)?.0;
        let position = dw.GetPosition().map_err(com)?.0;
        // A running slideshow reports DSS_SLIDESHOW; treat any other/failed status as not-a-slideshow.
        let slideshow_active = matches!(dw.GetStatus(), Ok(s) if s.0 & DSS_SLIDESHOW.0 != 0);

        let count = dw.GetMonitorDevicePathCount().map_err(com)?;
        let mut monitors = Vec::new();
        for i in 0..count {
            let id_pwstr = match dw.GetMonitorDevicePathAt(i) {
                Ok(p) if !p.is_null() => p,
                // A detached monitor the engine still remembers: omit it (no screen to restore),
                // distinct from a present monitor showing a solid colour (captured as image: None).
                _ => continue,
            };
            // Free the buffer before propagating a decode error (the ? would otherwise leak it).
            let id = id_pwstr.to_string();
            CoTaskMemFree(Some(id_pwstr.0 as *const core::ffi::c_void));
            let id = id.map_err(|e| PortError::Com(e.to_string()))?;
            // CAPTURE uses the STRICT read: a COM read error fails the whole capture so the
            // pre-first-apply snapshot is never persisted with a monitor's original silently
            // dropped to `None` (which restore would then clear to the background colour, losing
            // the real wallpaper behind a snapshot the user believes protects them). (F3)
            let image = read_wallpaper_strict(&dw, &id)?;
            monitors.push(MonitorWallpaper { monitor_id: id, image });
        }
        Ok(WallpaperSnapshot { background_color, position, slideshow_active, monitors })
    }
}

/// SAFETY: caller guarantees an STA thread; the returned PWSTR is freed here. Returns `None` for a
/// solid-colour desktop or transient slideshow state (GetWallpaper reports an empty path).
/// pub(crate): the topology reader (`topology.rs`) reuses this exact read discipline.
pub(crate) unsafe fn read_wallpaper(dw: &IDesktopWallpaper, monitor_id: &str) -> Option<String> {
    let id = HSTRING::from(monitor_id);
    match dw.GetWallpaper(PCWSTR(id.as_ptr())) {
        Ok(pwstr) if !pwstr.is_null() => {
            let text = pwstr.to_string().ok();
            CoTaskMemFree(Some(pwstr.0 as *const core::ffi::c_void));
            text.filter(|s| !s.is_empty())
        }
        _ => None,
    }
}

/// STRICT read for the pre-first-apply CAPTURE path: a `GetWallpaper` COM **error** propagates
/// (so `capture` fails and snapshot-once refuses to mutate the desktop behind an untrustworthy
/// backup), while a genuinely empty path — a solid-colour desktop — is a legitimate `Ok(None)`.
/// Contrast [`read_wallpaper`], which collapses both to `None`; that leniency is correct for the
/// topology/getScreens path (an unreadable source only shows the import CTA, no data-loss risk)
/// but unsafe for the snapshot. SAFETY: caller guarantees an STA thread; the PWSTR is freed here.
/// [WINDOWS-VERIFY] the Err-vs-empty distinction on a real desktop.
pub(crate) unsafe fn read_wallpaper_strict(
    dw: &IDesktopWallpaper,
    monitor_id: &str,
) -> PortResult<Option<String>> {
    let id = HSTRING::from(monitor_id);
    let pwstr = dw.GetWallpaper(PCWSTR(id.as_ptr())).map_err(com)?;
    if pwstr.is_null() {
        return Ok(None);
    }
    let text = pwstr.to_string();
    CoTaskMemFree(Some(pwstr.0 as *const core::ffi::c_void));
    let text = text.map_err(|e| PortError::Com(e.to_string()))?;
    Ok(if text.is_empty() { None } else { Some(text) })
}

fn set_blocking(monitor_id: &str, image_path: &str) -> PortResult<()> {
    let dw = create()?;
    let id = HSTRING::from(monitor_id);
    let image = HSTRING::from(image_path);
    // SAFETY: STA thread; the HSTRING buffers outlive the call.
    unsafe { dw.SetWallpaper(PCWSTR(id.as_ptr()), PCWSTR(image.as_ptr())) }.map_err(com)
}

fn restore_blocking(snapshot: &WallpaperSnapshot) -> PortResult<()> {
    let dw = create()?;
    // SAFETY: STA thread; HSTRING buffers outlive each call.
    unsafe {
        // Colour + position first, so a cleared (solid-colour) monitor shows the right colour.
        dw.SetBackgroundColor(COLORREF(snapshot.background_color)).map_err(com)?;
        dw.SetPosition(DESKTOP_WALLPAPER_POSITION(snapshot.position)).map_err(com)?;
        for monitor in &snapshot.monitors {
            let id = HSTRING::from(monitor.monitor_id.as_str());
            match &monitor.image {
                Some(image) => {
                    let img = HSTRING::from(image.as_str());
                    dw.SetWallpaper(PCWSTR(id.as_ptr()), PCWSTR(img.as_ptr())).map_err(com)?;
                }
                None => {
                    // Solid colour / slideshow frame at capture: clear our image so the restored
                    // background colour shows through. Best-effort — exact clear semantics for an
                    // empty path are [WINDOWS-VERIFY]; a rejected clear must not fail the restore.
                    let empty = HSTRING::default();
                    let _ = dw.SetWallpaper(PCWSTR(id.as_ptr()), PCWSTR(empty.as_ptr()));
                }
            }
        }
    }
    Ok(())
}

fn com(e: windows::core::Error) -> PortError {
    PortError::Com(e.to_string())
}
