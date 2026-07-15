# Owner-supervised live runs — verification runbook (Tauri stack)

> The ONE gate no automation may cross (ADR-0019, spec 04 §5, spec 01 Safety): the REAL desktop
> **icon bake**, the **arrow-overlay elevation**, the **wallpaper apply**, the **resident
> auto-format**, and the **calm writes**. Every one mutates the live desktop / registry, so the
> trigger is a HUMAN CLICK by design — never a script, a test, or an AI. This runbook is the owner's
> checklist for signing them off on a real Windows box.
>
> **Read surface is already verified** (2026-07-15, `cargo run -p dm-windows --example
> verify_readonly` — scan/topology/geometry/extraction/fingerprint/known-folders all PASS). This doc
> covers ONLY the WRITE gates that remain. Authoritative state: `docs/STATE.md`; tracker:
> `docs/ship-readiness.md`.

## Why it can't be automated

Each operation mutates live state (icon caches, the HKLM `Shell Icons\29` overlay, the actual
wallpaper, HKCU tweaks). The whole product promise is *reversible and supervised*, so the trigger is
a human click. The E2E suite stubs the mutation behind `DESKMAKEOVER_FAKE_APPLY=1`; that flag is OFF
in the real app. **If you ever see a bake happen without you clicking it, STOP — that is a bug.**

## Before you start

