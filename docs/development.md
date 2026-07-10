# DeskMakeover — Development & Build Runbook

How to develop, test, and package DeskMakeover from a cold start — plus the
non-obvious gotchas that will bite you. Read `STATE.md` first for *what* is in flight;
this doc is *how* to work the code.

Architecture context: **ADR-0011** (UI is a WebView2 + React web app; the engine stays
C#), **ADR-0013** (v3 "Premium Flat" visual language, light-first, bundled fonts —
supersedes ADR-0012's chrome decisions), **spec 05** (web-shell / bridge),
**spec 02 v3** (visual language). The old `references/prototype/*.html` is a
historical reference only — the specs are the source of truth.

> ⚠️ **Architecture note (2026-07-10 — read before trusting the passages below).** The
> pixel-production model has INVERTED since these were written: icons are now rendered by a
> **CPU TypeScript compositor + Worker** and wallpaper by **Pixi**, both IN THE WEB. C# keeps
> only window / source-image decode / ICO packaging / shell write / backup-restore. There is
> no live SharedBuffer frame stream. **Native Mode B/C (host-driven) is NOT wired yet** — the
> host is bridge **schema 1** while the web is **schema 3**, so only Mode A (browser + mock)
> runs today. Any line below describing C#-produced pixels, a shared-buffer channel, or
> host-rendered previews is STALE pending the Spec 05 rewrite — authoritative picture is
> `STATE.md` §Bridge state + §Known doc drift.

---

## 0. Quick commands

The web half runs on **any OS** (macOS included); the C# half needs **Windows**.
See §3 for the three dev modes and when to use which.

```bash
# --- web (React SPA) — Bun only, never npm/node — ANY OS, from the repo root ---
bun install                 # first time
bun run dev                 # Vite dev server (default :5173, auto-increments) + mock bridge
bun run build               # production web bundle -> dist/
bun test                    # web unit tests (currently 297, incl. banned-colour scan)
bun run lint                # oxlint
```

```bash
# --- desktop shell (Tauri 2 + Rust) — macOS-buildable, from the repo root (ADR-0019 M2) ---
bun install                 # first time — installs @tauri-apps/cli here
bun run tauri:dev           # launch the REAL app (mock desktop) on macOS — see §3.5
bun run gen:bindings        # regenerate src/bridge/generated.ts from the Rust commands
bun run check:bindings      # drift guard — fails if generated.ts is stale (CI gate)
```
```bash
# --- Rust workspace — repo root, needs ~/.cargo/bin on PATH ---
cargo check --workspace                        # compile gate (all dm-* crates + the Tauri host)
cargo test -p dm-contracts -p dm-operations    # contract + settings-store units
```

```powershell
# --- FROZEN .NET oracle (legacy/, ADR-0019) — WINDOWS ONLY, repo-local SDK (see §1) ---
cd legacy
.\.dotnet\dotnet.exe build
.\.dotnet\dotnet.exe test   # engine + host + E2E (frozen oracle — see §4)

# --- .NET packaging (frozen; superseded by Tauri M8) — ⚠️ UNVERIFIED (see §5) ---
pwsh scripts\dev\publish.ps1        # App-only publish (no ElevatedHelper, no web build) — see §5
bun  scripts\publish-win.mjs        # DEV/local folder (uncompressed) — see §5
```

---

## 1. Prerequisites

| Tool | Version | Notes |
|------|---------|-------|
| **Bun** | ≥ 1.3 | The ONLY JS toolchain — install/build/test/bundle. **No Node, no npm** (owner rule, ADR-0011). |
| **.NET SDK** | **10.0.1xx+** (repo-local) | See the SDK gotcha below — this is the #1 trap. |
| **PowerShell 7** (`pwsh`) | any | The release script is `.ps1`. |
| **WebView2 Runtime** | Evergreen | Preinstalled on Win11, auto-updated on Win10. **Not bundled** — do not ship it. |

### ⚠️ The .NET SDK gotcha (read this or lose an hour)

The whole .NET tree now lives under `legacy/` (ADR-0019 Amendment 1) — run every `dotnet`
command from `legacy/`. `legacy/global.json` pins **`10.0.100` with `rollForward:
latestFeature`**, and the SDK is **repo-local at `legacy/.dotnet/`** (gitignored). The
`dotnet` on your PATH is the *Program Files* muxer, which on this machine has **only the
.NET 8 runtime, no SDK** — so a bare `dotnet build` / `dotnet test` fails with *"No .NET
SDKs were found. Requested 10.0.100."* even though the project builds fine.

**Always invoke the repo-local SDK** (from `legacy/`), or a script that auto-resolves it:
- `.\.dotnet\dotnet.exe <cmd>` directly, **or**
- `legacy/scripts/dev/publish.ps1` and `legacy/scripts/publish-win.mjs` both prefer
  `legacy/.dotnet` automatically (the `.mjs` falls back to PATH `dotnet` if `.dotnet` is absent).

**If `legacy/.dotnet/` is missing** (fresh clone / wiped state), reinstall it — user-local, no
admin:
```powershell
cd legacy
iwr https://dot.net/v1/dotnet-install.ps1 -OutFile $env:TEMP\dotnet-install.ps1
& $env:TEMP\dotnet-install.ps1 -Channel 10.0 -InstallDir "$PWD\.dotnet"
.\.dotnet\dotnet.exe --list-sdks   # expect 10.0.3xx
```
(Do NOT diagnose "no SDK on this machine" by running the PATH `dotnet` — check
`legacy/.dotnet` first.)

---

## 2. Repository layout

```
DeskMakeover/
├─ src/                             # the visible UI — React 19 + Tailwind 4 + shadcn + Motion (Bun); the web app the Tauri shell hosts (ADR-0019 + Amendment 1)
├─ public/                          # THE asset truth root: fonts, app icon, arrow badge, real-icons/ SSoT (gitignored)
├─ index.html · vite.config.ts · tsconfig* · package.json · bun.lock   # web app root config
├─ src-tauri/                       # Tauri 2 + Rust composition root (ADR-0019 M2): window/tray/commands/capabilities/CSP
├─ crates/                          # Rust workspace (dm-* libs + xtask automation): the port of the C# engine per ADR-0019
├─ scripts/                         # web dev tooling: mock-icon gen (dev/), oracle capture, spike4 slice
├─ tests/                           # bun unit tests + tests/icon-parity (Rust/TS parity harness)
├─ testdata/icons/                  # frozen-TS parity oracle corpus (ADR-0019 M0b)
├─ docs/                            # specs / decisions (ADR) / plans / STATE / this runbook
├─ Cargo.toml · rust-toolchain.toml · deny.toml
└─ legacy/                          # FROZEN .NET oracle (ADR-0019) — quarantined; deleted in one commit at M8
   ├─ DeskMakeover.slnx · Directory.Build.props · global.json
   ├─ src/                          # 6 C# projects (Core / IconRendering / Shell / Operations / ElevatedHelper / App-WPF)
   ├─ tests/                        # 6 .NET test projects (Core/Operations/Shell/IconRendering/App/E2E)
   └─ scripts/                      # .NET publish + WPF capture + resx-i18n tooling
```

The visible UI is **web** (`src/`); it is hosted by the **Tauri 2 + Rust** shell
(`src-tauri/` + the `dm-*` crates), which is replacing the old .NET/WPF engine + WebView2
host per **ADR-0019** (+ Amendment 1: the app is un-nested to the repo root, the .NET tree
quarantined under `legacy/`). The whole .NET tree now lives, frozen, under `legacy/` as an
executable oracle for the Rust port (deleted at M8); nothing outside `legacy/` is .NET.
**WYSIWYG law** still holds: the web displays engine-produced pixels 1:1 and must never
paint visual state the bake cannot reproduce. The §3.2/§3.3 (native WebView2 host) and §5
(.NET packaging) passages below describe that frozen `legacy/` stack; the live loop is
Mode A (§3.1) and the Tauri loop (§3.5).

---

## 3. Develop — three modes, pick per machine and task

| Mode | OS | What runs | Use for |
|------|----|-----------|---------|
| **A. Browser + mock** | any (macOS/Windows/Linux) | `bun run dev` → plain browser, mock bridge | ALL UI/design iteration. The default loop. |
| **B. Native host + HMR** | Windows | Vite dev server + `DeskMakeover.App` (Debug) pointed at it | Real engine + hot-reload UI: bridge work, engine-rendered pixels, IME/DPI feel. |
| **C. Native host + built assets** | Windows | `bun run build` → host serves `<out>/web` | Release-like smoke before packaging. |

Any AI/tooling session can therefore develop the UI from a Mac (mode A) and the same
branch continues on Windows (mode B/C) with zero code changes — the mode is chosen by
how you LAUNCH, not by build flags.

> ⚠️ **Modes B/C are NOT usable today.** The host is bridge schema 1; the web is schema 3,
> and the web already calls RPCs (`wallpaper.applyBaked`, chunked `icons.applyBaked*`) the
> host does not expose. Wiring the host to schema 3 is **F8** (see STATE.md §Bridge state).
> Until then, **Mode A (browser + mock) is the only working loop.**

### 3.1 Mode A — browser + mock (any OS)

```bash
bun run dev     # http://localhost:5173 (auto-increments) — from the repo root
```
Uses the **mock bridge** (`src/bridge/mock.ts` + `src/bridge/mock-desktop.ts`) — NO C#
host needed. **The mock renders the FULL app** (since v3): a fake desktop (wallpaper +
the REAL icon pack from `public/real-icons/`, config-reactive restyles, per-style
mark previews), both mirrors, all panels, settings, the welcome gate. The web now
renders the real preview + bake pixels itself (CPU TS icons, Pixi wallpaper) — the mock
only supplies the DATA (grid/items/source URLs); every interaction, layout, and motion
is exercisable.

- `?debug=components` — the component gallery (primitives in dark + light).
- First-run flows replay via the **DEV menu** (flask icon, title bar; DEV builds
  only): per-key resets for `dm.welcome.done` / `dm.consent.icons` / `dm.changelog.seen`,
  plus clear-all-and-reload. (The `dm.wow.icons` first-screen reveal was removed.)
- Design tokens live in `src/index.css` (`@theme` — type ladder, hairlines, washes).
  Accent is coral `#FF6F5E` **only**; blue/violet AND stock cool-gray utilities are
  banned and test-gated (§4). Light theme is the design first-citizen (ADR-0013).

### 3.2 Mode B — native host + HMR (Windows)

```powershell
bun run dev                                         # terminal 1: Vite (note the port), from root
$env:DESKMAKEOVER_DEV_SERVER = "http://localhost:5173"
cd legacy ; .\.dotnet\dotnet.exe run --project src\DeskMakeover.App   # terminal 2 (Debug build)
```
`DESKMAKEOVER_DEV_SERVER` is a **DEBUG-only** affordance (`Host/WebShellWindow.cs`) —
a Release binary ignores it by construction. The host adds the dev origin to its
trusted-origins list and navigates the WebView at it: hot-reload UI against the real
engine, real bridge, real shared-buffer frames.

- `DESKMAKEOVER_WEBVIEW_DATA=<dir>` isolates the WebView user-data profile (needed to
  run a dev instance beside an installed copy — the profile folder is an exclusive lock).
- Browser dev on Windows works too — mode A is OS-agnostic; use it whenever the task
  doesn't need engine pixels.

### 3.3 Mode C — native host + built assets (Windows)

```powershell
bun run build                                             # from the repo root
cd legacy ; .\.dotnet\dotnet.exe run --project src\DeskMakeover.App    # no env var → serves <out>/web
```

### 3.4 Bun-only

`bun` for everything JS. E2E uses a raw CDP client (no Playwright, no Node). If you find
yourself typing `npm`/`node`, stop.

### 3.5 Mode D — Tauri host (the ADR-0019 replatform loop, macOS-buildable)

The Tauri 2 shell (`src-tauri`) hosts the SAME React app as Mode A. It is
the successor to the WebView2/WPF host (Modes B/C) and, unlike them, builds and runs on
**macOS** today. As of M2 only settings persistence + the frameless titlebar's window
controls are wired to Rust; every other bridge verb still hits the mock, so the desktop
window shows the full mock desktop exactly like the browser loop.

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

The bridge DTOs for this slice are Rust types in `crates/dm-contracts` (`SettingsDto`,
`SettingsPatch`, `DiagnosticsPing`). `tauri-specta` turns the `#[specta::specta]` command
surface into `src/bridge/generated.ts` — one command list feeds BOTH the runtime
`invoke` handler and the TS export, so arg/return/error shapes cannot drift (the reason
ts-rs was not chosen: it would need a hand-kept parallel command wrapper — the schema-1/4
split ADR-0019 bans).

```bash
# from the repo root
bun run gen:bindings        # writes generated.ts (do NOT hand-edit that file)
bun run check:bindings      # cargo test that fails if generated.ts is out of date
```

The rest of bridge schema 4 still lives in `src/bridge/types.ts`; later phases
migrate it into `dm-contracts`.

---

## 4. Test & gates

| Suite | Command | Count |
|-------|---------|-------|
| Web unit | `bun test` (from the repo root) | 359 |
| .NET oracle (legacy) | `cd legacy ; .\.dotnet\dotnet.exe test` (Windows) | 277 (frozen; pre-v3) |
| Web lint | `bun run lint` (oxlint) | — |

- **Banned-colour gate**: `tests/banned-colors.test.ts` walks every shipped web source
  file and fails on blue/violet hexes, Tailwind blue-family AND stock cool-gray
  utilities. Reviewed exemptions live IN the test: the OS-authentic hex list
  (Windows-arrow blue `#0067C0`, taskbar chips) and the decorative `taskbar-strip.tsx`.
- **Copy law**: no dashes (`—`) in any user-facing string value (owner decree; reads
  as AI text). Grep `'": "[^"]*—'` over `src/lib/i18n/*.ts` before shipping strings.
- **File size**: every source file ≤ 500 lines (split, don't grow).
- **E2E** (`legacy/tests/DeskMakeover.E2E`) is opt-in: `DESKMAKEOVER_E2E=1`; it drives the real
  exe with `DESKMAKEOVER_FAKE_APPLY=1` so the bake/apply are stubbed (never real — §6).
  Plain `dotnet test` stays hermetic.
- Core logic / rendering / restore behaviour require tests; a **bug fix ships a regression
  test** (owner standard + dev-cycle Phase 9).

---

## 5. Build / Package  ← the biggest gotcha lives here

> ⚠️ **Packaging is UNVERIFIED — there is no proven shippable artifact yet.** `publish.ps1`
> publishes the **App only** (no `ElevatedHelper.exe` — the one-click bake path would fail for
> end users), and it does **not** build the web first even though the App carries an adjacent
> `web/`. A bare exe is missing web + helper; the whole folder is not a single file. Nothing has
> shipped (version `0.0.0`). Treat the "single shippable exe" story as an OPEN item until proven
> on a real host (F8). The two scripts below still differ in output, but neither is release-ready.

**There are TWO publish scripts. Do not confuse them.**

| Script | Output | Size | Purpose |
|--------|--------|------|---------|
| **`legacy/scripts/dev/publish.ps1`** | `legacy/publish/DeskMakeover-v<ver>-win-x64/DeskMakeover.App.exe` | ~64 MB, single-file, compressed | Intended shippable target, but **App-only** (no helper, no web build) — packaging UNVERIFIED (see banner). |
| `legacy/scripts/publish-win.mjs` | `legacy/artifacts/win-x64/DeskMakeover/` | uncompressed folder | DEV/local smoke. **Not** the shipping size. Also publishes the ElevatedHelper as a separate self-contained single-file. |

```powershell
# ship (run from legacy/):
pwsh legacy\scripts\dev\publish.ps1
#   -> legacy\publish\DeskMakeover-v0.0.0-win-x64\DeskMakeover.App.exe  (~64 MB, no .NET install)

# local smoke (uncompressed, faster to iterate on the host):
bun legacy\scripts\publish-win.mjs
#   -> legacy\artifacts\win-x64\DeskMakeover\DeskMakeover.App.exe  (+ helper\)
```

Notes:
- **Build the web first.** Both scripts run `bun run build` (the `.mjs`) or expect
  `dist/**` to exist so the App carries `<out>/web`. If `web/` is missing from the
  output, you skipped the web build.
- **Version** = `legacy/Directory.Build.props` `<Version>` (currently `0.0.0`, pre-release).
  The in-app version narrative is RESTORED (ADR-0013 amendment): the About identity
  card shows the version and opens the changelog; the changelog auto-opens exactly
  once per UPDATE (never on first install — `shouldAutoShowChangelog`). Move off
  `0.0.0` when the owner names the first release (F8).
- **Self-contained WPF floor ≈ 60–150 MB.** WPF is **not trimmable**, so you cannot go
  lower and keep "no .NET install". The single-file+compressed release (~64 MB) is near
  the floor; the uncompressed dev folder (214 MB) is the same bits, just not packed.
- **Stray files to trim from the release**: the current single-file output includes two
  WebView2 `.xml` doc files (~0.7 MB) that need not ship — a minor cleanup.

---

## 6. 注意事项 (caveats — the hard-won ones)

1. **Repo-local `legacy/.dotnet` SDK** (§1) — the PATH `dotnet` is runtime-only; from
   `legacy/` always use `.\.dotnet\dotnet.exe` or the scripts. Never conclude "the machine
   has no SDK" from the PATH muxer.

2. **The ElevatedHelper is a SECURITY boundary — never "share the runtime to save space".**
   It runs `requireAdministrator`, and the app runs from user-writable `%LOCALAPPDATA%\
   DeskMakeover`. An elevated process loading its runtime from a user-writable folder is a
   DLL-hijack **privilege-escalation** vector. That is exactly why the helper is a
   *standalone self-contained single-file* exe (it does not load code from the shared app
   folder). The ~70 MB is buying that boundary — do not collapse it. Safe slimming is
   single-file compression, not runtime-sharing.
   - **Open item (release blocker)**: `legacy/scripts/dev/publish.ps1`'s output is app-only (no
     `ElevatedHelper.exe`); the release must deliver/embed the helper before shipping or the
     one-click bake path fails for end users. Nothing has shipped yet — this is unproven, verify on a real host.

3. **WYSIWYG law** (spec 05 §3, ADR-0011): preview pixels **==** baked pixels. Web CSS
   scaling is viewport-fit only; never paint UI state the bake cannot reproduce. Zone-title
   rasterisation stays host-side so baked text keeps the same pen.

4. **Owner-supervised gates — NEVER auto-trigger** the real desktop icon-bake or the
   wallpaper-apply. They are human-click-only by design (spec 01 Safety, ADR-0011 §7).
   Automation/E2E must stub them (`DESKMAKEOVER_FAKE_APPLY=1`). The live-run checklist is
   `docs/verification/owner-supervised-live-runs.md`.

5. **i18n: the TS dictionaries ARE the source.** `src/lib/i18n/{en,zh-hans}.ts`
   are hand-edited directly (ADR-0019 adopted default: the resx pipeline is retired
   with the .NET tree; `scripts/resx-to-i18n.ts` deleted 2026-07-11). The frozen
   resx files under `legacy/` serve the C# oracle only and receive no new keys.

6. **Coral-only accent** `#FF6F5E`; blue/violet permanently banned; **light-first,
   theme follows the system** (ADR-0013 D3 — supersedes the old dark-default rule).
   Test-gated (§4).

7. **WebView2 is Evergreen** — do not bundle the browser runtime (that would add ~150 MB).
   Win11 has it; Win10 auto-updates. A bootstrapper note for edge cases is deferred to
   release (ADR-0011). Consumer-machine hardening lives in TWO layers:
   `src/lib/webview-hardening.ts` (web-side: drop-navigation guard,
   Ctrl+wheel page-zoom guard, host-only context-menu/accelerator suppression — also
   protects the browser dev loop) and the host settings audited against
   `docs/references/webview2-pitfalls.md` §补丁清单 (F8).

8. **File ≤ 500 lines · squircle controls for all visible corners · localized strings for
   all user-facing copy · no system-cleaner/fear language** (`docs/conventions/code-style.md`).

---

## 7. Where things live

| Need | Path |
|------|------|
| What's in flight / next / blockers | `docs/STATE.md` (**start here**) |
| Completed-work archive (swept from STATE) | `docs/journal/` |
| Architecture decisions | `docs/decisions/` (ADR-0011 replatform · ADR-0013 v3 redesign are current) |
| Capability specs | `docs/specs/` (02 visual language · 03 shell+settings · 04 wallpaper · 05 web-shell) |
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
