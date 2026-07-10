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

**Mac-first execution note (ADR-0019 Amendment 1, owner order).** Everything verifiable
on a Mac is built and verified on a Mac now: the Tauri UI, the full Rust icon core vs the
frozen TS oracle corpus, and the WASM preview. Windows platform code is **blind-written
behind `cfg(windows)`** and kept compiling via `cargo check --target
x86_64-pc-windows-msvc` (`rustup target add` — `check` needs no linker). Concretely:
**M3/M4 split into a "blind-write on Mac" half** (Rust behind `cfg(windows)`, compiles but
does not run) **and a "verify on Windows" half** that batches with M1 spikes 1/2/5 when
the owner is at his box; **M2's "runs on Windows" exit is deferred into that Windows
batch** (M2 still ships and is accepted on Mac).

## M0 — Freeze truth (Mac, ~0.5d)
Scaffold cargo workspace per ADR-0019 layout (Amendment 1: web at the repo root, not
`apps/`); move `src/DeskMakeover.Web` → the repo root `src/` (path-only move, imports
fixed, 356 tests stay green); freeze
banners into TS compositor + C# projects; capture the TS oracle corpus: golden 256px
renders + stage dumps (profile/masks/rim/seeds) over the mock PNGs + perf
baselines (48px, 256px warm).
**Exit**: corpus committed under `testdata/icons/`; `bun test` + `tsc -b` green from
the new path; CHANGELOG Unreleased notes the replatform.

**M0b DONE (oracle corpus).** Committed under `testdata/icons/` (1,368 PNGs, 19 MB;
sources referenced, not copied). The mock pack is **120** synthetic 256² PNGs — NOT
488 (that figure was stale). Tiers: **A** = full desktop under the spectrum default
(120 masters + resolved config/lane per item); **B** = 24-source style matrix, 47
cells each (7 presets · 12 shapes · 4 subjects · 5 plate stops · 2 distinctions ·
8 marks · 5 filters · shortcut badge · peek) = 1,128 cells; **C** = one hue-spread
session JSON per look (decode seed + resolved fieldSeed). Per-source stage dumps:
profile JSON (classification, own-background verdict + anchor, corner-symmetry, rim
band + majority hex, dominant, foreground bbox, matchesShape/maxScaleInside) +
subject-mask PNG. The real Win11 shortcut-arrow badge is loaded and composited exactly
as the app worker does, so Keep/peek goldens carry the true badge (its sha256 is pinned
in the manifest). Harness `scripts/capture-oracle.ts`
(`--capture` / `--verify [--sample N]`, deterministic — capture-twice byte-identical);
parity anchor is the per-cell RGBA sha256. CI smoke `tests/oracle-corpus.test.ts` runs
`--verify --sample 12`. Additive read-only diagnostics export added to the frozen
`compose.ts` (compose lane) + `analysis.ts` (`cornersSymmetric`) — no pixel change, 357
tests green. Lane coverage: 9/10 compose lanes + 5/6 field sub-lanes (the `empty` guard
and the `derived-plate` field branch have no source in the 120-pack — unit-tested
elsewhere, not fabricated). This harness is the TS side of the M5 tri-target
differential.

## M1 — Go/no-go spikes (Windows, ~2d, gate for everything after)
The five ADR-0019 spikes: STA actor + IFolderView reads · cross-process
SysListView32 layout (or prove IFolderView suffices) · elevated helper roundtrip
(overlay-29 apply/restore + UAC cancel) · tri-target pixel slice (one style, ~100
sources: TS vs wasm vs native) · kill-injected `.lnk` transaction with CAS conflict.
**Exit**: all five pass, or the failing one is escalated to the owner with a
re-priced alternative before proceeding.

