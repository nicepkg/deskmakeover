# Plan — Tauri 2 + Rust replatform (master sequencing)

Decision: ADR-0019 (+ ADR-0020 resident v1, ADR-0021 arrow default). This is the
master phase plan; each phase gets its own bite-sized task plan (exact paths + code +
verify commands) authored at phase start. Owner effort currency: AI-agent days.
Panel pricing for the whole endpoint: 18–29 agent-days.

**Global constraints (apply to every phase)**
- DRY: one Rust core for preview/bake/background; contracts generated from Rust DTOs
  (tauri-specta); no hand-mirrored schemas; shared pixel primitives.
- Frozen oracles: TS compositor + all C# are frozen at M0 (banner comments). TS pixel
  code is deleted ONLY after M6's certification gate; .NET tree deleted at M8
  (`last-dotnet` tag). No new features land in frozen code.
- Parity law: classification/branch decisions exactly equal (TS↔Rust); wasm↔native
  byte-equal (libm routing, no FMA/SIMD in core v1); pixel gates SSIM≥0.995/bounded ΔE.
- Every phase ends with something runnable + its exit gate green before the next
  phase's irreversible steps.
- Windows verification runs on the owner's Windows box (SSH/Tailscale) in a logged-in
  interactive Explorer session — session-zero results don't count.

## M0 — Freeze truth (Mac, ~0.5d)
Scaffold cargo workspace per ADR-0019 layout; move `src/DeskMakeover.Web` →
`apps/desktop/frontend` (path-only move, imports fixed, 356 tests stay green); freeze
banners into TS compositor + C# projects; capture the TS oracle corpus: golden 256px
renders + stage dumps (profile/masks/rim/seeds) over the ~488 mock PNGs + perf
baselines (48px×300 warm, 256px×300 warm).
**Exit**: corpus committed under `testdata/icons/`; `bun test` + `tsc -b` green from
the new path; CHANGELOG Unreleased notes the replatform.

## M1 — Go/no-go spikes (Windows, ~2d, gate for everything after)
The five ADR-0019 spikes: STA actor + IFolderView reads · cross-process
SysListView32 layout (or prove IFolderView suffices) · elevated helper roundtrip
(overlay-29 apply/restore + UAC cancel) · tri-target pixel slice (one style, ~100
sources: TS vs wasm vs native) · kill-injected `.lnk` transaction with CAS conflict.
**Exit**: all five pass, or the failing one is escalated to the owner with a
re-priced alternative before proceeding.

## M2 — Tauri foundation (Mac-first, ~2d)
`apps/desktop/src-tauri`: host the React app; mock bridge rides Tauri commands
(browser mock loop stays intact); generated TS bindings from `dm-contracts`;
settings/look persistence in Rust (rusqlite); scoped asset protocol; CSP +
per-window capabilities; titlebar/window-state parity with the WPF shell; delete
`DeskMakeover.App` WPF presentation once the Tauri window hosts the app.
**Exit**: full UI runs in Tauri on Mac (mock) + Windows; window close verifiably
kills WebView children; binding drift is a CI failure.

## M3 — First vertical slice (Windows, ~2d)
One disposable user-desktop `.lnk`, end to end: Rust scan (identity + observed
position) → extract + normalize 256px source → asset URL → preview (frozen TS
renderer at this stage) → apply: native rerender from config, `dm-icon-codec` ICO,
durable prepared transaction, STA IconLocation write, Explorer refresh, verify,
commit → exact restore when untouched, conflict-safe when externally modified →
tray reopen restores state.
**Exit**: the slice survives the kill-point battery + restore leaves zero residue.

## M4 — Platform breadth (Windows, ~3-4d, parallel with M5)
Full apply/restore matrix (lnk/url/exe/folder/system/RecycleBin ×2 sources,
kindPolicy, per-icon overrides) · BakeService invariant harvest (each C# invariant
becomes a named Rust test) · incremental owned-field apply (NO restore-first port) ·
wallpaper get-source/apply/restore (`IDesktopWallpaper`, multi-monitor state
preserved) · `IcoWriter` + linear-light resampler ladder port · elevated helper
final (verb pair only) · global arrow overlay transaction per ADR-0021 · packaging
fix (helper path).
**Exit**: manual product complete on the new stack; C#-vs-Rust differential run on a
real desktop shows no semantic drift on the overlapping subset.

## M5 — Rust icon core (Mac-testable, ~4-5d, parallel with M4)
Port order (determinism-purity first, fixture gate per module): color/math →
shapes → raster → analysis/profile → segment → hue-spread + RenderSession →
compose → marks → filters → sampling. Rules: mirror TS Float32/Float64 precision
field-by-field; `libm` for transcendentals; no `mul_add`; no SIMD; JS rounding
helpers (`js_round`, `clamp_u8_round_half_even`) at byte boundaries; internal
re-architecture free (SOLID, session caching, scratch reuse) — external pixel
contract fixed. Port the 16 bun test files' semantics as Rust property tests;
stage-dump differential harness across the full corpus.
**Exit**: every module green vs TS oracle (classification exact + SSIM/ΔE gates);
wasm↔native byte-equal in CI; RenderSession serves register/analyze/setLook/render
with the persisted profile cache (`source_hash + analysis_schema_version`).

## M6 — Dual-target cutover (single truth source, ~1-2d)
Wire `dm-icon-wasm` into the existing Worker-pool adapter behind a dev comparison
flag (one WASM instance per worker); manual apply switches to native rerender
(already M3/M4); flip preview TS → WASM once BOTH gates are green: wasm↔native
byte-equality CI + TS↔Rust corpus certification. Then delete the TS pixel modules
(keep the worker adapter + PresetMinis facade), delete ArrowGateSheet (ADR-0021).
**Exit**: v1 renders every pixel from ONE Rust core in both targets; `bun test`
suite re-pointed at the WASM-backed renderer stays green; owner eyeballs the
before/after on his real desktop.

## M7 — Resident mode (spec 07, ~2-3d)
`dm-resident`: watcher (hints) + reconciler (truth) + job processor (native core,
persisted profile cache) + privileged queue + incremental ledger versions with
pinned hue seeds; tray/single-instance/autostart wiring; consent ladder surfaces
(proposal card, toggle chip, 新增图标 markers, audit summary).
**Exit**: spec 07 §Verification battery green (bursts, overflow, self-write,
kill-points, conflicts, OneDrive/sleep/multi-user matrix, idle budget).

## M8 — Release engineering + .NET deletion (~2d)
Authenticode signing (all PE payloads + installer, RFC 3161 timestamps) · NSIS
per-machine · WebView2 embedded bootstrapper · no updater/no crash upload (v1
policy) · fresh-VM matrix (Win10 22H2/Win11, DPI/IME/OneDrive/standard user/UAC
denial/interrupted apply) · owner-supervised live runs (manual + resident) ·
delete the entire .NET tree in one commit tagged `last-dotnet` · owner names the
version · repo public.
**Exit**: spec 00 release gate green; v1.0 ships on Tauri + Rust only.

## Parallelism map
M1 unblocks everything. M5 (pure core, Mac) runs alongside M2–M4 (platform,
Windows). M6 needs M4+M5. M7 needs M6 (native core certified). M8 last.

## De-scoped / explicitly NOT ported
Restore-first whole-desktop reapply (replaced by incremental CAS) · C ABI/cbindgen ·
resx i18n pipeline (TS dictionaries become source) · Pixi wallpaper stays web-only ·
regular-file wrapping as silent background behaviour (proposal queue, ADR-0020 §5) ·
rayon inside the core v1 · SIMD in core v1.