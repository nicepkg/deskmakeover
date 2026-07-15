---
updated: 2026-07-14
version: 0.1.0 pre-release (owner-named 2026-07-15; root package.json + src-tauri/tauri.conf.json = 0.1.0; stays 0.x until the owner-supervised Windows WRITE surface is human-verified, then 1.0)
branch: main (repo is PRIVATE; making it public is the owner's call at release)
---

# State

A POINTER, not a journal (dev-cycle ~150-line budget). Completed work is swept to
`docs/journal/2026-07.md` (append-only, grep-not-read). The living design is in `docs/specs/`;
the detailed ship tracker is `docs/ship-readiness.md`; this file says only what is TRUE now,
what is in flight, and what comes next.

> **Architecture (current, ADR-0019):** Tauri 2 + Rust. The .NET/C# host is RETIRED and
> **removed from the repo** (2026-07-14, ahead of the ADR-0019 M8 deletion). One Rust icon core (`dm-icon-core`,
> WASM preview/bake + native resident/background) is the single pixel truth; the TS compositor is frozen
> (tree-shaken out; physical deletion held to M8). UI = React in the Tauri webview (WebView2 on
> Windows). Bridge contract is GENERATED from `dm-contracts` via tauri-specta.

## Governing docs (current truth)

- `docs/ship-readiness.md` — the authoritative "what is left before a Windows user can install this
  and it works" inventory (milestones · ship-blockers [MAC]/[WIN] · [WV] surface · owner decisions).
  Owner decision 2026-07-12: polish everything Mac-closable to near-perfection first; Windows is
  final integration + the `[WINDOWS-VERIFY]` runtime pass only.
- ADR-0019/0020/0021 + `docs/plans/2026-07-10-tauri-migration.md` — the Tauri 2 + Rust replatform
  (M0–M8), background-resident v1 (spec 07), global transparent arrow default (60s gate retired).
- ADR-0022 + spec 07 — M7 常驻自动 format appearance model / reset / trust model.
- ADR-0023 + spec 08 + `docs/plans/2026-07-13-calm-windows-module.md` — the 清爽 module
  (calm-Windows, 4th rail tile). Capability-gated release: the write slice rides v1 iff the
  Windows cert lab (W3) turns green; else v1 ships the guided-only 「教你关」 face.
- ADR-0013 (+ amendments) — v3 "Premium Flat": light-first OKLCH following system; bundled Inter +
  HarmonyOS Sans SC; in-app version narrative restored.
- Specs 00–09 are the intended source of truth (00 roadmap · 01 architecture · 02 visual language ·
  03 shell/settings · 04 wallpaper · 05 bridge · 06 icons · 07 resident · 08 calm · 09 preset packages).
- Runbook: `docs/development.md`.

## Bridge state

- Contract truth = `src/bridge/types.ts` `BRIDGE_SCHEMA_VERSION` (currently **9**) + the generated
  `src/bridge/generated.ts` (from `dm-contracts`). Wallpaper (schema 6), icons (schema 7), calm
  (schema 8) and preset packages (schema 9, `presets.*` + `dmpreset://`) all route through real Rust
  on Mac-Tauri; the frontend assembles the rich store shapes from thin bridge DTOs. Windows runtime
  for every native path is `[WINDOWS-VERIFY]` (preset-packages recipes in `docs/ship-readiness.md`).

## Active work (in flight)

- **Icons round — reset lens · panel P-B/H-A · preset packages · File shape · Comet mark
  (2026-07-15).** Owner-disposed 7 decisions; binding record
  `docs/reviews/2026-07-15-icon-preset-io-file-shape-arrow.md`; plan
  `docs/plans/2026-07-15-icon-preset-io-file-shape-arrow.md`; specs 06 §3.10/§3.13/§3.14 +
  02 Shape/Marks + NEW spec 09 (.dmpreset). P1–P7 BUILT (build record in the review doc):
  lens model · panel two-card + 风格库/history popovers · icon-look single serializer +
  versioned styleJson + migration chain · Rust preset store + presets.* (schema 9,
  dmpreset://, dialog picker grants) · import/export UI + drag-drop · File shape (ABI 12) ·
  Comet mark (ABI 7). Two codex adversarial passes CLOSED: FIX-6 (kindPolicy round-trip,
  recipe-rendered import preview, per-batch dedup, thumbnail bounded-decode+re-encode,
  archive-wide zip-slip/ratio screening, stage-first atomic swap) + focused security FIX-4
  (crash-recovery for interrupted swaps, write mutex, selected-state policy match; the
  Windows dir-rename flag was a non-issue — std replaces files atomically). File shape's
  cut corners SOFTENED (r6→r16 + s0.85) per owner 「别像狗啃」. Gates green: tsc 0 · bun 633 ·
  cargo (desktop+engine+ops) 462 · bindings ok · WASM rebuilt · playwright boards.
  Owner-facing: refresh to judge the softened File fold + the whole icons round.
  UNCOMMITTED (shared worktree w/ M6 — commit with literal pathspecs). NOTE: native-arrow
  60s gate RE-AFFIRMED (ArrowGateSheet untouched).
- **Wallpaper round 3 — material/title system + editor UX (2026-07-15).** Spec 04
  §2/§3/§4.1/§4.2 amended; binding record `docs/reviews/2026-07-15-zone-material-title-ux.md`;
  plan `docs/plans/2026-07-15-zone-material-title-ux.md`. BUILT (P1–P6, gates green:
  tsc 0 · 603 pass · bindings ok · playwright six-material/title/menu boards verified):
  six one-axis finishes (retired Luminous/Solid/Halo → Frost/Paper/Float migration on
  load), titles None/Etched/Chip/Bare/Bar (Tab→Chip), opacity 0–100 (glass tint, default 0),
  corner 0–60 (glass default 44, render guard shortestSide/2), touched-model material
  switching, WYSIWYG wallpaper-crop material tiles + caption, emoji beside the title in
  the zone list, zone context menu (icons dialect). PENDING: codex cross-review verdict,
  then owner look-approval; uncommitted.
- **清爽 W3 — cert lab (the ADR-0023 D2 gate).** VM ladder inspect→apply→verify→reboot→restore,
  populate the write allowlist, enumerate per-recipe `policy_guards`, rule on GPP limitations.
  Real Windows box; all `[WINDOWS-VERIFY]`. Lab green → the write slice rides v1; else guided-only.
  W0/W1/W2 are DONE + codex-approved (→ journal). The deferred refresh / `ms-settings:` launch
  adapters ride W3.
- **M6 kernel-speed + TS-pixel deletion** (concurrent session) — the WASM single-truth flip already
  EXECUTED (WASM is the only foreground production pixel path; resident/background uses the
  byte-identical native `dm-icon-core` build). Remaining: the byte-identical SIMD perf line
  (`docs/plans/2026-07-11-m6-kernel-speed.md`) + the physical deletion of the now-frozen TS pixel
  modules (`docs/plans/2026-07-11-m6-p4-cutover.md`), the deletion gated on WASM-vs-TS perf parity.
- **M7 resident — platform bodies.** Decision core DONE + hardened on Mac (→ journal); remaining is
  the [WV] platform layer: tray + windowless residency wiring, tray bitmaps, watcher→reconciler→
  driver loop, T2 judge-1 WinEventHook precision layer.
- **M8 release engineering — INSTALLER BUILDS (2026-07-15).** `bun run tauri:build` produces a
  working per-user NSIS installer (app + `dm-elevated` sidecar w/ requireAdministrator manifest +
  WebView2 downloadBootstrapper; gen-bindings excluded). Remaining: owner names the version (still
  `0.0.0`), Authenticode signing, updater, per-machine-vs-per-user call, `.dmpreset` file
  association, CI. (The frozen TS compositor still awaits physical deletion.)
- **The Windows-runtime gate** — READ half now CLOSED (2026-07-15): the app boots on a real Win11
  box, the WebView2 bridge routes, startup recovery + STA COM run, and the read-only `[WINDOWS-VERIFY]`
  surface (scan/topology/geometry/extraction/fingerprint/known-folders) passes (`verify_readonly`).
  WRITE half still PENDING + owner-supervised: icon-bake apply/restore, elevated overlay HKLM+UAC,
  wallpaper apply/restore, kill-point battery, resident auto-format loop, calm W3, M1 spikes 1/2/3/5.
  This write surface is now the dominant ship risk.

## Recently shipped (one line each — detail in journal/CHANGELOG)

- 2026-07-15 **FIRST WINDOWS-RUNTIME SESSION**: built + tested (683) + booted on a real Win11 box; read-only [WV] surface verified (`verify_readonly`); 2 Windows-only bugs fixed (comctl32 v6 test manifest, IDesktopWallpaper CLSCTX_ALL); M8 NSIS installer builds (app + dm-elevated sidecar). See `ship-readiness.md` §Windows-runtime session.
- 2026-07-14 清爽 W2: two Windows platform ports (WinregBackend + WindowsSystemProfileProbe), codex R5 Approve → journal.
- 2026-07-14 清爽 W1: Rust decision core + bridge schema 8, codex R7 Approve → journal.
- 2026-07-13 清爽 W0 + polish + schematics: web skeleton + rail 4th tile, codex R8 Approve → journal.
- 2026-07-12 M6-WIRE Wave B (B1-B10) + icon-bridge codex R12 Approve + M7 decision core → journal.
- 2026-07-11 M6 single-truth WASM flip EXECUTED + wave-2 hardening + arrow-restore UX → journal.
- 2026-07-10 Tauri 2 + Rust replatform (M2–M5 Mac-first) + all-real icon corpus certified → journal.

## Owner rules (durable)

- Accent = warm coral `#FF6F5E` only; blue/violet permanently banned (grep+test gated). Reviewed
  exemptions in `tests/banned-colors.test.ts`: OS-authentic depictions (Windows arrow `#0067C0`,
  taskbar chips) + the multicolour celebration confetti (one file).
- Light-first, theme follows system (ADR-0013). Version narrative restored (ADR-0013 amendment).
- No dashes in user-facing copy (reads as AI text). Every axis's 「无」 sits FIRST wearing
  slash-circle (dashed = auto, slash = none); ONE 16px keyline for all axis glyphs.
- Control scale unified app-wide: segmented `sm` (22px/11px), chip buttons 11px; page-scale
  touches the TEXT layer only.
- Arrow semantics (ADR-0021): the global transparent overlay is the DEFAULT; every shortcut redrawn;
  「保留原样」 = subject + baked classic arrow; the 60s penance gate is retired.
- ⛔ Icon subject pixels are never recoloured (ADR-0016 D8): looks differentiate via plates,
  silhouette shadows/halos, outlines, backgrounds — never by re-inking subjects.
- Visual work acceptance loop (owner order): a look/effect is done only when the designer-seat
  subagent passes a pixel-level acceptance on REAL renders; FAIL → iterate and resubmit.
- Extreme DRY; files ≤500 lines; WYSIWYG (preview == bake pixels); bake/apply owner-supervised.
- ⛔ Owner-supervised LIVE gates (never auto-triggered): icon-bake, wallpaper-apply, resident-mode
  audit, calm writes. Checklist `docs/verification/owner-supervised-live-runs.md` (Tauri rewrite pending).

## Blockers

- None for Mac web/core development (M0/M2/M5 run on Mac). The Windows box (SSH/Tailscale, logged-in
  interactive session) blocks: M1 spikes 1/2/3/5, the M3/M4 `[WINDOWS-VERIFY]` checklist, calm W3
  cert lab, and all M7 platform bodies.
- **Repo history purge — ✅ DONE 2026-07-14.** `legacy/` C# + stale evidence purged from all history
  (filter-repo, force-pushed); fresh clone `.git` 146 MB → 54 MB. Pre-purge backup bundle kept
  off-repo. Record: `docs/plans/2026-07-14-repo-history-purge.md`.

## Open questions (owner)

- Release version number + name (release time).
- Repo visibility: `nicepkg/deskmakeover` is PRIVATE — make public at release (the About card +
  免费开源 chip promise it).
- Signing entity/name for the OV certificate (release gate).
- Distribution channel (direct download + pinned comment reply).
