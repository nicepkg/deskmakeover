---
updated: 2026-07-16
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
- **M8 release engineering — INSTALLER BUILDS + SIGNING CI VALIDATED (2026-07-16).**
  `bun run tauri:build` produces a working per-user NSIS installer (app + `dm-elevated` sidecar w/
  requireAdministrator manifest + WebView2 downloadBootstrapper; gen-bindings excluded).
  Authenticode signing CI is END-TO-END VALIDATED (Certum SimplySign + self-hosted runner
  `deskmakeover-signer`; `git tag v* && push` → signed Release; runbook `docs/signing-setup.md`).
  Remaining: updater, per-machine-vs-per-user call, `.dmpreset` file association, runner service
  install (admin), the owner cutting the actual first tag. (The frozen TS compositor still awaits
  physical deletion.)
- **The Windows-runtime gate** — READ half now CLOSED (2026-07-15): the app boots on a real Win11
  box, the WebView2 bridge routes, startup recovery + STA COM run, and the read-only `[WINDOWS-VERIFY]`
  surface (scan/topology/geometry/extraction/fingerprint/known-folders) passes (`verify_readonly`).
  WRITE half still PENDING + owner-supervised: icon-bake apply/restore, elevated overlay HKLM+UAC,
  wallpaper apply/restore, kill-point battery, resident auto-format loop, calm W3, M1 spikes 1/2/3/5.
  This write surface is now the dominant ship risk.

## Recently shipped (one line each — detail in journal/CHANGELOG)

- 2026-07-16 **Codex full-project review round (owner "叫codex全面审查…有问题则修复")**: 3 parallel
  codex passes (native/security · frontend/wiring · build/CI/release), ~17 findings triaged. FIXED +
  committed (08553c1 + 48f0513, all green): the whole "deceptive UI / dead button" class the owner
  hates — calm boot fail-closed placeholder backend closing the mock-race fake-success (B1), calm
  probe/route rejection honesty (B2), wallpaper apply/restore/resetSource failure toasts (B3/B6),
  preset dialog error handling (B4), diagnostics false-"copied" (B5); wallpaper restore reversibility
  (A3, empty-path clear now propagates so the snapshot survives a failed restore); release tooling
  (first-release-can't-run, preflight, no Cargo churn, tag==version gate, stale-installer selection,
  doc drift C1/C2/C4/C5/C6/C7); and two owner-selected security-write-path fixes — elevated overlay
  junction/mapped-drive redirect closed (A2, GetFinalPathNameByHandleW + DRIVE_FIXED) and the undo
  CAS check-then-act window narrowed (A4). +8 regression tests. DEFERRED to the on-box write-surface
  pass (owner call): A1 (verify dm-elevated Authenticode signer before runas) + C3 (prove the sidecar
  is signed) — see Open questions.
