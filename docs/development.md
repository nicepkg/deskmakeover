# DeskMakeover — Development & Build Runbook

How to develop, test, and package DeskMakeover from a cold start — plus the
non-obvious gotchas that will bite you. Read `STATE.md` first for *what* is in flight;
this doc is *how* to work the code.

Architecture context: **ADR-0011** (UI is a WebView2 + React web app; the engine stays
C#), **ADR-0012** (v2 "Quiet Material" visual language), **spec 05** (web-shell /
bridge), **spec 02 v2** (visual language). The old `references/prototype/*.html` is a
historical reference only — the specs are the source of truth (ADR-0012).

---

## 0. Quick commands (from the repo root `D:\codes\deskmakeover`)

```powershell
# --- web (React SPA) — Bun only, never npm/node ---
cd src\DeskMakeover.Web
bun install                 # first time
bun run dev                 # dev server (mock bridge — see §3.1 for what renders)
bun run build               # production web bundle -> dist/
bun test                    # web unit tests (currently 136)
bun run lint                # oxlint

# --- C# engine/host — MUST use the repo-local SDK (see §1) ---
.\.dotnet\dotnet.exe build
.\.dotnet\dotnet.exe test   # engine + host + E2E (currently 277)

# --- package ---
pwsh scripts\dev\publish.ps1        # THE SHIPPABLE release (single-file ~64 MB) — see §5
bun  scripts\publish-win.mjs        # DEV/local folder (214 MB, NOT for shipping) — see §5
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

`global.json` pins **`10.0.100` with `rollForward: latestFeature`**, and the SDK is
**repo-local at `.dotnet/`** (gitignored). The `dotnet` on your PATH is the *Program
Files* muxer, which on this machine has **only the .NET 8 runtime, no SDK** — so a
bare `dotnet build` / `dotnet test` fails with *"No .NET SDKs were found. Requested
10.0.100."* even though the project builds fine.

**Always invoke the repo-local SDK**, or a script that auto-resolves it:
- `.\.dotnet\dotnet.exe <cmd>` directly, **or**
- `scripts/dev/publish.ps1` and `scripts/publish-win.mjs` both prefer `repo/.dotnet`
  automatically (the `.mjs` falls back to PATH `dotnet` if `.dotnet` is absent).

**If `.dotnet/` is missing** (fresh clone / wiped state), reinstall it — user-local, no
admin:
```powershell
iwr https://dot.net/v1/dotnet-install.ps1 -OutFile $env:TEMP\dotnet-install.ps1
& $env:TEMP\dotnet-install.ps1 -Channel 10.0 -InstallDir "D:\codes\deskmakeover\.dotnet"
.\.dotnet\dotnet.exe --list-sdks   # expect 10.0.3xx
```
(Do NOT diagnose "no SDK on this machine" by running the PATH `dotnet` — check
`repo/.dotnet` first.)

---

## 2. Repository layout

```
DeskMakeover/
├─ src/
│  ├─ DeskMakeover.Core/            # domain types (StyleConfig, WallpaperConfig, …) — no Win32
│  ├─ DeskMakeover.IconRendering/   # pure render: shapes, colour math, marks, WallpaperBakeRenderer (WYSIWYG)
│  ├─ DeskMakeover.Shell/           # Win32/Shell interop behind adapters (IFolderView, IDesktopWallpaper, …)
│  ├─ DeskMakeover.Operations/      # journaled ops, snapshot/restore
│  ├─ DeskMakeover.ElevatedHelper/  # the requireAdministrator helper (icon bake) — a SECURITY boundary (§6)
│  ├─ DeskMakeover.App/             # the WPF host: WebView2 window + JSON-RPC bridge + RPC controllers (Host/)
│  └─ DeskMakeover.Web/             # the visible UI — React 19 + Tailwind 4 + shadcn + Motion (Bun)
├─ tests/                           # .NET test projects (Core/Operations/Shell/IconRendering/App/E2E)
├─ scripts/
│  ├─ dev/publish.ps1               # SHIPPABLE single-file release
│  ├─ dev/upsert-strings.py         # add UI strings into the resx source (see §6 i18n)
│  └─ publish-win.mjs               # DEV/local uncompressed publish
├─ docs/                            # specs / decisions (ADR) / plans / STATE / this runbook
├─ .dotnet/                         # repo-local .NET SDK (gitignored — see §1)
├─ artifacts/  publish/             # build outputs (gitignored)
└─ Directory.Build.props            # <Version> + warnings-as-errors for all C# projects
```

The visible UI is **web**; the engine is **C#**. They talk over a JSON-RPC bridge
(`WebMessageReceived`) plus a shared-buffer channel for preview pixels (spec 05).
**WYSIWYG law**: the web displays engine-produced pixels 1:1 — it must never paint
visual state the bake cannot reproduce.

---

## 3. Develop

### 3.1 Web-only loop (fast UI iteration)

```powershell
cd src\DeskMakeover.Web ; bun run dev      # http://localhost:5173 (or next free port)
```
This uses the **mock bridge** (`src/bridge/mock.ts`) — a browser fallback with NO C#
host. **What renders:**
- ✅ the shell chrome + **Settings page** (mock provides `app.getInfo` / `settings`)
- ✅ **`http://localhost:<port>/?debug=components`** — the component gallery (every
  primitive, Card, chips-with-previews, the type ladder — in dark + light). Best surface
  for design-system work.
