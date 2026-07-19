//! Explorer icon-cache refresh, ported from `DeskMakeover.Shell/ExplorerRefresh.cs`. The light
//! notifications are `SHChangeNotify`; `restart_shell` is the owner-consented (2026-07-17)
//! disruptive path — a SUPERVISED stop→purge→relaunch that must leave Explorer provably alive.

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
    /// SUPERVISED and MUTEXED (2026-07-19 vanish fix). The old implementation spawned ONE
    /// fire-and-forget PowerShell that force-killed Explorer and hoped its own tail (or Winlogon
    /// AutoRestartShell) relaunched it — on machines where AV/policy killed that child after the
    /// kill but before the relaunch, the WHOLE shell (taskbar + every icon) stayed dead, restore
    /// re-ran the same dying chain, and the user saw "all my icons are gone and 还原 does
    /// nothing". Two chains could also interleave (apply's and restore's), the later kill
    /// orphaning the earlier relaunch. Now: a process-wide mutex serializes restarts, every step
    /// runs supervised from THIS process (kill → wait-for-exit → native cache purge → relaunch →
    /// VERIFY ALIVE with retry), and a shell that cannot be confirmed alive returns an error the
    /// caller must surface instead of a silent empty desktop. Blocking by design (a few seconds
    /// at the tail of an apply): correctness over latency. [WINDOWS-VERIFY] runtime.
    fn restart_shell(&self) -> PortResult<()> {
        use std::sync::Mutex;
        static RESTART_GATE: Mutex<()> = Mutex::new(());
        // Serialize restarts process-wide; a poisoned lock (a prior panic mid-restart) must not
        // wedge every future restart — take the guard either way.
        let _gate = RESTART_GATE.lock().unwrap_or_else(|p| p.into_inner());

        // 1. Stop Explorer (it holds the icon-cache DBs open) and WAIT for it to actually exit —
        //    the old fixed 500ms sleep raced Winlogon's respawn against the purge below.
        run_hidden("taskkill", &["/F", "/IM", "explorer.exe"]); // exit code ignored: may not be running
        wait_until(EXPLORER_EXIT_WAIT_MS, || !explorer_running());

        // 2. Purge the icon caches natively — no child shell to be killed halfway. Per-file faults
        //    are tolerated (a cache Explorer still holds is re-purged on the next restart).
        purge_icon_caches();

        // 3. Refresh the per-user icon registration (cosmetic, best-effort, bounded).
        run_hidden("ie4uinit.exe", &["-show"]);

        // 4. Relaunch and VERIFY. AutoRestartShell may have respawned Explorer already (that is
        //    fine — dedup by liveness, not by launching blind). Retry the spawn once; only a shell
        //    we cannot confirm alive after both attempts is an error — the caller surfaces it
        //    rather than leaving the user staring at an empty desktop wondering what happened.
        for attempt in 0..2 {
            if wait_until(EXPLORER_ALIVE_WAIT_MS, explorer_running) {
                return Ok(());
            }
            log::warn!("restart_shell: explorer not alive after wait (attempt {attempt}); launching it");
            let _ = std::process::Command::new("explorer.exe").spawn();
        }
        if wait_until(EXPLORER_ALIVE_WAIT_MS, explorer_running) {
            return Ok(());
        }
        Err(dm_domain::PortError::Io(
            "explorer did not come back after the shell restart; the desktop may be blank until \
             the user starts explorer.exe (Ctrl+Shift+Esc → run explorer) or signs in again"
                .to_string(),
        ))
    }
}

const EXPLORER_EXIT_WAIT_MS: u64 = 5_000;
const EXPLORER_ALIVE_WAIT_MS: u64 = 8_000;

/// Polls `done` every 200ms up to `budget_ms`; true iff the condition was met in time.
fn wait_until(budget_ms: u64, done: impl Fn() -> bool) -> bool {
    let start = std::time::Instant::now();
    loop {
        if done() {
            return true;
        }
        if start.elapsed().as_millis() as u64 >= budget_ms {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

/// Whether any `explorer.exe` process exists in this session (tasklist CSV filter — a stable
/// in-box tool; empty/`INFO:` output means none).
fn explorer_running() -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq explorer.exe", "/FO", "CSV", "/NH"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).to_ascii_lowercase().contains("explorer.exe"))
        .unwrap_or(false)
}

/// Runs a console tool hidden and WAITS for it (bounded by the tool's own runtime — taskkill and
/// ie4uinit are subsecond). Exit codes are intentionally not propagated.
fn run_hidden(program: &str, args: &[&str]) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    match std::process::Command::new(program).args(args).creation_flags(CREATE_NO_WINDOW).status() {
        Ok(_) => {}
        Err(e) => log::warn!("restart_shell: {program} failed to run: {e}"),
    }
}

/// Deletes `IconCache.db` + `iconcache_*.db` natively. Explorer is down when this runs, so the
/// handles are free; any straggler failure is logged and tolerated (re-purged next time).
fn purge_icon_caches() {
    let Some(local) = std::env::var_os("LOCALAPPDATA") else { return };
    let local = std::path::PathBuf::from(local);
    let _ = std::fs::remove_file(local.join("IconCache.db"));
    let explorer_dir = local.join("Microsoft").join("Windows").join("Explorer");
    if let Ok(entries) = std::fs::read_dir(&explorer_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if name.starts_with("iconcache") && name.ends_with(".db") {
                if let Err(e) = std::fs::remove_file(entry.path()) {
                    log::debug!("icon-cache purge left {name}: {e}");
                }
            }
        }
    }
}
