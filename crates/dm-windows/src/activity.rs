//! Desktop activity detection ([WINDOWS-VERIFY]) — the `ActivityMonitor` port (spec 07 §11).
//!
//! Data safety is already the driver's CAS job; this is a UX-layer signal only: don't let an icon
//! visibly change under the user's cursor mid-drag. The reconciler POLLS `is_desktop_busy()`
//! synchronously between every icon in a batch, so this impl is a synchronous poll — no hook
//! thread, no message pump.
//!
//! v1 uses **judge 2** (spec 07 §11, the coarse fallback), which is exactly what a synchronous
//! poll can answer: the foreground window is a desktop/Explorer class AND the user gave input
//! recently. **Judge 1** (the precise `SetWinEventHook` on DRAGDROP/CAPTURE scoped to the desktop
//! `SysListView32`) needs a dedicated hook thread with a message pump maintaining an atomic
//! "busy-until" flag; it is a documented `[WINDOWS-VERIFY]` precision enhancement layered on top,
//! not required for a correct v1 (a false "idle" only risks a cosmetic mid-drag repaint, never
//! data — the CAS guarantees that). The handle-resolution chain for judge 1 is the same
//! `Progman → SHELLDLL_DefView/WorkerW → SysListView32` walk documented in `shell/layout.rs`.

/// The recency window (ms) within which recent input, under a desktop-class foreground window,
/// reads as "busy" (spec 07 §11 judge 2 uses < 2s).
#[cfg(windows)]
const RECENT_INPUT_MS: u32 = 2_000;

/// The `ActivityMonitor` implementation for a real Windows desktop.
#[cfg(windows)]
pub struct WindowsActivityMonitor;

#[cfg(windows)]
impl WindowsActivityMonitor {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(windows)]
impl Default for WindowsActivityMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
impl dm_domain::ActivityMonitor for WindowsActivityMonitor {
    fn is_desktop_busy(&self) -> dm_domain::PortResult<bool> {
        Ok(win::is_busy())
    }
}

/// Whether a foreground window class name is a desktop/Explorer shell class (judge 2). Pure — the
/// class list is exercised on the Mac host; the live `GetForegroundWindow`/`GetClassNameW` reads
/// are `[WINDOWS-VERIFY]`.
#[cfg(any(windows, test))]
fn is_desktop_class(class: &str) -> bool {
    // Progman/WorkerW host the desktop; SHELLDLL_DefView/SysListView32 are the icon view;
    // CabinetWClass is an Explorer window (the user could be dragging from a folder onto the
    // desktop). Case-insensitive to be defensive about class-name casing.
    const DESKTOP_CLASSES: &[&str] = &[
        "Progman",
        "WorkerW",
        "SHELLDLL_DefView",
        "SysListView32",
        "CabinetWClass",
    ];
    DESKTOP_CLASSES.iter().any(|c| c.eq_ignore_ascii_case(class))
}

#[cfg(windows)]
mod win {
    use super::{is_desktop_class, RECENT_INPUT_MS};
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    use windows::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow};

    /// Judge 2: the foreground window is a desktop/Explorer class AND input landed within the
    /// recency window. Any read failure reads as BUSY (err on the quiet side — a missed suppress
    /// only risks a cosmetic repaint, never data).
    pub(super) fn is_busy() -> bool {
        // SAFETY: plain user32 reads, no resources to release.
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return true; // no foreground window resolved → conservative
            }
            let mut buf = [0u16; 256];
            let len = GetClassNameW(hwnd, &mut buf);
            if len <= 0 {
                return true;
            }
            let class = String::from_utf16_lossy(&buf[..len as usize]);
            if !is_desktop_class(&class) {
                return false; // the user is in another app — the desktop is not busy
            }
            let mut info = LASTINPUTINFO {
                cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                dwTime: 0,
            };
            if GetLastInputInfo(&mut info).as_bool() {
                let idle_ms = GetTickCount().wrapping_sub(info.dwTime);
                idle_ms < RECENT_INPUT_MS
            } else {
                true // cannot read input recency → conservative
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_and_explorer_classes_are_recognized_case_insensitively() {
        for c in ["Progman", "WorkerW", "SHELLDLL_DefView", "SysListView32", "CabinetWClass"] {
            assert!(is_desktop_class(c), "{c} is a desktop/Explorer class");
        }
        // Case-insensitive.
        assert!(is_desktop_class("progman"));
        assert!(is_desktop_class("syslistview32"));
        // A normal application window is NOT the desktop.
        assert!(!is_desktop_class("Chrome_WidgetWin_1"));
        assert!(!is_desktop_class("Notepad"));
        assert!(!is_desktop_class(""));
    }
}
