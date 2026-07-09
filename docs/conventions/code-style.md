# Code Style

DeskMakeover follows the owner standards from `ai-command-center` with project-local
emphasis. Two stacks live here: the **web UI** (React 19 + TypeScript + Tailwind 4 +
Motion, Bun-only) and the **C# engine/host** (WPF shell + Win32 adapters).

## Both stacks

- Keep product code modular and cohesive. A file heading toward **500 lines** must be
  split before it becomes hard to review.
- User-facing strings come from the localization pipeline — the **resx is the source,
  the TS dictionaries are GENERATED** (`docs/development.md` §6.5); zh-Hans + English
  required. Mac-authored strings carry `// PENDING-RESX` until the Windows sweep.
- User-facing copy avoids system-cleaner language, fear tactics, and unexplained
  technical jargon; **no dashes** in any user-facing string (ADR-0013 copy law).
  Banned words in UI strings: 应用计划, dry-run, 注册表, 缓存, HKLM, journal, and any
  enum identifier (「正在扫描桌面…」/「还原快照」 are approved product copy).
  *Owner exception (2026-07-10): the welcome-gate ritual + arrow penance copy is
  authored verbatim by the owner and exempt — do not soften it (spec 01 §Identity).*
- Core logic, rendering decisions, transaction journals, and restore behavior require
  tests; **a bug fix ships a regression test** reproducing the failure.
- Dangerous operations must be explicit, reversible, and represented in the operation
  plan before execution; bake/apply are user-clicked only, never auto-triggered.
- Prefer clear names over comments. Comment only where behavior is non-obvious
  (Windows Shell quirks, motion/projection traps, algorithm rationale) — and when a
  hard-won lesson is reverted-prone, leave a ⛔ comment PLUS a regression test.

## Web (TypeScript / React)

- **Bun only** — install/test/build/scripts; never npm/node. Typecheck with
  `./node_modules/.bin/tsc -b` (a bare `bun x tsc` can false-green).
- The bridge contract lives ONCE per side: `src/bridge/types.ts` (schema constant
  asserted at startup) mirrored by `Host/Bridge/Contracts.cs`. Never widen a DTO
  without bumping the schema and checking strict decoders on both sides.
- Design tokens only — colours come from `@theme` tokens; raw blue/violet hexes,
  Tailwind blue-family and stock cool-gray utilities are TEST-BANNED
  (`tests/banned-colors.test.ts`; reviewed exemptions live inside the test).
- Rendering truth: `icon-compositor/` + `compositor/` are the WYSIWYG engines —
  preview and bake share the same functions; the chip swatch clips with the same
  authoring (`lib/shape-paths.ts` ← `icon-compositor/shapes.ts`). Never fork a
  "close enough" copy of engine math into a component.
- Motion: prefer the shared tokens in `lib/motion.ts`; every animation degrades
  under `useReducedMotion`. Sliding-highlight patterns use a single
  container-level element translated in list/track space — **never a per-row
  `layoutId` projection inside `overflow-hidden` rows** (segmented thumb +
  zone-list wash lessons; regression-tested).
- Zustand stores: selectors return stable refs; modules stay mounted
  (visibility-hidden) across switches; continuous inputs coalesce history steps.

## C# (engine / host)

- Domain types live away from Win32 and Shell interop; Shell code belongs behind
  explicit adapters.
- Domain enums never bind directly to UI; presentation mappers translate them.
- The frozen renderers (`TileRenderer` oracle et al.) carry banner comments — no
  new styles land in C# (ADR-0015 freeze); new styles are TS-first.
- The ElevatedHelper stays a standalone self-contained exe (privilege boundary —
  never share the runtime to save space).
- (WPF-era rules — squircle controls for visible corners, XAML binding bans —
  apply only to the remaining native chrome; the visible UI is the web app.)