- 2026-07-16 **Open-source readiness round (owner "准备开源")**: README revamp (English +
  README.zh-CN, hero screenshot from the mock UI, badges, features/install/build/architecture) +
  LICENSE (MIT) / CONTRIBUTING / SECURITY / CODE_OF_CONDUCT + issue forms & PR template (blank
  issues kept on so the in-app diagnostics deep-link survives). NEW `.github/workflows/ci.yml`
  (GitHub-hosted: ubuntu web gates + portable icon-core tests + wasm/build; windows-latest full
  `cargo test --workspace` + unsigned tauri build artifact) — the **signed** release stays
  self-hosted (Certum cert is bound to the signing PC; GitHub instances can't sign). NEW
  `scripts/release.mjs` (`bun run release`) single-sources the version across
  package.json/tauri.conf.json/Cargo.toml (corrects the 0.0.0 workspace drift on bump) → optional
  commit/tag/push. GitHub repo About set (description/homepage→releases/15 topics). CHANGELOG
  de-staled. Commit bc2c302. Owner still owns: going public + cutting the first `v*` tag.
- 2026-07-16 **Clean-system + links + grid/polish ship round (owner "欺骗性纯 UI/点了没反应")** →
  journal 2026-07 for the full record. Headlines: 清爽 backend was the devhost fake on ALL platforms
  → Windows now runs the real WinregBackend/profile ports under a FAIL-CLOSED manifest (no write pre-W3,
  ADR-0023 D2) + 「带我去关」 launches the real ms-settings: route (verified on-box) + honest guided-only
  hero face. Opener capability had EMPTY scope → all Settings links dead → granted allow-default-urls +
  $APPDATA-scoped open-path. app.getInfo → host getVersion (About was 0.0.0). Desktop grid reads the TRUE
  cell pitch (IFolderView GetSpacing/GetViewModeAndIconSize) → icons preview + wallpaper lattice no longer
  22px-drifted. Polish: slider cursor, required-field gating, ring-inset highlight, zone outline offset,
  lang sync, 10 dead ui/ deleted. **Font blur DIAGNOSED** (fractional DPR from the sqrt auto-zoom) — the
  DPR-snap fix carries a sharpness-vs-density tradeoff, left as an OWNER DECISION (see Open questions).
  codex: no P1; 3 P2 + 1 P3 fixed. Commits 6b5c2b9 + 14ab7ad.
- 2026-07-16 **Tray surface ship-wired (owner "菜单没反应" report)**: every §12 menu item now responds — deep-links (history/settings/reset via `resident://navigate` + app-store routing), 撤销最近一次整理 = `Reconciler::restore_batch` (snapshot-CAS, keeps the ledger row, 4 tests incl. superseded-restyle regression), toggle precondition feedback both tray+Settings, settings-poll enablement convergence; **§14 scope RESOLVED** (SHGetKnownFolderPath → foreground host + resident engine; watcher on real roots) — reset/version-switch/auto-format un-dormant on Windows; Settings gains the §13.2 恢复系统原始外观 row + auto-format switch un-hidden. Owner-box E2E via UIA tray clicks + CDP. codex: P1+P2 fixed, 2 documented.
- 2026-07-16 **Icons owner round**: System bucket merged into App (taxonomy + presets + Rust resolve, legacy keys tolerated); per-type accordion hover try-on (WYSIWYG with global axes, CDP-verified); File kind glyph folds top-right (matches the File shape); Comet badge = native arrow footprint (0.28); `{true&&}` dead wrappers cleaned. Codex pass: 1 finding, judged by-design (bareLook reset semantics).
- 2026-07-15/16 **Signing CI validated end-to-end**: Certum SimplySign + self-hosted runner; signed `DeskMakeover_0.1.0_x64-setup.exe`, Authenticode Valid + RFC-3161 timestamp, headless no-PIN. Runbook `docs/signing-setup.md`.
- 2026-07-15 **Pre-ship hardening**: WebGL context-loss recovery, bake MAX_TEXTURE_SIZE probe, WEBVIEW2_* env sanitizing (devtools feature); 2 codex review rounds (9 real fixes) + 3 owner-approved bugs (overlay registry raw+CAS, apply screen-bind, .url ANSI lossless).
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

- **Font blur — RESOLVED to A2 (owner 2026-07-16).** Diagnosed as fractional devicePixelRatio from the
  sqrt auto-zoom; owner chose A2 「密度优先」: KEEP the smooth auto-zoom (max density), and mitigate the
  softness with a CJK-only weight-500 bump at the small text steps (11–13px, `:lang(zh)` scoped, headings
  ≥14px + English/Inter untouched, desktop-mirror labels self-excluded → stay 400 WYSIWYG). Shipped in
  `src/index.css`. The DPR-snap (A1 sharpness) is deliberately NOT done — it would coarsen zoom density,
  which the owner prioritized keeping. If the mitigation proves insufficient on a specific laptop, A1
  remains the escalation.
- Release version number + name (release time).
- Repo visibility: `nicepkg/deskmakeover` is PRIVATE — make public at release (the About card +
  免费开源 chip promise it).
- Signing entity/name for the OV certificate (release gate).
- Distribution channel (direct download + pinned comment reply).
- **Elevated-helper hardening deferred to the on-box write-surface pass (codex 2026-07-16):**
  **A1** — verify `dm-elevated.exe`'s Authenticode signer in `run_helper` BEFORE the `runas`
  ShellExecuteExW (the per-user install dir is user-writable, so a swapped exe would run elevated;
  DLL planting is already closed by `+crt-static`). **C3** — confirm Tauri actually signs the
  `externalBin` sidecar; if not, add an explicit sidecar sign + `Get-AuthenticodeSignature` Valid
  gate to `release.yml`. A1 depends on C3. Both are [WINDOWS-VERIFY]. (A2 path-redirect + A4 CAS
  window were fixed 2026-07-16; A1/C3 need the on-box signed build to implement+verify.)
