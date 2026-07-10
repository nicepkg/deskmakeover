//! Explorer icon-cache refresh, ported from `DeskMakeover.Shell/ExplorerRefresh.cs`. A light,
//! non-disruptive `SHChangeNotify` — never an Explorer kill (spec 01 Safety Rules).

use dm_domain::{ExplorerRefresher, PortResult};
use windows::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};

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
}