- **Launch the real app** one of three ways (all run REAL applies — `DESKMAKEOVER_FAKE_APPLY` must be
  unset):
  1. Install `target\release\bundle\nsis\DeskMakeover_0.1.0_x64-setup.exe`, launch from Start Menu.
  2. Run `target\release\deskmakeover-desktop.exe` directly (release, strict CSP — closest to shipped).
  3. `bun run tauri:dev` (dev loop; fine for a run, but #1/#2 are the ship-representative path).
- The title chip should read **0.1.0** (once wired) / the window title is **桌面美颜**.
- **Independent safety net:** before the first bake, note your current desktop icon arrangement +
  wallpaper (a photo on your phone is enough). The app takes its own snapshot too, but an independent
  reference lets you judge "zero residue" objectively.
- The very FIRST apply is the highest-risk moment: the pre-first-apply snapshot MUST fire + persist
  durably before any mutation, or the original is lost with no way back. Watch that the first restore
  returns things exactly.

## A. Icon bake (图标 module)

1. Open **图标**. Confirm the mirror shows your REAL desktop — icon count + look match reality (the
   icons are the live-extracted `dmicon://` sources, verified in the read pass). If the mirror is
   empty or wrong, STOP.
2. Pick a look (e.g. a preset from 风格库) and press **一键美化** (`Cta_Apply`).
   - Watch: the CTA goes to **正在应用…** then **✓ 已与桌面同步** (`Cta_Synced`).
   - Watch (first time / if the arrow overlay is engaged): ONE UAC prompt for **dm-elevated.exe**
     (the batched elevated helper writing the HKLM overlay). Approve it. See §C for what to verify on
     that prompt.
3. Minimize the app and verify on the REAL desktop: icons wear the new look, the arrangement is
   intact, no icon is missing or generic-broken. `.lnk` shortcuts, the Recycle Bin, folders, `.url`
   items, and loose files should all restyle (each uses a different writer — spot-check one of each).
4. Press **还原系统默认** (`Cta_RestoreDefault`) / the **还原** link.
   - Verify: every icon returns to its original icon, arrangement intact, and the toast reads
     **已还原系统默认 · 无残留** (`Restored`). **ZERO residue** is the load-bearing guarantee — no
     leftover `.ico` files under `%LOCALAPPDATA%`, no stale overlay, no hidden+system wrapper files
     left beside a formerly-wrapped loose file. If ANYTHING is left behind, STOP and report before
     shipping.
5. Edge passes worth one run each:
   - **Deny the UAC prompt**: press 取消 on the dm-elevated UAC. Expect the non-privileged styling to
     still apply and the arrow step to stay retryable — toast **图标已美化 · 隐藏箭头一步已跳过（未授权）**
     (`Toast_AppliedNoOverlay`), never a dead-end.
   - **Apply → reboot → restore**: the restore anchor + residue clearing must survive a reboot.
   - **Edit an icon externally between apply and re-apply**: a hand-changed icon must be CAS-skipped
     (toast `Toast_ApplySkipped` "跳过 N 项"), never silently clobbered.

Recovery if step 4 leaves residue: the restore anchor is KEPT on a failed restore (never deleted).
Re-run 还原; if it still fails, the per-user icon-cache reset (`ie4uinit.exe -show`) + an Explorer
restart is the manual fallback — but a residue failure is a **release blocker**, not a shrug.

## B. Wallpaper apply (壁纸 module)

1. Open **壁纸**. Add a zone (drag on the mirror or use a recommended layout). Confirm the composed
   preview looks right — **this preview is byte-identical to what will be baked** (WYSIWYG law).
2. Press **应用到壁纸** (`Paper_Cta_Apply`) — or **应用到屏幕 N** (`Paper_Cta_ApplyScreen`) on a
   multi-monitor setup.
   - Watch: the real desktop wallpaper becomes the composed image; the app toast confirms the
     original was backed up (or that a slideshow was paused).
3. Verify on the REAL desktop: the zoned wallpaper is set; zone panels align with the icon grid
   (icons sit inside the panels you drew).
4. Press **换回我的壁纸** (`Paper_Restore`).
   - Verify: your original wallpaper is back EXACTLY (a slideshow resumes; a solid colour returns to
     that colour, not a leftover DeskMakeover image). The backup is byte-copied and never overwritten,
     so this must be lossless.
5. Edge passes:
   - **Solid-colour start** and **slideshow start**: apply then restore from each — the original must
     return, not a stuck static image (M34 item #19).
   - **Change the desktop icon size AFTER applying**, reopen 壁纸 — the fingerprint-mismatch banner
     must appear (it must NOT silently re-bake); press 重新合成 to regenerate against the new grid.

Recovery: a failed apply does NOT change the desktop (backup-before-mutate). A failed restore keeps
the anchor — re-run 换回我的壁纸. The original is never at risk.

## C. Arrow-overlay elevation (dm-elevated + UAC) — [WINDOWS-VERIFY] #7

The global transparent-arrow overlay is the DEFAULT (ADR-0021); it writes HKLM `Shell Icons\29` via
the elevated helper. The helper is now packaged (M8) as a self-contained sidecar, so this path is
finally runnable. Verify on the box:

1. On the UAC prompt raised by 一键美化, confirm the publisher/name is **dm-elevated.exe** and its
   path is inside the DeskMakeover install dir. (Unsigned for now → the prompt is the yellow
   "unknown publisher" style; that's expected until the cert lands.)
2. Approve → the shortcut arrows across the WHOLE machine (not just the desktop) switch to the
   transparent/refined overlay. Confirm on a Start-Menu shortcut + an Explorer folder, not only the
   desktop.
3. Restore: Settings › **恢复系统箭头** (`Settings_ArrowRestoreAction`) → confirm **恢复箭头**
   (`ArrowRestore_Confirm`) → ONE UAC → the classic Windows arrow returns everywhere, zero residue.
4. Security spot-check (the LPE fix): the shipped `dm-elevated.exe` must import NO `VCRUNTIME140.dll`
   (it is `+crt-static`) — `dumpbin /dependents` on the installed helper should show only KnownDLLs +
   `api-ms-win-*`. This is what makes elevating it from the user-writable install dir safe.

## D. Resident auto-format (spec 07) — [WINDOWS-VERIFY]

The decision core is built + Mac-tested; the real watcher→reconciler→driver loop only runs on Windows.

1. Enable auto-format (the consent strip / settings toggle). Confirm the tray icon appears and shows
   the correct state glyph.
2. Drop a NEW shortcut on the desktop. Within the debounce window (~4s) the resident should PROPOSE
   (or, after the 3-batch silent tier, silently apply) the saved look to just that icon — via store ①
   only, never elevating in the background (the §14 red line is structural: background never touches
   dm-elevated).
3. **Self-write suppression** (#9a): the resident must NOT treat its own writes as a new-icon event
   and format-loop. Watch that a single drop yields exactly ONE format, not a runaway.
4. **Explorer restart / sleep-resume catch-up** (#9b): kill+restart `explorer.exe` (or sleep→resume)
   with items appearing — on re-arm the resident should full-rescan so nothing landed-while-unwatched
   is missed.
5. **Burst** (#9c, the notify-8.2 overflow backstop): dump hundreds of files at once → confirm the
   periodic full reconcile still heals every final item (exactly one format each), even if the
   watcher dropped an overflow.
6. **Mid-batch desktop drag**: start dragging icons while a batch applies — the batch must stop and
   the remainder apply once idle (activity gate).
7. Close the window with automation ON → the process stays resident (tray still there, window
   reopens). Toggle automation OFF → zero autostart residue.

## E. Calm / 清爽 writes (ADR-0023 W3 cert lab) — [WINDOWS-VERIFY]

The W0/W1/W2 decision core is codex-approved; W3 is the real-box write ladder + the D2 gate.

1. Open 清爽. Press **一键清爽** (`Calm_Cta`) → inspect → apply the allowlisted recipes → verify →
   reboot → **恢复系统推送** (`Calm_Restore`) / per-item **恢复** (`Calm_RestoreOne`).
2. Confirm each recipe's `policy_guards` hold, GPP-locked settings are detected + reported (not
   silently failed), and a user-modified setting is left untouched on restore (toast
   `Calm_Toast_RestoredSkipped` "N 项你自己改过（未动）").
3. Capability gate: if this ladder turns green, the calm WRITE slice rides v1; if not, v1 ships the
   guided-only 「教你关」 face (ADR-0023 D2).

## Kill-point battery (#6) — the durability proof

For the icon bake specifically: kill the app (Task Manager → End Task) around each ledger transition
(mid-apply, after ① write, mid-commit, mid-restore) and relaunch. On every kill point, each item must
end EITHER at its true original OR at the target look — never a torn/half-styled state. Startup
recovery runs on launch (you'll see it drive the journal to a terminal state). This is the invariant
the whole transaction kernel exists to guarantee; it can only be proven on a real desktop.

## Sign-off

A + B + C clean (including the restore/residue checks), D + E per the capability gates, and the
kill-point battery holding → the WRITE surface is owner-verified end-to-end and the Windows-runtime
gate is fully closed. Any residue, any lossy restore, any silent re-bake, any half-styled kill-point,
or any background elevation → blocker; capture what happened and re-open the relevant section.
Once this is signed off, `0.1.0` can graduate toward `1.0`.