- ❌ the **图标 / 壁纸 canvases stay empty** — their tiles/frames come from the C# host
  over the shared buffer, which the mock does not emit. Use the full-app loop for those.

Design tokens live in `src/index.css` (`@theme` — type ladder `text-caption/body/
cardtitle/section/display`, `--elev-1/2`, `--glass`, `--canvas-stage`). Accent is coral
`#FF6F5E` **only** — blue/violet are banned and grep-gated (§4).

### 3.2 Full-app loop (real host + canvases)

Run `DeskMakeover.App` (VS/Rider, or `.\.dotnet\dotnet.exe run --project src\DeskMakeover.App`).
In a DEBUG build the host points the WebView at the Vite dev server (env-gated in
`Host/WebShellWindow.cs`), so you get hot-reload UI against the real engine. In Release
the host serves the bundled `<out>/web`.

- `DESKMAKEOVER_WEBVIEW_DATA=<dir>` isolates the WebView user-data profile (needed to run
  a dev instance beside your own installed copy — the profile folder is an exclusive lock).

### 3.3 Bun-only

`bun` for everything JS. E2E uses a raw CDP client (no Playwright, no Node). If you find
yourself typing `npm`/`node`, stop.

---

## 4. Test & gates

| Suite | Command | Count |
|-------|---------|-------|
| Web unit | `cd src\DeskMakeover.Web ; bun test` | 136 |
| C# engine + host + E2E | `.\.dotnet\dotnet.exe test` | 277 |
| Web lint | `bun run lint` (oxlint) | — |

- **Banned-colour gate**: `tests/banned-colors.test.ts` fails on blue/violet/cool-gray in
  app chrome. The decorative `taskbar-strip.tsx` (simulated Win11 OS chrome) is the one
  allowlisted file — keep app-glass vs OS-frost distinct.
