// Hide the extra console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Task 11 launch fallback (ADR-0024 D6): appx wrapper `.lnk`s launch THROUGH this binary —
    // `--activate-aumid <AUMID>` activates the Store/MSIX app via IApplicationActivationManager
    // and exits WITHOUT booting the GUI (this box's `explorer.exe shell:AppsFolder\…` route is
    // verified broken on-box — three CLI attempts + a real wrapper click all failed to activate —
    // while the COM API returns S_OK; evidence in the plan's Task 11 gate log). Exit codes: 0
    // activated · 2 missing argument · 3 activation failed — the wrapper's own launch reports
    // honestly through the process exit code.
    let args: Vec<String> = std::env::args().collect();
    // Uninstall-residue cleanup (plan "Uninstall residue" finding): the NSIS `NSIS_HOOK_PREUNINSTALL`
    // hook runs this BEFORE the uninstaller removes our files, so a per-user uninstall REVERTS every
    // surface we styled (icons across all surfaces + wallpaper + arrow overlay) instead of leaving
    // residue. Never boots the GUI; exits with the restore's own status code.
    if args.iter().any(|a| a == "--uninstall-restore") {
        std::process::exit(deskmakeover_desktop_lib::run_uninstall_restore());
    }
    if let Some(pos) = args.iter().position(|a| a == "--activate-aumid") {
        let code = match args.get(pos + 1) {
            Some(aumid) if !aumid.is_empty() => {
                if dm_windows::shell::appsfolder::activate_app(aumid).is_ok() {
                    0
                } else {
                    3
                }
            }
            _ => 2,
        };
        std::process::exit(code);
    }
    deskmakeover_desktop_lib::run()
}
