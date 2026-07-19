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
        // SHCNF_FLUSH (SYNCHRONOUS), not the async SHCNF_IDLIST it was: an async global refresh
        // RACES the just-finished desktop.ini/.url writes + asset GC, so Explorer re-reads a
        // transitional state and then never re-notifies — a folder / `.url` re-styled on a second
        // apply stayed on its stale (default / blank) icon (owner box 2026-07-17). `.lnk` items
        // self-refresh on the file change; the path-referencing kinds depend on this notification
        // actually landing after the writes. FLUSH waits for the shell handlers, matching the
        // proven-good reference's `SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_FLUSH, …)`.
        // SAFETY: null params; no ownership taken.
        unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST | SHCNF_FLUSH, None, None) };
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

    /// Restart the shell AND purge Explorer's icon-cache DBs, then relaunch one `explorer.exe` only if
    /// Winlogon's AutoRestartShell did not already bring it back (the conditional relaunch avoids a
    /// stray window). The reliable desktop-icon refresh (see the trait doc + owner box 2026-07-17).
    ///
    /// The icon-cache purge is NOT optional: the on-disk arrow-overlay `.ico` (`HKLM ...\Shell
    /// Icons\29`) is VISUALLY transparent (every pixel `(0,0,0,alpha=1)` — deliberately NOT
    /// alpha-0, see `dm_icon_codec::ladder::transparent_ico`), but Explorer caches the RENDERED
    /// overlay bitmap in `%LOCALAPPDATA%\...\Explorer\iconcache*.db`. Once a cold reload rendered
    /// a BLACK bitmap for it, every later restart re-served that cached black block over every
    /// shortcut (owner box 2026-07-17: "the small arrow became a huge black square on the second
    /// apply"; root-caused 2026-07-19: an all-zero-alpha or non-trivially-masked overlay bitmap
    /// does not survive the cache's serialize→deserialize round trip). A restart alone re-reads
    /// the registry but still trusts the poisoned cache; deleting the cache DBs (only possible
    /// while Explorer is down — it holds them open) forces a fresh render from the transparent
    /// `.ico`. This mirrors the proven reference tool's `Refresh-ExplorerIconCache`.
    ///
    /// Async (`.spawn()`) — the PowerShell owns the stop→purge→relaunch chain, so the caller returns
    /// without blocking and all desktop writes are already flushed. Best-effort: a spawn fault is
    /// swallowed (a stale icon must never fail the whole op).
    fn restart_shell(&self) -> PortResult<()> {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                // Stop Explorer so the icon-cache DBs it holds open can be deleted, purge them, then
                // relaunch only if the shell did not auto-respawn (no stray window).
                "Stop-Process -Name explorer -Force -ErrorAction SilentlyContinue; \
                 Start-Sleep -Milliseconds 500; \
                 Remove-Item -LiteralPath (Join-Path $env:LOCALAPPDATA 'IconCache.db') -Force -ErrorAction SilentlyContinue; \
                 Get-ChildItem -LiteralPath (Join-Path $env:LOCALAPPDATA 'Microsoft\\Windows\\Explorer') -Filter 'iconcache*.db' -File -ErrorAction SilentlyContinue | Remove-Item -Force -ErrorAction SilentlyContinue; \
                 & (Join-Path $env:windir 'System32\\ie4uinit.exe') -show; \
                 Start-Sleep -Milliseconds 300; \
                 if (-not (Get-Process -Name explorer -ErrorAction SilentlyContinue)) { Start-Process explorer.exe }",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
        Ok(())
    }
}