- **File size**: every source file ≤ 500 lines (split, don't grow).
- **E2E** (`tests/DeskMakeover.E2E`) is opt-in: `DESKMAKEOVER_E2E=1`; it drives the real
  exe with `DESKMAKEOVER_FAKE_APPLY=1` so the bake/apply are stubbed (never real — §6).
  Plain `dotnet test` stays hermetic.
- Core logic / rendering / restore behaviour require tests; a **bug fix ships a regression
  test** (owner standard + dev-cycle Phase 9).

---

## 5. Build / Package  ← the biggest gotcha lives here

**There are TWO publish scripts. Do not confuse them.**

| Script | Output | Size | Purpose |
|--------|--------|------|---------|
| **`scripts/dev/publish.ps1`** | `publish/DeskMakeover-v<ver>-win-x64/DeskMakeover.App.exe` | **~64 MB, single-file, compressed** | **THE SHIPPABLE RELEASE.** Self-contained → end users install **nothing** (no .NET). This is what you distribute. |
| `scripts/publish-win.mjs` | `artifacts/win-x64/DeskMakeover/` (426 files) | 214 MB **uncompressed folder** | DEV/local smoke only. **Not** the shipping size. Also publishes the ElevatedHelper as a separate self-contained single-file. |

```powershell
# ship:
pwsh scripts\dev\publish.ps1
#   -> publish\DeskMakeover-v0.0.0-win-x64\DeskMakeover.App.exe  (~64 MB, no .NET install)

# local smoke (uncompressed, faster to iterate on the host):
bun scripts\publish-win.mjs
#   -> artifacts\win-x64\DeskMakeover\DeskMakeover.App.exe  (+ helper\)
```

Notes:
- **Build the web first.** Both scripts run `bun run build` (the `.mjs`) or expect
  `dist/**` to exist so the App carries `<out>/web`. If `web/` is missing from the
  output, you skipped the web build.
- **Version** = `Directory.Build.props` `<Version>` (currently `0.0.0`, pre-release —
  ADR-0012 removed the in-app version chip + changelog until a real release). It drives
  the release folder name (`-v<ver>-`) and the assembly version.
- **Self-contained WPF floor ≈ 60–150 MB.** WPF is **not trimmable**, so you cannot go
  lower and keep "no .NET install". The single-file+compressed release (~64 MB) is near
  the floor; the uncompressed dev folder (214 MB) is the same bits, just not packed.
- **Stray files to trim from the release**: the current single-file output includes two
  WebView2 `.xml` doc files (~0.7 MB) that need not ship — a minor cleanup.

---

## 6. 注意事项 (caveats — the hard-won ones)

1. **Repo-local `.dotnet` SDK** (§1) — the PATH `dotnet` is runtime-only; always use
   `.\.dotnet\dotnet.exe` or the scripts. Never conclude "the machine has no SDK" from
   the PATH muxer.

2. **The ElevatedHelper is a SECURITY boundary — never "share the runtime to save space".**
   It runs `requireAdministrator`, and the app runs from user-writable `%LOCALAPPDATA%\
   DeskMakeover`. An elevated process loading its runtime from a user-writable folder is a
   DLL-hijack **privilege-escalation** vector. That is exactly why the helper is a
   *standalone self-contained single-file* exe (it does not load code from the shared app
   folder). The ~70 MB is buying that boundary — do not collapse it. Safe slimming is
   single-file compression, not runtime-sharing.
   - **Open item**: the single-file release folder is app-only (no `ElevatedHelper.exe`);
     verify how the release delivers/embeds the helper before shipping, or the one-click
     bake path fails for end users. (v1.0.0 shipped the same way — confirm it, don't assume.)

3. **WYSIWYG law** (spec 05 §3, ADR-0011): preview pixels **==** baked pixels. Web CSS
   scaling is viewport-fit only; never paint UI state the bake cannot reproduce. Zone-title
   rasterisation stays host-side so baked text keeps the same pen.

4. **Owner-supervised gates — NEVER auto-trigger** the real desktop icon-bake or the
   wallpaper-apply. They are human-click-only by design (spec 01 Safety, ADR-0011 §7).
   Automation/E2E must stub them (`DESKMAKEOVER_FAKE_APPLY=1`). The live-run checklist is
   `docs/verification/owner-supervised-live-runs.md`.

5. **i18n: the resx is the SOURCE, the TS is GENERATED.** `src/DeskMakeover.Web/src/lib/
   i18n/{en,zh-hans}.ts` carry a *"GENERATED by scripts/resx-to-i18n.ts — do not edit by
   hand"* header; the source is `src/DeskMakeover.App/Resources/Strings*.resx` (the C#
   host reads them via `UiText.cs`). To add/change a string, run
   `python scripts/dev/upsert-strings.py <json>` (`{"en":{Key:val},"zh":{Key:val}}` — it
   writes both resx), then regenerate the TS. Hand-editing only the TS works until someone
   regenerates, then your keys vanish (compile error / missing strings).

6. **Coral-only accent** `#FF6F5E`; blue/violet permanently banned; **dark is the default
   theme**, light is a first-class flip. Grep-gated (§4).

7. **WebView2 is Evergreen** — do not bundle the browser runtime (that would add ~150 MB).
   Win11 has it; Win10 auto-updates. A bootstrapper note for edge cases is deferred to
   release (ADR-0011).

8. **File ≤ 500 lines · squircle controls for all visible corners · localized strings for
   all user-facing copy · no system-cleaner/fear language** (`docs/conventions/code-style.md`).

---

## 7. Where things live

| Need | Path |
|------|------|
| What's in flight / next / blockers | `docs/STATE.md` (**start here**) |
| Architecture decisions | `docs/decisions/` (ADR-0011 replatform · ADR-0012 redesign are current) |
| Capability specs | `docs/specs/` (02 visual language · 03 shell+settings · 04 wallpaper · 05 web-shell) |
| Point-in-time plans | `docs/plans/` |
| Design-panel reviews | `docs/reviews/` |
| Live owner-gated runs | `docs/verification/owner-supervised-live-runs.md` |
| Engineering conventions | `docs/conventions/code-style.md` |
