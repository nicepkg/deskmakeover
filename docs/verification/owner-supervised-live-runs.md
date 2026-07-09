# Owner-supervised live runs — verification runbook

> ⚠️ **DO NOT EXECUTE AS-IS — pending F8 rewrite (2026-07-10).** This checklist predates the
> web-renders-pixels inversion and the schema-1→3 host work: the native host cannot yet drive
> Web v3, there is no published `v1.1.0` build, the byte-identical wallpaper claim no longer
> matches the dual-resolution path, and the icon-size edge pass was removed with the size control.
> Two supervised live runs are also NOT a release gate (they miss all F8 + packaging work). The
> live-run gate itself (a human click bakes the real desktop) stands; the STEPS here must be
> rebuilt after F8. Authoritative state: `docs/STATE.md`.

The one gate no automation may cross (ADR-0011 §7, spec 04 §5, spec 01 Safety
Rules): the REAL desktop icon bake and the REAL wallpaper apply. This runbook is the
owner's checklist for signing those two operations off on the real machine.

Why it can't be automated: both operations mutate the live desktop (icon caches,
overlay registry, the actual wallpaper). The whole product promise is *reversible
and supervised*, so the trigger is a human click by design — never a script, a
test, or an AI. The E2E suite runs them behind `DESKMAKEOVER_FAKE_APPLY=1`, which
stubs the mutation out; that flag is off in the real app.

## Before you start

- Run the published build: `artifacts\win-x64\DeskMakeover\DeskMakeover.App.exe`
  (title chip should read **v1.1.0**).
- Optional safety net: note your current desktop icon arrangement and wallpaper
  so you have an independent reference. The app takes its own snapshot too.

## A. Icon bake (图标 module)

1. Open 图标. Confirm the mirror shows your real desktop (icon count + wallpaper
   match reality).
2. Pick a look (e.g. 苹果极简) and press **一键美化**.
   - Watch: one UAC prompt (the batched elevated helper). Approve it.
   - Watch: the desktop icons restyle; the app CTA goes to **✓ 已与桌面同步**.
3. Verify on the REAL desktop (minimize the app): icons wear the new look, the
   arrangement is intact, no icon is missing or generic-broken.
4. Press **还原**.
   - Verify: every icon returns to its original icon, arrangement intact, **zero
     residue** (no leftover .ico files, no stale overlay). This is the load-bearing
     guarantee — if anything is left behind, STOP and report before shipping.
5. Edge passes worth one run each: deny the UAC prompt (should apply all
   non-privileged styling and leave the arrow step retryable, never dead-end);
   apply → reboot → restore (residue must survive a reboot and still clear).

Recovery if step 4 leaves residue: the restore anchor is kept on failed restore
(never deleted). Re-run 还原; if it still fails, the per-user icon-cache reset
(`ie4uinit.exe -show`) plus an Explorer restart is the manual fallback — but a
residue failure is a release blocker, not a shrug.

## B. Wallpaper apply (壁纸 module)

1. Open 壁纸. Add a zone (drag on the mirror or 用推荐布局). Confirm the composed
   preview looks right — **this preview is byte-identical to what will be baked**.
2. Press **应用到壁纸**.
   - Watch: the real desktop wallpaper becomes the composed image; the app toast
     confirms 原壁纸已备份 (or 幻灯片已暂停 if you were on a slideshow).
3. Verify on the REAL desktop: the zoned wallpaper is set; zone panels align with
   the icon grid (icons sit inside the panels you drew).
4. Press **换回我的壁纸** (restore).
   - Verify: your original wallpaper is back exactly (a slideshow resumes). The
     backup anchor is byte-copied and never overwritten, so this must be lossless.
5. Edge pass: change the desktop icon size AFTER applying, reopen 壁纸 — the
   fingerprint-mismatch banner should appear (it must NOT silently re-bake); press
   重新合成 to regenerate against the new grid.

Recovery: a failed apply does not change the desktop (backup-before-mutate). A
failed restore keeps the anchor — re-run 换回我的壁纸. The original is never at risk.

## Sign-off

Both A and B clean, including the restore/residue checks → v1.1 is owner-verified
end-to-end and clear to ship. Any residue, any lossy restore, any silent re-bake →
blocker; capture what happened and re-open the relevant phase.
