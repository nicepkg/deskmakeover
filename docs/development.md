# DeskMakeover — Development & Build Runbook

How to develop, test, and package DeskMakeover from a cold start — plus the
non-obvious gotchas that will bite you. Read `STATE.md` first for *what* is in flight;
this doc is *how* to work the code.

Architecture context: **ADR-0019** (the product is now **Tauri 2 + Rust**; the .NET/C# engine
has been fully retired and **removed from the repo** — 2026-07-14, ahead of the ADR-0019 M8
deletion), **ADR-0013** (v3 "Premium Flat" visual language,
light-first, bundled fonts — supersedes ADR-0012's chrome decisions), **spec 05** (Tauri/Rust
bridge), **spec 02 v3** (visual language). The old `references/prototype/*.html` is a
historical reference only — the specs are the source of truth.

> **Pixel-production model (current, ADR-0019).** Icon pixels come from the Rust `dm-icon-core`
> (WASM for the web preview + foreground bake; native for the resident/background path — foreground
> apply writes WASM-baked masters via the host); the frozen TS `icon-compositor/`
> is the byte-parity oracle. Wallpaper compositing is Pixi in the web. The Rust host decodes
> source images, packages ICO ladders, writes to the shell, and backs up/restores. The bridge
> contract is `dm-contracts` → tauri-specta → `src/bridge/generated.ts`, `BRIDGE_SCHEMA_VERSION = 8`
> (wallpaper schema 6, icons 7, calm 8 all wired on Mac-Tauri; Windows runtime is `[WINDOWS-VERIFY]`).
> The old C# WebView2/WPF host has been fully removed (2026-07-14) and will never be re-wired.

---

## 0. Quick commands

The web loop and the Tauri/Rust host both build on **any OS** (macOS included); only the
`[WINDOWS-VERIFY]` runtime pass needs **Windows**.
See §3 for the dev modes and when to use which.

```bash
# --- web (React SPA) — Bun only, never npm/node — ANY OS, from the repo root ---
bun install                 # first time
bun run dev                 # Vite dev server (default :5173, auto-increments) + mock bridge
bun run build               # production web bundle -> dist/
bun test                    # web unit tests (incl. banned-colour + copy-law gates)
bun run lint                # oxlint
```

```bash
# --- desktop shell (Tauri 2 + Rust) — macOS-buildable, from the repo root (ADR-0019 M2) ---
bun install                 # first time — installs @tauri-apps/cli here
bun run tauri:dev           # launch the REAL app (mock desktop) on macOS — see §3.3
bun run gen:bindings        # regenerate src/bridge/generated.ts from the Rust commands
bun run check:bindings      # drift guard — fails if generated.ts is stale (CI gate)
```
```bash
# --- Rust workspace — repo root, needs ~/.cargo/bin on PATH ---
cargo check --workspace                        # compile gate (all dm-* crates + the Tauri host)
cargo test -p dm-contracts -p dm-operations    # contract + settings-store units
```

---

## 1. Prerequisites

| Tool | Version | Notes |
|------|---------|-------|
| **Bun** | ≥ 1.3 | The ONLY JS toolchain — install/build/test/bundle. **No Node, no npm** (owner rule). |
| **Rust toolchain** (`rustc`/`cargo`) | per `rust-toolchain.toml` | The engine + the Tauri host. `cargo` on PATH (`~/.cargo/bin`). |
| **Tauri 2 prerequisites** | per platform | macOS: Xcode CLT + a WebKit webview (built-in). Windows: MSVC build tools + the `x86_64-pc-windows-msvc` target + WebView2. |
| **WebView2 Runtime** (Windows) | Evergreen | Preinstalled on Win11, auto-updated on Win10. **Not bundled** — do not ship it. |

> The .NET SDK / PowerShell prerequisites are gone: the C# engine was removed from the repo
> (ADR-0019, 2026-07-14). Nothing in the tree is .NET anymore.

---

## 2. Repository layout

```
DeskMakeover/
├─ src/                             # the visible UI — React 19 + Tailwind 4 + shadcn + Motion (Bun); the web app the Tauri shell hosts (ADR-0019 + Amendment 1)
├─ public/                          # THE asset truth root: fonts, app icon, arrow badge, real-icons/ SSoT (gitignored)
├─ index.html · vite.config.ts · tsconfig* · package.json · bun.lock   # web app root config
├─ src-tauri/                       # Tauri 2 + Rust composition root (ADR-0019 M2): window/tray/commands/capabilities/CSP
├─ crates/                          # Rust workspace (dm-* libs + xtask automation): the port of the retired C# engine per ADR-0019
├─ scripts/                         # web dev tooling: mock-icon gen (dev/), oracle capture, spike4 slice
├─ tests/                           # bun unit tests + tests/icon-parity (Rust/TS parity harness)
├─ testdata/icons/                  # frozen-TS parity oracle corpus (ADR-0019 M0b)
├─ docs/                            # specs / decisions (ADR) / plans / STATE / this runbook
└─ Cargo.toml · rust-toolchain.toml · deny.toml
```

