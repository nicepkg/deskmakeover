# Multi-Screen Wallpaper — Implementation Plan

**Date:** 2026-07-11 · **Spec:** `docs/product/multi-screen-wallpaper.md` (owner-approved) · **Mode:** dev-cycle iterate.
**Acceptance:** all tasks built + lead-verified (build + dev-app E2E on mock multi-monitor) → **Codex end-to-end acceptance** (owner directive: build all, then Codex accepts).

## Global constraints (every task)
- **Single-monitor parity:** with `screens.length === 1`, the app must behave EXACTLY as today. This is the regression guard.
- **Strict client decoder:** the widened `WallpaperStateDto` must round-trip through the existing strict decoders (a widened payload that a strict decoder rejects → silent empty state — known trap). Add/adjust decoders in the SAME task that widens the DTO.
- **Icons stay primary-monitor-global** — do NOT make icon styling per-screen (non-goal).
- **Host verbs are `[WINDOWS-VERIFY]`** — build against the **mock** (simulate 2-3 monitors incl. one portrait); real per-monitor host wiring is verified on the owner's Win11 box, not here. Every mock-only seam gets a `[WINDOWS-VERIFY]` marker.
- Cross-platform, ≤500-line files, kebab-case, delete-don't-comment, tests for non-trivial logic.
- Commit per task with a clear message; report status DONE/BLOCKED + evidence.

## File ownership (prevents collision — tasks run SEQUENTIALLY in this order)

### Task 1 — CORE (data layer). Agent: mscreen-core
**Owns:** `src/bridge/types.ts`, `src/bridge/mock-desktop.ts`, `src/bridge/mock.ts` (if decoders live there), `src/stores/wallpaper.ts`.
**Build:**
- **B1 contract:** `WallpaperStateDto` → add `screens: MonitorLookDto[]` + `activeScreenId` + top-level `position` (global) + `spanActive`. `MonitorLookDto = { monitorId: string /*device path*/, name, bounds:{x,y,w,h}, orientation:'portrait'|'landscape', look: LookDto, source: SourceRef|null, grid: WallpaperGridInfoDto, slideshowActive: boolean, hasReadableSource: boolean }`. Widen bridge verbs: `wallpaper.getState`→screens[]; `wallpaper.setLook`→`(monitorId, look)`; `wallpaper.apply`/`restore`→`(monitorId | 'all')`. Update strict decoders.
- **Mock multi-monitor:** `mock-desktop.ts` returns 2 monitors by default (one 1920×1080 landscape primary, one 1080×1920 portrait secondary) with distinct wallpapers; expose a way to simulate 1 / 3 / span / a slideshow / a no-readable-source screen for testing.
- **B2 store:** `useWallpaper` single `look`+`past/future` → `screens: Record<monitorId,{look,source,past,future}>` + `activeScreenId`; all mutators act on the active screen; per-screen undo; `selectScreen(id)`.
- **B3 persistence:** persist per-screen looks keyed `wallpaper.look.v2::<monitorId>`; reconcile on load (present→restore, detached→keep dormant, new→default); fallback fingerprint (bounds) when a device path can't be matched (mock: keep simple, mark `[WINDOWS-VERIFY]` for real EDID matching).
**Produces (interface for Task 2/3):** the `MonitorLookDto`/`WallpaperStateDto` shape + `useWallpaper` API (`screens`, `activeScreenId`, `selectScreen`, per-screen mutators/undo).
**Acceptance:** tsc clean; `bun test` green; single-monitor mock → identical behavior; per-screen edit isolation + per-screen undo tests; strict decoder round-trips the new DTO.

### Task 2 — UI + behaviors. Agent: mscreen-ui (AFTER Task 1)
**Owns:** `src/components/canvas/wallpaper-mirror.tsx`, a NEW `src/components/canvas/screen-switcher.tsx`, `src/lib/canvas-view.ts` (portrait), `src/components/panels/wallpaper-panel.tsx` (per-screen header + dynamic banner), wallpaper i18n keys in `src/lib/i18n/zh-hans.ts` + `en.ts`.
**Build (consumes Task 1's store):**
- **B4 switcher:** new `screen-switcher.tsx` — floating glass pill at canvas TOP-LEFT (mirror CanvasToolbar grammar), rendered only when `screens.length>=2`; mini tiles: width:height = each screen's real aspect, positioned per relative bounds, fill = live crop of that screen's wallpaper, number badge + primary dot; selected `ring-2 ring-coral`. Clicking → `selectScreen`.
- **B5 mental model:** per-screen `正在编辑 · 屏幕 N（竖屏）` header (reuse `Zone_EditingHeader`, crossfade on switch); apply CTA → `应用到屏幕 N`; canvas shows the active screen only.
- **A2 portrait:** on select, `view.reset(h>w ? 'height' : 'all')` (ultrawide ≥21:9 → 'all'); re-pick on orientation flip; opacity-dip transition on screen switch (120ms out / 180ms in) to mask the aspect change.
- **A4 dynamic:** non-blocking amber `Reveal` banner when `!hasReadableSource` (→ import CTA) or `slideshowActive` (→ rotation-won't-re-arm warning); `动态` chip on the affected tile; first destructive apply → one `ConfirmSheet`, remembered per screen.
- **B6 Span/edge:** `spanActive` → single unified canvas + a one-line note (hide switcher); 0 screens → one virtual screen fallback; never crash-empty.
**Acceptance:** tsc + `bun test` green; dev app on mock 2-monitor (1 portrait) shows the switcher, portrait opens fit-height, per-screen header + CTA rename correct, dynamic banner appears on the simulated no-source/slideshow screen, span mock degrades to unified. Visual acceptance by the design seat.

### Task 3 — Icons: System Default + resume unification. Agent: mscreen-icons (AFTER Task 2)
**Owns:** `src/stores/icons.ts`, `src/components/panels/icons-panel.tsx`, icons i18n keys.
**Build:**
- **A1 System Default:** new preset `system-default` FIRST in the style row, i18n 「系统默认」/「回到未美化的原始桌面」; selecting it runs `restore`/show-original (NOT `apply`); `activePresetIdOf` lights it when the desktop is un-styled. Keep the beautified first-paint (spectrum) — System Default is the reset escape hatch, not the default. NOT `ascast`.
- **A3 resume (unify):** on launch restore the last DRAFT for icons (already persisted) + surface a status line (`已恢复你上次的设计` / `上次未应用的草稿` via `fingerprintMismatch`); coordinate with Task 1's wallpaper resume so both modules use the same last-draft semantic + status grammar.
**Acceptance:** tsc + `bun test` green; first row = 系统默认, click returns icons to bare, highlights only when un-styled, no host `apply` fired; relaunch shows last draft + an explicit applied/not-applied status line.

## Final gate
Lead verification (full build + `bun test` + dev-app E2E on mock multi-monitor incl. portrait/span/no-source) → **Codex end-to-end acceptance** against every spec acceptance clause → fix any finding → owner review. Real Win11 per-monitor apply/restore stays `[WINDOWS-VERIFY]` on the owner's box.
