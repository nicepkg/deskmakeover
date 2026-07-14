# Code Style

DeskMakeover follows the owner's cross-project engineering standards with project-local
emphasis. Two stacks live here: the **web UI** (React 19 + TypeScript + Tailwind 4 +
Motion, Bun-only) and the **Rust engine/host** (Tauri 2 shell + `dm-*` crates + Win32/COM
adapters). The .NET/C# tree that served as the port's parity oracle was removed from the repo on
2026-07-14 (ADR-0019).

## Both stacks

- Keep product code modular and cohesive. A file heading toward **500 lines** must be
  split before it becomes hard to review.
- User-facing strings come from the TS localization dictionaries — **`src/lib/i18n/{en,zh-hans}.ts`
  is the SOURCE** (the resx pipeline was retired 2026-07-11, `docs/development.md` §6); zh-Hans +
  English required, kept in lockstep.
- User-facing copy avoids system-cleaner language, fear tactics, and unexplained
  technical jargon; **no dashes** in any user-facing string (ADR-0013 copy law).
  Banned words in UI strings: 应用计划, dry-run, 注册表, 缓存, HKLM, journal, and any
  enum identifier (「正在扫描桌面…」/「还原快照」 are approved product copy).
  *Owner exception (2026-07-10): the welcome-gate ritual + arrow penance copy is
  authored verbatim by the owner and exempt — do not soften it (spec 01 §Identity).*
- Core logic, rendering decisions, transaction journals, and restore behavior require
  tests; **a bug fix ships a regression test** reproducing the failure.
- Dangerous operations must be explicit, reversible, and represented in the operation
  plan before execution; foreground bake/apply/calm-writes are user-clicked, never auto-triggered
  (the one exception is the opt-in resident auto-format on NEW icons — spec 07 — which is consented,
  batched-propose + timeout, and always undoable).
- Prefer clear names over comments. Comment only where behavior is non-obvious
  (Windows Shell/COM quirks, motion/projection traps, algorithm rationale) — and when a
  hard-won lesson is reverted-prone, leave a ⛔ comment PLUS a regression test.

## Web (TypeScript / React)

- **Bun only** — install/test/build/scripts; never npm/node. Typecheck with
  `./node_modules/.bin/tsc -b` (a bare `bun x tsc` can false-green).
- The bridge contract is GENERATED from the Rust host: `dm-contracts` → tauri-specta →
  `src/bridge/generated.ts`; `src/bridge/types.ts` holds the hand-written DTOs + the
  `BRIDGE_SCHEMA_VERSION` constant asserted at startup. Never widen a DTO without bumping
  the schema and checking strict decoders on both sides; run `bun run check:bindings` (the
  drift guard) after any contract change.
- Design tokens only — colours come from `@theme` tokens; raw blue/violet hexes,
  Tailwind blue-family and stock cool-gray utilities are TEST-BANNED
  (`tests/banned-colors.test.ts`; reviewed exemptions live inside the test).
- Rendering truth: **icon pixels are produced by the Rust `dm-icon-core`** (compiled to WASM for
  the web preview/bake, native for the resident/background path) — the frozen TS `icon-compositor/` is the
  parity ORACLE, not a live path (never fork "close enough" engine math into a component). The
  Pixi wallpaper `compositor/` is the live web wallpaper engine; preview and bake share the same
  functions, and the chip swatch clips with the same authoring (`lib/shape-paths.ts`).
- Motion: prefer the shared tokens in `lib/motion.ts`; every animation degrades
  under `useReducedMotion`. Sliding-highlight patterns use a single
  container-level element translated in list/track space — **never a per-row
  `layoutId` projection inside `overflow-hidden` rows** (segmented thumb +
  zone-list wash lessons; regression-tested).
- Zustand stores: selectors return stable refs; modules stay mounted
  (visibility-hidden) across switches; continuous inputs coalesce history steps.

## Rust (engine / host)

- The workspace crates split by responsibility: `dm-domain` (pure ports/models),
  `dm-operations` (transaction engine + journals), `dm-windows` (Win32/COM/registry adapters),
  `dm-elevated` (privilege boundary), `dm-icon-core`/`dm-icon-codec` (pixel truth + ICO writer),
  `dm-contracts` (bridge DTOs). Domain types live away from Win32/Shell interop; platform code
  belongs behind explicit adapters/ports.
- Domain enums never bind directly to UI; the generated bridge DTOs are the presentation contract.
- Blind-written Windows platform bodies are kept compiling via
  `cargo check --target x86_64-pc-windows-msvc` and unit-tested through Mac fakes; every real
  COM/WIC/registry/shell call is `[WINDOWS-VERIFY]` until it runs on a real box.
- The ElevatedHelper (`dm-elevated`) stays a standalone self-contained exe (privilege boundary —
  never share the runtime to save space).
- The frozen TS `icon-compositor/` carries banner comments — it is a byte-parity reference; no new
  styles or features land in it (it exists to certify the Rust port, ADR-0015/0019). The C# oracle
  tree was removed from the repo on 2026-07-14.
