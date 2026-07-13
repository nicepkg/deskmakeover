# Multi-Screen Wallpaper · Session Resume · System Default — Design Spec

**Date:** 2026-07-11 · **Mode:** dev-cycle iterate · **Status:** ✅ EXECUTED — historical design record
(owner decisions locked 2026-07-11; the multi-monitor data layer + switcher UI landed — see
`docs/journal/2026-07.md`). Windows runtime remains `[WINDOWS-VERIFY]`.
**Panel:** Chief PM + Chief UI/UX (isolated subagents) + Codex (implementation feasibility, cross-vendor).

## Owner decisions (locked 2026-07-11)
1. **Ship it all together** (Wave 1 quick wins + Wave 2 per-screen), not phased.
2. **Per-screen = independent designs** — each monitor has its own wallpaper look (the full isolation).
3. **Resume = last DRAFT** ("continue my experiment"), with `fingerprintMismatch` marking whether the live desktop actually reflects it. Unified across icons + wallpaper.
4. **First install = keep the instant beautified preview**; "System Default" is a *reset escape hatch* (first in the style row), NOT the default first-paint.

## The load-bearing finding (reframes effort/risk)
The hard OS layer is **already built**: `crates/dm-windows/src/wallpaper.rs` uses `IDesktopWallpaper` with per-monitor `GetWallpaper`/`SetWallpaper(monitorId,…)`, `GetMonitorDevicePathCount/At`, `GetMonitorRECT`, `WallpaperSnapshot { monitors, position, slideshow_active }`, and already detects slideshows. The single-screen collapse lives entirely at the **bridge + web** layer (`WallpaperStateDto` carries one look/grid). So this is mostly **contract + TS** work, low platform risk.

## Scope
Wallpaper multi-monitor + session resume + a reset preset. **Non-scope:** per-screen *icon* styling (icons stay primary-monitor-global — a separate, larger question); third-party live-wallpaper (Wallpaper Engine/Lively) detection (infeasible via documented Windows APIs — see §A4).