The visible UI is **web** (`src/`); it is hosted by the **Tauri 2 + Rust** shell
(`src-tauri/` + the `dm-*` crates), which replaced the old .NET/WPF engine + WebView2
host per **ADR-0019** (+ Amendment 1: the app is un-nested to the repo root). The .NET
tree served as an executable parity oracle during the port and was **removed from the repo
on 2026-07-14** (ADR-0019's planned M8 deletion, done early); nothing in the tree is .NET.
**WYSIWYG law** still holds: the web displays engine-produced pixels 1:1 and must never
paint visual state the bake cannot reproduce. The live loop is Mode A (§3.1) and the Tauri
loop (§3.3).

---

## 3. Develop — the two live modes

| Mode | OS | What runs | Use for |
|------|----|-----------|---------|
| **A. Browser + mock** | any (macOS/Windows/Linux) | `bun run dev` → plain browser, mock bridge | ALL UI/design iteration. The default loop. |
| **D. Tauri host** | macOS-buildable (Windows at integration) | `bun run tauri:dev` → real Rust host, mock desktop | The real app loop: bridge verbs on real Rust, window/tray, settings persistence. §3.3. |

Develop the UI from a Mac in Mode A, then bring up the real host in Mode D (also
macOS-buildable) — the mode is chosen by how you LAUNCH, not by build flags. The old
C# WebView2 host was removed with the .NET tree (ADR-0019, 2026-07-14); F8 host wiring is void.

### 3.1 Mode A — browser + mock (any OS)

```bash
bun run dev     # http://localhost:5173 (auto-increments) — from the repo root
```
Uses the **mock bridge** (`src/bridge/mock.ts` + `src/bridge/mock-desktop.ts`) — NO Rust
host needed. **The mock renders the FULL app** (since v3): a fake desktop (wallpaper +
the REAL icon pack from `public/real-icons/`, config-reactive restyles, per-style
mark previews), both mirrors, all panels, settings, the welcome gate. The web
renders the real preview + bake pixels via the **WASM `dm-icon-core`** (icons) + Pixi
(wallpaper) — the mock only supplies the DATA (grid/items/source URLs); every interaction,
layout, and motion
is exercisable.

- `?debug=components` — the component gallery (primitives in dark + light).
- First-run flows replay via the **DEV menu** (flask icon, title bar; DEV builds
  only): per-key resets for `dm.welcome.done` / `dm.consent.icons` / `dm.changelog.seen`,
  plus clear-all-and-reload. (The `dm.wow.icons` first-screen reveal was removed.)
- Design tokens live in `src/index.css` (`@theme` — type ladder, hairlines, washes).
  Accent is coral `#FF6F5E` **only**; blue/violet AND stock cool-gray utilities are
  banned and test-gated (§4). Light theme is the design first-citizen (ADR-0013).

### 3.2 Bun-only

`bun` for everything JS. E2E uses a raw CDP client (no Playwright, no Node). If you find
yourself typing `npm`/`node`, stop.

### 3.3 Mode D — Tauri host (the ADR-0019 replatform loop, macOS-buildable)

The Tauri 2 shell (`src-tauri`) hosts the SAME React app as Mode A. It is
the successor to the removed WebView2/WPF host and, unlike it, builds and runs on
**macOS** today. Wallpaper (schema 6), icons (schema 7) and calm (schema 8) verbs now route
through real Rust on Mac-Tauri over a devhost/mock desktop; the remaining native paths (real
Windows shell/COM/registry) are `[WINDOWS-VERIFY]`. Settings persistence + the frameless
titlebar's window controls have been Rust-backed since M2.

```bash
# from the repo root (package.json + src-tauri live here)
bun install                 # once — @tauri-apps/cli
bun run tauri:dev           # compiles the Rust host, starts Vite (:5173), opens the window
```

- **`tauri:dev` runs from the repo root.** It merges `src-tauri/tauri.dev.conf.json` over
  the base config. That merge relaxes the CSP for Vite HMR (inline+eval scripts, a
  `ws://localhost` socket); the base `tauri.conf.json` keeps the **strict production CSP**
  (Tauri hashes the app's own inline scripts at build time, so prod needs no `unsafe-*`).
  Run bare `bun run tauri` (`tauri dev`/`tauri build`) to exercise the strict CSP.
- **First compile is slow** (~1–2 min, full Rust dep tree); later runs are cached.
- **Window**: frameless (`decorations:false`) to match the Win11-style web titlebar, min
  1024×700. The titlebar's minimize/maximize/close drive the real window; on macOS drag
  the window by the titlebar (`data-tauri-drag-region`). Position/size persist across runs
  (`tauri-plugin-window-state`); a second launch focuses the existing window
  (`tauri-plugin-single-instance`, registered first).
- **Settings persist for real** in rusqlite under the OS app-data dir
  (`~/Library/Application Support/com.xiaominglab.deskmakeover/settings.sqlite3` on macOS).
  Toggle theme/language in 设置 and relaunch — the choice sticks (mock resets each load).
- **`bun run dev` (Mode A) is unchanged** — the Tauri bridge only activates inside a Tauri
  WebView (`window.__TAURI_INTERNALS__`); a plain browser always uses the mock.

#### Contracts → TypeScript bindings (DRY law, ADR-0019)

The bridge DTOs are Rust types in `crates/dm-contracts` (settings, diagnostics, wallpaper,
icons, tweaks/calm). `tauri-specta` turns the `#[specta::specta]` command surface into
`src/bridge/generated.ts` — one command list feeds BOTH the runtime `invoke` handler and the
TS export, so arg/return/error shapes cannot drift (the reason ts-rs was not chosen: it would
need a hand-kept parallel command wrapper — the kind of two-source split ADR-0019 bans).

```bash
# from the repo root
bun run gen:bindings        # writes generated.ts (do NOT hand-edit that file)
bun run check:bindings      # cargo test that fails if generated.ts is out of date
```

Hand-written DTOs + the `BRIDGE_SCHEMA_VERSION` constant (currently **9**) live in
`src/bridge/types.ts`; the wallpaper/icons/calm/preset slices are already generated from `dm-contracts`.

---

## 4. Test & gates

| Suite | Command | Notes |
|-------|---------|-------|
| Web unit | `bun test` (from the repo root) | incl. banned-colour + copy-law gates |
| Rust workspace | `cargo test --workspace` + `cargo check --target x86_64-pc-windows-msvc` | the engine; msvc-check keeps blind Windows code compiling |
| Bindings drift | `bun run check:bindings` | fails if `generated.ts` is stale |
| Web lint | `bun run lint` (oxlint) | — |

(Run the commands for the current pass/fail count; test totals drift every few commits and are
not restated here.)

- **Banned-colour gate**: `tests/banned-colors.test.ts` walks every shipped web source
  file and fails on blue/violet hexes, Tailwind blue-family AND stock cool-gray
  utilities. Reviewed exemptions live IN the test: the OS-authentic hex list
  (Windows-arrow blue `#0067C0`, taskbar chips) and the decorative `taskbar-strip.tsx`.
- **Copy law**: no dashes (`—`) in any user-facing string value (owner decree; reads
  as AI text). Grep `'": "[^"]*—'` over `src/lib/i18n/*.ts` before shipping strings.
- **File size**: every source file ≤ 500 lines (split, don't grow).
- **Owner-gated apply is E2E-stubbed**: automation drives the app with
  `DESKMAKEOVER_FAKE_APPLY=1` so the bake/apply are stubbed (never real — §6).
- Core logic / rendering / restore behaviour require tests; a **bug fix ships a regression
  test** (owner standard + dev-cycle Phase 9).

---

## 5. Build / Package

> **The shipping artifact is the Tauri build (ADR-0019, M8).** `bun run tauri build` produces a
> working per-user NSIS installer (Windows) bundling the Rust host + web + the `dm-elevated` helper
> (requireAdministrator manifest + WebView2 downloadBootstrapper). Authenticode signing CI is
> validated end-to-end (Certum SimplySign + a self-hosted runner — `docs/signing-setup.md`), and
> GitHub-hosted CI (`.github/workflows/ci.yml`) gates every push/PR. The bundle version is `0.1.0`
> (`package.json` + `src-tauri/tauri.conf.json`; the Cargo workspace intentionally stays `0.0.0`,
> `publish=false`). **Still open:** updater, `.dmpreset` file association, the owner cutting the
> first `v*` tag, and the owner-supervised Windows write-surface verification (STATE.md). Nothing has
> been tagged/released yet, but the packaging + signing path is proven.

The old .NET/WPF publish path (single-file WPF exe, ElevatedHelper as a separate binary, the
`publish.ps1` / `publish-win.mjs` scripts) was removed with the C# tree on 2026-07-14. The
`dm-elevated` privilege boundary (§6.1) now ships as a Rust sidecar bundled by the Tauri build.

---

## 6. 注意事项 (caveats — the hard-won ones)

1. **The elevated helper (`dm-elevated.exe`, Rust) is a SECURITY boundary — never "share the
   runtime to save space".** It runs `requireAdministrator`, and the app runs from user-writable
   `%LOCALAPPDATA%\DeskMakeover`. An elevated process loading code from a user-writable folder is a
   DLL-hijack **privilege-escalation** vector — the helper must be a *standalone self-contained* exe
   that loads no code from the shared app folder. Do not collapse that boundary to save disk.
   - **Open item (M8 release)**: the Tauri build must ship `dm-elevated.exe` as a bundled sidecar
     (`externalBin`); if the release omits it, the one-click bake path fails for end users. Nothing
     has shipped yet — unproven, verify on a real Windows box.

2. **WYSIWYG law** (spec 05 §3, ADR-0019): preview pixels **==** baked pixels. For icons this is
   structural (the web preview and manual bake both run the WASM `dm-icon-core`; the native
   background renderer is the same core's native build, WASM↔native byte-parity); for wallpaper the
   Pixi compositor bakes the same canvas it previews. Web CSS scaling is viewport-fit only; never
   paint UI state the bake cannot reproduce.

3. **Owner-supervised gates — NEVER auto-trigger** the real desktop icon-bake or the
   wallpaper-apply. They are human-click-only by design (spec 01 Safety, ADR-0019).
   Automation/E2E must stub them (`DESKMAKEOVER_FAKE_APPLY=1`). The live-run checklist is
   `docs/verification/owner-supervised-live-runs.md`.

4. **i18n: the TS dictionaries ARE the source.** `src/lib/i18n/{en,zh-hans}.ts`
   are hand-edited directly (ADR-0019 adopted default: the resx pipeline was retired
   with the .NET tree; `scripts/resx-to-i18n.ts` deleted 2026-07-11).

5. **Coral-only accent** `#FF6F5E`; blue/violet permanently banned; **light-first,
   theme follows the system** (ADR-0013 D3 — supersedes the old dark-default rule).
   Test-gated (§4).

6. **WebView2 is Evergreen** — do not bundle the browser runtime (that would add ~150 MB).
   Win11 has it; Win10 auto-updates. A bootstrapper note for edge cases is deferred to
   release (M8). Consumer-machine hardening lives in TWO layers:
   `src/lib/webview-hardening.ts` (web-side: drop-navigation guard,
   Ctrl+wheel page-zoom guard, host-only context-menu/accelerator suppression — also
   protects the browser dev loop) and the host settings audited against
   `docs/references/webview2-pitfalls.md` §补丁清单 (`[WINDOWS-VERIFY]`).

7. **File ≤ 500 lines · squircle controls for all visible corners · localized strings for
   all user-facing copy · no system-cleaner/fear language** (`docs/conventions/code-style.md`).

---

## 7. Where things live

| Need | Path |
|------|------|
| What's in flight / next / blockers | `docs/STATE.md` (**start here**) |
| Completed-work archive (swept from STATE) | `docs/journal/` |
| Architecture decisions | `docs/decisions/` (ADR-0019 Tauri/Rust replatform · ADR-0013 Premium Flat · 0016-0023 govern; 0001/0011/0012 superseded) |
| Capability specs | `docs/specs/` (00 roadmap · 01 architecture · 02 visual · 03 shell+settings · 04 wallpaper · 05 bridge · 06 icons · 07 resident · 08 calm) |
| Point-in-time plans | `docs/plans/` |
| Design-panel reviews | `docs/reviews/` |
| Live owner-gated runs | `docs/verification/owner-supervised-live-runs.md` |
| Engineering conventions | `docs/conventions/code-style.md` |

## Real icon fixtures (dev only)

`public/real-icons/` is the SINGLE SOURCE OF TRUTH for genuine icon fixtures
(owner order 2026-07-11), subfoldered by type — `windows/` (native system),
`folders/`, `apps/` (third-party), `files/`, `wallpapers/`. Add an icon by
DROPPING it into the right subfolder, then run
`bun scripts/dev/fetch-real-icons.ts --scan` to rebuild `manifest.json`
(kind = subfolder, label = filename stem; refinements via `overrides.json`).
A full `bun scripts/dev/fetch-real-icons.ts` re-harvests from the two win11
simulator repos, MERGES (never deletes owner-added files) and drops the clone
cache afterwards. The directory is gitignored from this repo (extracted
Microsoft/brand assets — never shipped; the vite closeBundle hook strips it
from dist/) but is its OWN nested git repo: commit there after adding icons.
The mock desktop REQUIRES this pack — there is no synthetic fallback. The
synthetic pack lives on ONLY as committed parity fixtures at
`testdata/icons/source-pack/` (the oracle corpus is anchored to it).
