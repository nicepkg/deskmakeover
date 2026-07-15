//! The guided-route launch adapter (spec 08 §5: "route to the official Windows settings
//! entry"): opens an `ms-settings:` URI with the shell's default handler. URIs come ONLY from
//! the static catalog (`ManualRoute::SettingsPage` carries `&'static str` literals) — never from
//! the frontend — so there is no injection surface. `[WINDOWS-VERIFY]` the live launch.

use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// Launch a settings URI (e.g. `ms-settings:taskbar`) via `ShellExecuteW("open", …)`.
/// ShellExecuteW reports success as an HINSTANCE value > 32 (the legacy contract).
pub fn open_settings_page(uri: &str) -> Result<(), String> {
    let target = HSTRING::from(uri);
    // SAFETY: plain shell32 call with owned wide strings that outlive it.
    let code = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(target.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if code.0 as usize > 32 {
        Ok(())
    } else {
        Err(format!("ShellExecuteW({uri}) failed with code {}", code.0 as usize))
    }
}
