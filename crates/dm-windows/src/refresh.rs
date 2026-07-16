//! Explorer icon-cache refresh, ported from `DeskMakeover.Shell/ExplorerRefresh.cs`. A light,
//! non-disruptive `SHChangeNotify` — never an Explorer kill (spec 01 Safety Rules).

use dm_domain::{ExplorerRefresher, PortResult};
use windows::core::HSTRING;
use windows::Win32::UI::Shell::{
    SHChangeNotify, SHCNE_UPDATEDIR, SHCNE_UPDATEITEM, SHCNE_ASSOCCHANGED, SHCNF_FLUSH,
    SHCNF_PATHW,
};
use windows::Win32::UI::Shell::SHCNF_IDLIST;

/// The `SHChangeNotify`-based refresher.
pub struct WindowsExplorerRefresher;

impl ExplorerRefresher for WindowsExplorerRefresher {
    /// [WINDOWS-VERIFY] runtime.
    fn notify_icons_changed(&self) -> PortResult<()> {
        // SAFETY: `SHChangeNotify(SHCNE_ASSOCCHANGED, …, null, null)` is the documented global
        // icon-association refresh; it takes no ownership and cannot fail meaningfully.
        unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
        Ok(())
    }

    /// Directory-scoped update for a FOLDER (Explorer re-reads its `desktop.ini` only on
    /// `SHCNE_UPDATEDIR` — the global refresh leaves the custom icon cached, owner 2026-07-17),
    /// item-scoped update otherwise. [WINDOWS-VERIFY] runtime.
    fn notify_item_changed(&self, path: &str, is_dir: bool) -> PortResult<()> {
        let event = if is_dir { SHCNE_UPDATEDIR } else { SHCNE_UPDATEITEM };
        let wide = HSTRING::from(path);
        // SAFETY: `wide` outlives the call; SHCNF_PATHW names the pointer type; FLUSH makes the
        // event synchronous so a batch of notifications lands before the caller returns.
        unsafe {
            SHChangeNotify(event, SHCNF_PATHW | SHCNF_FLUSH, Some(wide.as_ptr() as *const _), None)
        };
        Ok(())
    }
}