**Spike 4 DONE (tri-target pixel slice, Mac).** Slice = Circle + fixed white
plate + subject blit + dock silhouette shadow, 120 sources × {256, 512} = 240
cells (118 area-averaged / 122 supersampled — both drawScaled lanes). Result:
**native↔wasm 240/240 byte-identical; TS↔Rust 0 diff bytes / 157,286,400**
(byte-equal, exceeding the SSIM≥0.995 gate). Rust side: `dm-icon-core`
{js_math, raster, shapes, color, analysis, sampling, slice} (libm-only, real
M5 module layout) + plain-`extern "C"` wasm adapter (wasm32-unknown-unknown,
no wasm-bindgen, run under Bun's WebAssembly — recorded choice) + `xtask
spike4-native/spike4-compare`. One command: `bun
tests/icon-parity/spike4/run.ts`. Key intel for M5: JSC `Math.pow` vs
`libm::pow` differ by 1 ulp on 34/256 sRGB decode-LUT entries (measured by the
fixture probes; never surfaced in pixel bytes here) — TS↔Rust byte-equality is
NOT guaranteed by construction near rounding boundaries, so the full-corpus
differential stays the certification gate; wasm↔native equality IS structural
(same libm bits). JS-semantics helpers that made bytes match: `js_round`
(ties → +∞, and NOT `floor(x+0.5)`), `Uint8ClampedArray` ties-to-even at
non-integer stores, f32-store/f64-accumulate in the shadow `boxBlurInPlace`,
`Math.trunc/ceil` mirrors in the area-average sampler.

## M2 — Tauri foundation (Mac-first, ~2d)
`src-tauri`: host the React app; mock bridge rides Tauri commands
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

**M3/M4 blind-write DONE (2026-07-10, Mac-first — Windows runtime pending).** Task
plan `2026-07-10-m34-windows-blind.md` (incl. the [WINDOWS-VERIFY] checklist).
Ports & adapters: `dm-domain` (I/O-free kernel: item/fingerprint/restore-anchor/
ports) ← `dm-operations` (pure transaction + incremental ledger, crash-durable WAL
upgrade over C#'s in-memory JournaledOperationRunner) + `dm-windows` (COM adapters,
cfg(windows)) + `dm-elevated` (requireAdministrator bin, verb whitelist + LPE
guards). 55 tests green on Mac via in-memory fakes, incl. the kill-point battery
(every journal truncation point + torn-write variants → each item exactly original
or target). C# invariants harvested as named tests (CAS conflict abort, LIFO
rollback, anchor-before-mutation, corrupt-ledger-never-reads-empty, pinned hue
seeds). Three deliberate divergences per ADR-0019/0020: per-item CAS skip (not
batch abort) · incremental re-apply (true original locked in the ledger, no
restore-first) · durable WAL journal. dm-domain/dm-windows/dm-elevated pass
`cargo check --target x86_64-pc-windows-msvc` in isolation (kept zero-C-deps);
the workspace-wide msvc check is blocked on Mac by rusqlite's bundled SQLite C
source (needs Windows SDK headers) — ruled: runs natively in the Windows batch,
no cargo-xwin. Two documented stubs await Windows: `shell/layout.rs`
(IFolderView2 positions) and `watcher.rs` (ReadDirectoryChangesW, M7 scope).

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

**M5 DONE (2026-07-11, certified byte-exact).** Full TS pixel pipeline in
`dm-icon-core` (task plan `2026-07-10-m5-icon-core.md`; 10 modules + RenderSession,
50 unit tests, all files ≤500 lines). One-command certification
`bun tests/icon-parity/m5/run.ts` — ALL GATES PASS, independently re-run at
acceptance: shape masks 48/48 bit-identical · StageProfile 120/120 deep-equal +
masks 120/120 byte-equal · hue-spread 7/7 · **full-corpus pixel differential
1248/1248 cells byte-identical (0/327,155,712 diff bytes), every lane/fieldLane
exact**. Zero logged pixel-byte exceptions — certification landed at byte-equality,
tighter than the SSIM gate above. Exactness intel: `x**2` ported as explicit `d*d`
(dodges the JSC-vs-libm pow 1-ulp LUT drift, which never surfaced anywhere); all
byte stores pre-rounded via js_math (the Uint8ClampedArray ties-to-even path never
fires in practice — kept as defence). `slice.rs` absorbed into `compose/`.
Remaining for M6: full render_tile wasm export + Config ABI serialization +
wasm↔native CI. Follow-on M5.11 in flight: `dm-icon-codec` ICO writer + content
hash from the C# IcoWriter oracle (serves the ledger's content-addressed AssetRef,
ADR-0021 overlay ICOs, RecycleBin empty pair).

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