# ADR-0019 — Full replatform to Tauri 2 + Rust; .NET exits the product

- **Status**: Accepted (owner, 2026-07-10)
- **Inputs**: 4-seat adversarial architecture panel (chief architect, chief Windows
  platform engineer, chief algorithms engineer — isolated seats — plus Codex/gpt-5.6 as
  cross-vendor chief architecture engineer, 2 adversarial rounds), the ChatGPT
  consultation `docs/references/gpt-refactor-suggest.md`, and repo evidence cited below.
- **Supersedes**: ADR-0001 (the .NET/WPF half of the tech stack), ADR-0011 (WPF+WebView2
  host). **Amends**: ADR-0014 (wallpaper host I/O — source decode / PNG write / `SetWallpaper` /
  backup-restore move to Rust; the TS/Pixi compositor decision stands), ADR-0015 (renderer/oracle
  ownership — see §Oracles), ADR-0013 (version-narrative / changelog implementation moves to the
  Tauri/Rust host), ADR-0017 (the "C# host re-port at F8" Consequences become Rust contracts/native
  work), ADR-0018 (the schema 3→4 C# migration mechanism → generated Rust `dm-contracts`). Spec 05
  rewritten for the Tauri bridge; spec 00's "F8 Windows host pass" is void, replaced by the migration
  phases in `docs/plans/2026-07-10-tauri-migration.md`.

## Context

- The C# host was never wired to the shipped product shape: host bridge = schema 1
  (`Contracts.cs`), web = schema 4 (`types.ts`); controllers are 55-line shells on verbs
  the web abandoned three schemas ago. Phase F8 (host wiring) never started.
- The bridge transport (`BridgeDispatcher` → `CoreWebView2.PostSharedBufferToScript`) is
  WebView2-specific: the bridge/session/orchestration layer is greenfield in ANY
  language. "Ship on .NET first" means building that layer twice.
- The "validated C# safety layer" is partly myth (Codex, verified in code): native E2E
  fakes apply (`AppFixture.cs`); Shell-namespace scan returns an empty array
  (`DesktopScanner.cs`); the "journaled" runner is an in-memory rollback stack, not
  crash-durable (`JournaledOperationRunner.cs`); packaging puts the elevated helper where
  the runtime does not look (`publish-win.mjs` vs `OverlayBadgeService.cs`).
- What IS real: `DeskMakeover.Shell` COM interop edge cases (`DesktopLayoutReader`
  cross-process SysListView32 read + WorkerW fallback; `FolderIconWriter` ReadOnly-bit
  trick; `RecycleBinIconWriter` REG_EXPAND_SZ preservation), the `DesktopBakeService`
  reversible-transaction invariants (only documented in code), `IcoWriter` +
  `IconResampler`, and ~5,600 LOC of tests encoding regression knowledge.
- The owner's core ambition — a resident tray process that auto-formats new desktop
  icons — structurally demands a lean always-on process with a headless native renderer
  and no WebView. Once that native renderer exists, .NET is a third wheel.
- Panel effort pricing (AI-agent days): permanent .NET hybrid 13–21 · **full Tauri now
  18–29** · finish-F8-then-migrate 25–39 (worst path; the bridge layer is written twice).
- The app is unreleased with zero users: there is no cheaper moment to replatform.

## Decision

1. **Endpoint**: Tauri 2 + Rust. React 19/Vite/Bun front-end unchanged; Pixi wallpaper
   compositor stays web-only. .NET is absent from the released product.
2. **Single algorithm truth source ships in v1.0** (owner order 2026-07-10): the icon
   pixel pipeline is rewritten in Rust as ONE core compiled twice — WASM for the
   in-window preview/manual bake, native for apply and the resident background path.
   The TypeScript compositor is **FROZEN** (no new styles, no fixes except oracle
   corrections) and serves as the primary parity oracle; it is deleted only after the
   Rust core passes the parity gates. The rewrite may freely re-architect internals
   (SOLID, session/caching model, performance); **pixel-output parity against the frozen
   TS oracle is the external contract** — classification/branch decisions exactly equal,
   pixels within the §Parity tolerances.
3. **Legacy disposition**: freeze all C# immediately (banner comments; no new code).
   Delete the WPF presentation (`DeskMakeover.App` UI/ViewModels/theming) as soon as the
   Tauri shell hosts the web app. Keep `Shell`/`Operations`/`ElevatedHelper`/
   `IconRendering` + their tests as **executable oracles**: port semantics faithfully
   (BakeService invariants become named Rust tests during the port), run C# vs Rust
   differentially on a real desktop where useful, then delete the entire .NET tree in
   one commit tagged `last-dotnet`. (**Amended by Amendment 1**: the frozen tree is
   quarantined under `legacy/` until that M8 deletion, rather than left in place at the root.)
4. **Workspace structure** (maintainability-first; all `unsafe` confined to
   `dm-windows` + the helper):

```text
deskmakeover/                    # AMENDED by Amendment 1 (2026-07-10): app un-nested to the root
├─ package.json · index.html · vite.config.ts · tsconfig* · Cargo.toml · rust-toolchain.toml · deny.toml
├─ src/                        # React/Vite/Bun web app — the visible UI (moved from src/DeskMakeover.Web)
├─ public/                     # web static assets (fonts, mock-icons)
├─ src-tauri/                  # thin composition root: commands, lifecycle, tray, capabilities
├─ crates/
│  ├─ dm-domain/               # IDs, config, plans, typed errors (no I/O)
│  ├─ dm-contracts/            # serde DTOs → generated TS bindings (tauri-specta)
│  ├─ dm-icon-core/            # pure pixel algorithms + planner + RenderSession (no platform)
│  ├─ dm-icon-codec/           # normalized PNG, resample ladder, ICO assembly
│  ├─ dm-icon-wasm/            # wasm-bindgen adapter only
│  ├─ dm-operations/           # durable transaction ledger, snapshots, CAS restore
│  ├─ dm-windows/              # ALL windows-rs/COM/unsafe: STA actor, scan, layout,
│  │                           #   extract, .lnk/.url, desktop.ini, system icons,
│  │                           #   wallpaper, watcher, Explorer refresh
│  ├─ dm-resident/             # reconciler, job processor, privileged queue, policy
│  ├─ dm-elevated/             # requireAdministrator Rust bin crate, fixed verb whitelist
│  └─ dm-test-support/
├─ scripts/                    # web dev tooling: mock-icon gen, oracle capture, spike4 slice
├─ tests/                      # bun unit tests + icon-parity · recovery-killpoints ·
│                              #   windows-shell · webview2-smoke · installer
├─ testdata/icons/             # parity corpus: inputs / expected / profiles / manifest
├─ xtask/                      # binding/golden generation, package verification
└─ legacy/                     # FROZEN .NET oracle (Amendment 1): quarantined, deleted at M8
```

## Engineering discipline (binding)

- **DRY is structural** (owner order): one Rust core renders preview, manual bake, and
  background — never two pixel implementations again. Contracts are generated from Rust
  DTOs (`tauri-specta`); hand-mirrored schemas are banned (the schema-1/4 split is the
  proof they fail). Pixel primitives are shared modules, not per-caller copies.
- **wasm↔native bit-exactness**: transcendentals route through `libm` on both targets;
  `f64::mul_add`/FMA banned; no SIMD and no order-dependent parallel reductions in the
  core for v1; TS `Float32Array`/`Float64Array` precision is mirrored field-by-field
  (`f32`/`f64`), including JS rounding semantics at `Uint8ClampedArray` boundaries.
  Byte-equality between the two Rust targets is a CI gate, not an aspiration.
- **TS↔Rust parity gates**: classification/branch/plate-seed decisions exactly equal;
  pixels SSIM≥0.995 / bounded ΔE (blur/filter stages may carry documented regional
  tolerances). Stage-level differential dumps (profile, masks, rim stats, per-layer)
  locate any break to a single function. Golden updates require reviewed `--bless`.
- **Crate policy**: `dm-icon-core` depends on effectively `libm` + std only. tiny-skia,
  imageproc, palette, and fast_image_resize are REJECTED for the core (they replace
  tuned, oracle-locked math and add target-specific SIMD divergence); `palette` may
  serve as a dev-dependency test oracle. Platform side uses `windows-rs`, `notify`
  (hints only), `rusqlite` (backend-only ledger), `tracing`, `thiserror`, `cargo-deny`.
  Tauri plugins: single-instance (registered first), autostart, log, window-state,
  dialog; tray is core. `cbindgen`/C ABI is moot — the Tauri backend links the crates.
- **Parallelism ownership**: exactly one layer parallelizes per environment — the JS
  Worker pool in the front-end (one WASM instance per worker), `rayon` across icons on
  the native side only. Never nested.
- **Apply semantics**: the ported apply updates owned fields incrementally per item
  (compare-and-swap against the ledger fingerprint); the current C# restore-entire-
  desktop-before-reapply behaviour is NOT ported. Externally-modified items surface as
  conflicts, never silent overwrites.
- **COM discipline**: one persistent STA actor thread (`CoInitializeEx(APARTMENTTHREADED)`
  + message loop); COM interfaces never cross `.await` or Tokio threads; Tauri commands
  send owned DTOs to the actor. A disposable extraction STA worker isolates hangs from
  third-party shell extensions.

## Go/no-go spike gates (before bulk translation)

1. STA actor: enumerate desktop + read `IFolderView` positions from Rust (mixed DPI,
   Explorer restart).
2. Cross-process SysListView32 layout read (translate `DesktopLayoutReader`, WorkerW
   fallback) — or prove `IFolderView::GetItemPosition` suffices.
3. Elevated-helper roundtrip: signed dummy helper, `ShellExecuteExW`+`runas`, HKLM
   overlay-29 write + restore, UAC-cancel mapping, malicious-request rejection.
4. One complete style rendered TS vs Rust-WASM vs Rust-native on ~100 normalized
   sources (parity harness proof).
5. One `.lnk` transaction with kill-injection around every durable write; external
   modification surfaces as conflict.

A failed spike is a signal to re-price the premise, not to improvise around it.

## Adopted engineering defaults (owner may veto individually)

Win10 22H2+ / Win11, x64 v1 (ARM64 later) · NSIS per-machine (helper lives in Program
Files) · WebView2 embedded bootstrapper · no auto-updater and no crash upload in v1
(local rotating logs + user-triggered export; preserves the no-cloud promise) · i18n
source of truth becomes the TS dictionaries (the resx pipeline retires with the WPF
host) · Rust owns settings/look/ledger persistence (rusqlite, transactional);
localStorage holds only ephemeral UI state · window close destroys the WebView (verified
child-process exit); resident mode runs windowless.

## Consequences

- STATE.md §F8 and spec 00's release train are replaced by migration phases M0–M8
  (`docs/plans/2026-07-10-tauri-migration.md`).
- ADR-0015's oracle table shifts: the FROZEN TS compositor becomes the primary visual
  oracle; the frozen C# TileRenderer remains an oracle only for the legacy style subset.
- Spec 06 §1/§2/§7, spec 01 §System Architecture, and spec 05 are updated/rewritten.
- Background-resident v1 scope: ADR-0020 + spec 07. Arrow semantics: ADR-0021.
- Panel record with owner dispositions:
  `docs/reviews/2026-07-10-tauri-rust-migration-panel.md` (includes the chief-architect
  seat's round-2 minority preference for shipping once on .NET first, and why it was
  outpriced).

## Amendment 1 (2026-07-10, owner) — layout overruled; Mac-first execution

Supersedes §3's "No `legacy/` graveyard" wording and §4's `apps/desktop/**` nesting
(the §4 diagram above is edited in place to match).

1. **Community-standard Tauri layout, single app.** There is exactly one product — a
   Windows desktop app — so the `apps/` nesting is dropped. The web app is hoisted to the
   repo root (`src/`, `public/`, `tests/`, `index.html`, one root `package.json`), the
   Tauri composition root is `src-tauri/` at the root, and the shared Rust logic stays in
   `crates/`.
2. **`legacy/` containment.** The frozen .NET oracle is quarantined under `legacy/`
   (self-contained — `cd legacy && dotnet build` — so C# relative project refs stay
   intact) until the M8 deletion; nothing .NET-related lives outside it. This overrides
   §3's "no `legacy/` graveyard" line only in *where the interim home is*; the intent
   (delete in one `last-dotnet` commit at M8, git as the long-term archive) is unchanged.
   The future elevated helper is a workspace crate `crates/dm-elevated` (a
   `requireAdministrator` bin crate), not a root directory.
3. **Mac-first execution (owner order).** Everything verifiable on a Mac gets built and
   verified on a Mac: the Tauri UI, the full Rust icon core vs the frozen TS oracle corpus,
   and the WASM preview. Windows platform code (`dm-windows`, the M3/M4 apply / restore /
   shell work) is **blind-written behind `cfg(windows)`** and kept compiling via
   `cargo check --target x86_64-pc-windows-msvc` (add the target with `rustup target add`;
   `check` needs no linker). ALL Windows runtime verification batches until the owner is at
   his Windows box; the M1 spikes 1/2/5 remain the entry gate for that batch, and M2's
   "runs on Windows" exit folds into it.