## Assumptions / Dependencies
- Rust host per-monitor wallpaper adapter exists and is correct (dm-windows) — but is `[WINDOWS-VERIFY]` (never run on real Win11; owner's box verifies).
- Icons/wallpaper persistence round-trips through the host settings store (SQLite). Note the broader `[M6-WIRE]` reality: many `icons.*`/`wallpaper.*` verbs currently hit the web mock in a real build; per-screen wallpaper needs its host verbs actually wired (tracked separately).

---

# A. Wave 1 — single-screen quick wins (no contract refactor)

### A1. "System Default" reset preset (owner #4)
- A new preset row, **first** in the style list, id `system-default`, i18n `Preset_SystemDefault` = 「系统默认」, desc 「回到未美化的原始桌面」.
- It is a **reset affordance, not an applied style**: selecting it runs the existing `restore`/show-original path (icons return to bare), NOT `apply`. `activePresetIdOf` must light it when the desktop is un-styled (today no preset maps to "bare").
- `ascast` is NOT this — `ascast` still applies Apple shape + Ring mark + tidied plates. System Default is truly untouched.
- **First-install:** boot the existing beautified preview (spectrum) as today; System Default sits first-in-row, selectable, unselected. Do not boot bare.
- **Acceptance:** first row of the style deck is 系统默认; clicking it returns every icon to its original unmodified art; the row highlights when (and only when) the desktop is un-styled; no `apply`/host write is triggered by selecting it.

### A2. Portrait preview fit-to-height (owner request)
- `useCanvasView` currently defaults `initialFitMode:'all'`; portrait screens open as a tiny centered strip. `canvas-view.ts` already implements `FitMode 'height'`.
- On selecting a screen whose `height > width`, default `view.reset('height')`; landscape/16:9 → `'all'`; ultrawide ≥21:9 → `'all'` (whole desktop visible for zone placement).
- Orientation flip while running re-picks the fit mode.
- **Acceptance:** a 1080×1920 portrait screen opens filling the preview vertically (letterboxed L/R), not a centered strip; a landscape screen still opens letterboxed; switching between them does not break the stage layout.

### A3. Resume last-draft + fingerprintMismatch surfacing (owner #3)
- Wallpaper already debounce-persists the draft look and reloads it on boot. Icons already persist config/overrides and rehydrate. Formalize: on launch, restore the last **draft** for icons AND wallpaper (unified), and **say so** in a status line:
  - draft matches live desktop → toast/status 「已恢复你上次的设计」.
  - draft edited but never applied (differs from desktop) → status 「上次未应用的草稿」 + CTA sits in its `dirty` phase + `Restore` available; use the existing `fingerprintMismatch` flag.
- **Acceptance:** relaunch lands on the last-edited state (icons + wallpaper) with an explicit status line stating whether the desktop reflects it; an un-applied draft never silently reads as "applied".

### A4. Dynamic wallpaper — smart fallback, not a dead-end (panel §6)
- Two cases: **Windows slideshow** — already detected (`slideshow_active`); we can beautify the current frame, just warn rotation won't re-arm. **Third-party engine** — invisible to `IDesktopWallpaper`; `GetWallpaper` returns empty → route the user to the existing `importSourceViaPicker` ("这块屏在用动态/视频壁纸，无法直接改造 — 导入一张静态图，或用当前画面").
- UI: **non-blocking amber `Reveal` banner** (reuse `fingerprintMismatch` grammar, never red) + a 「动态」 chip on the affected screen tile. First **apply that would overwrite a live wallpaper** gets one `ConfirmSheet` (「应用后将用静态壁纸替换这块屏的动态壁纸」); never ask again for that screen.
- **Acceptance:** a screen with no readable wallpaper source shows the import CTA (not an error); a slideshow screen is editable with a rotation warning; the destructive apply is confirmed once, then remembered.

---

# B. Wave 2 — per-screen independent wallpaper (owner #1, #2)

### B1. Contract change — `WallpaperStateDto` single-look → per-monitor
- `WallpaperStateDto` gains `screens: MonitorLookDto[]` + `activeScreenId`. Each `MonitorLookDto = { monitorId (device path), name, bounds{x,y,w,h}, orientation, look: LookDto, source: {…}|null, grid: WallpaperGridInfoDto, slideshowActive, hasReadableSource }`. Keep a top-level `position` (global) + `spanActive`.
- Bridge verbs widen: `wallpaper.getState` → screens[]; `wallpaper.setLook` → `(monitorId, look)`; `wallpaper.apply`/`restore` take a monitorId or "all".
- **Acceptance:** a strict client decoder round-trips the new DTO; a single-monitor host produces `screens.length === 1` (the app behaves exactly as today).

### B2. Store refactor — per-screen map + per-screen undo
- `useWallpaper` single `look` + one `past/future` → `screens: Record<monitorId, { look, source, past, future }>` + `activeScreenId`. All mutators operate on the active screen. Undo/redo is per-screen.
- **Acceptance:** editing screen A leaves screen B untouched; undo on A does not affect B; switching screens preserves each screen's in-progress edits and undo stack.

### B3. Persistence — device-path key + fallback + reconcile (Codex)
- Persist per-screen looks keyed `wallpaper.look.v2::<monitor-device-path>` in SQLite (store the full device path; hash it if used as a filename). **Device path is durable-ish, NOT permanent** across port/driver/dock/EDID changes → store fallback fingerprint metadata (bounds + EDID/DisplayConfig) and, when a saved screen can't be matched by path, match by fingerprint **only with a confirmation when ambiguous**.
- **Reconcile lifecycle** on launch + on monitor hot-plug/resolution/orientation change: re-key present monitors, keep detached-monitor looks dormant (don't delete — a reconnected monitor should resume), seed newly-present monitors with a default.
- **Acceptance:** relaunch restores each present screen's last draft by device path; a monitor unplugged then replugged resumes its look; a genuinely new monitor gets a clean default; a resolution change re-fits without losing the look.

### B4. Screen switcher UI — arrangement thumbnails (UX; kills the "slider")
- **Reject the slider** (continuous control for discrete spatial referents; useless for two identical monitors). Use a **floating glass pill at the canvas TOP-LEFT** (mirror the bottom-center CanvasToolbar grammar), rendered only when `screens.length >= 2`.
- Inside: mini tiles reproducing the OS "Displays arrangement" — each tile's **width:height = that screen's real aspect** (orientation shown by shape, no icon needed), tiles **positioned relative** to each other per reported bounds, **fill = a live crop of that screen's wallpaper** (answers "which wallpaper is which"), number badge + primary dot. Selected = `ring-2 ring-coral`.
- **Reject top-right** (collides with the 46px titlebar drag region + caption buttons).
- **Acceptance:** ≥2 monitors → the pill shows one correctly-shaped, correctly-positioned tile per screen with its wallpaper crop; 1 monitor → nothing renders; selecting a tile switches the active screen with the per-screen `正在编辑` header + the apply CTA renamed 「应用到屏幕 N」.

### B5. Per-screen mental model + Restore (UX + Codex global-state wrinkle)
- Three anti-wrong-screen signals: the coral-ringed tile; a per-screen 「正在编辑 · 屏幕 N（竖屏）」 header (reuse `Zone_EditingHeader`, crossfades on switch); the CTA names the target 「应用到屏幕 N」. Canvas shows ONE screen at a time.
- **Restore wrinkle:** wallpaper position/slideshow/bg-color are GLOBAL (only image paths are per-monitor). Reversibility = **one durable whole-desktop pre-first-apply snapshot** (all monitors + global settings), not per-screen restore. A per-screen "restore this screen" restores that monitor's image but cannot independently restore a prior global slideshow/position mode — document this; the master Restore reverts the whole snapshot.
- **Acceptance:** the user can always tell which screen they're editing; apply can never silently hit the wrong monitor; a whole-desktop restore returns every monitor + global settings to the pre-first-apply state.

### B6. Span mode + edge cases (PM landmines)
- If desktop `position == Span`, Windows uses ONE image across all monitors → per-screen isolation is undefined. **Detect Span and degrade** to a single unified canvas (with a one-line note), don't fight it.
- 0 monitors detected → one virtual screen, never crash-empty. Mirrored/clone → treat as one surface. Ultrawide → one wide screen (fit-all).
- **Acceptance:** Span desktop → unified single canvas + note; 0 monitors → virtual screen fallback; none of these crash or empty the UI.

---

## Non-goals (explicit)
- Per-screen **icon** styling — icons stay primary-monitor-global.
- Detecting arbitrary third-party live wallpapers — infeasible; §A4 fallback only.
- Promising permanent monitor identity — device path is best-effort.

## Effort (Codex + panel; AI-accelerated senior dev)
- Wave 1 (A1–A4): quick wins, no contract refactor — ~1 day total.
- Wave 2 per-screen wallpaper (B1–B6): ~8–16 h backend/contract (Rust adapter already exists) + the store refactor + switcher UI. The bridge-contract redesign touches every wallpaper file — its own review cycle.
- Total: a few days end-to-end; the risk is not platform (done) but the contract/store refactor breadth + `[WINDOWS-VERIFY]` on real hardware.

## Verification
Host verbs on real Win11 (owner's box) for per-monitor apply/restore/reconcile; strict-decoder round-trip of the new DTO; per-screen isolation + undo tests; Span/0-monitor/hot-plug simulations; visual acceptance of the switcher + portrait fit by the design seat.
