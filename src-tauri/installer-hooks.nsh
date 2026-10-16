; DeskMakeover NSIS installer hooks (Tauri `bundle.windows.nsis.installerHooks`).
;
; PREUNINSTALL: a per-user uninstall must leave NO residue (the module constitution's
; "one-click reversible with zero residue" promise). BEFORE the uninstaller removes our files we run
; the app's headless full restore — it reverts every surface we style (desktop `.lnk`, profile-folder
; `desktop.ini`, taskbar pins, Start-menu shortcuts, fixed drives, appx wrappers), the wallpaper, and
; the machine-wide arrow overlay. The HKLM overlay restore raises ONE UAC (the uninstaller is
; per-user); every other path is HKCU/user-scope. Best-effort: an app that cannot restore must still
; let the uninstall proceed, and the leftover data dirs are removed afterwards.

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "DeskMakeover: restoring your desktop before uninstall..."
  ExecWait '"$INSTDIR\deskmakeover-desktop.exe" --uninstall-restore' $0
  ; Drop the per-user data (settings, ledger, cached icon-assets, markers) and the elevated overlay
  ; copy under ProgramData. The restore above already reverted the styling these pointed at.
  RMDir /r "$APPDATA\com.xiaominglab.deskmakeover"
  RMDir /r "$LOCALAPPDATA\com.xiaominglab.deskmakeover"
  RMDir /r "$PROGRAMDATA\DeskMakeover"
!macroend
