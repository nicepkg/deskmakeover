# Audit record: full-repo pass + fix run (2026-07-12 night)

**Date:** 2026-07-12 · **Scope:** full repo, git range `7f7e5c2..HEAD` (11 commits) ·
**Trigger:** owner-ordered general audit ("其他问题"), no specific bug report
**Gates at close:** `cargo test` workspace green · `cargo check --target
x86_64-pc-windows-msvc` (zero-C-dep crates) green · `tsc -b` green · `bun test` (516) green

## 1. Method

- **Codex ran first**, unsupervised, to grab low-hanging findings fast and produce a candidate
  list across the diff surface.
- **Lead ran the primary audit** in parallel, independent of Codex's list — reading the
  changed/suspect modules directly to catch what a fast automated pass misses.
- **3 breadth subagents** fanned out over crate boundaries a single pass under-covers:
  icon-core (`crates/dm-icon-core`), elevated + Windows apply (`crates/dm-elevated`,
  `crates/dm-windows`), shell/bridge (`src/bridge`, `src-tauri`).
- **Every candidate finding was verified a second time before fixing** — re-read the exact
  file + line, confirmed the failure mode with a reasoning trace or a reproduction test,
  THEN fixed. Findings that didn't survive re-verification are noted as declined/downgraded
  in §3, not silently dropped.

## 2. Findings ledger (11 commits, `7f7e5c2..HEAD`)

### P1 — user-facing breakage / permanent failure

| Commit | Finding | File(s) | Verification |
|---|---|---|---|
| `a339196` | Client bridge detected the retired WebView2 host via `window.chrome.webview` presence and routed every `call()` through its dead postMessage protocol. Tauri v2 on **Windows** renders via WebView2 too — its runtime injects `chrome.webview` on every page — so on the real ship target every bridge call hung forever (`app.getInfo`/`settings.get` never resolve, the app never boots). Invisible on Mac (WKWebView has no `chrome.webview`) and in the plain browser, which is why it survived this long. | `src/bridge/client.ts` | Re-verified against Tauri v2's documented Windows WebView2 injection behavior; confirmed `stores/app.ts`'s boot sequence blocks on exactly the two calls that would hang forever. |
| `de1617e` | `secure_dir`'s `DATA_DIR_SDDL` had no owner (`O:`) component. `dm-elevated.exe` is a `requireAdministrator` helper that runs as the **current user** with an elevated token, never SYSTEM; with `NoDefaultAdminOwner=1` (the Vista+ default) `CreateDirectoryW` assigns the user's own SID as owner. On the **second** apply/restore, `dir_verdict`'s `owner_is_admin_or_system` check fails — every apply/restore after the first is refused permanently on a stock Windows install. | `crates/dm-elevated/.../secure_dir.rs` | Traced the SDDL string through `dir_verdict`'s owner check against Windows' documented `NoDefaultAdminOwner` default. Fix: pin owner to Builtin Administrators (`O:BA`). Also hardened `materialize_ico` to temp+fsync+rename (was bare `fs::write`) — the ICO is read live by Explorer's icon cache off HKLM Shell Icons, so a crash mid-write must not leave a torn icon. |

### P2 — data-loss risk / silent-fail edge cases

| Commit | Finding | File(s) | Verification |
|---|---|---|---|
| `07dbc0a` | Windows Explorer BOM-prefixes a `.url` (and `desktop.ini`) whenever its content has non-ASCII characters — routine for Chinese site names/paths, this product's userbase. `str::trim()` does NOT strip U+FEFF (Unicode Cf, not White_Space), so `[InternetShortcut]` sitting behind a BOM failed the section match: icon apply silently no-op'd, read-back silently returned `None`. `parse_desktop_ini_icon` already stripped the BOM — the `.url` path was the overlooked twin. | `.url` shortcut icon read/write path | Reproduced the BOM-prefixed file scenario directly; confirmed `desktop.ini`'s existing BOM-strip as the parity reference for the fix. |
| `7e2b2eb` | Four blind-write hardenings, msvc cross-check clean: (1) wallpaper capture used a **lenient** read that collapsed a `GetWallpaper` COM error to `None` — a transient failure records a monitor's original as "empty", `hasBackup` goes true, the user believes they're protected, and restore later **clears the real wallpaper to background colour**. Switched to a strict read that fails the whole capture instead. (2) `known_folders::desktop_roots` `?`-propagated a single failed known-folder lookup, so one `FOLDERID_PublicDesktop` failure could discard the resolved **user** Desktop and fail the entire scan. (3) apply (folder/shortcut/file_wrapper) now re-checks `FILE_ATTRIBUTE_REPARSE_POINT` at apply time — scan excludes reparse points but `is_dir()`/`exists()` FOLLOW a junction, so a folder/file swapped for a junction in the scan→apply window would otherwise write through into the link's target. (4) STA pump jobs now run under `catch_unwind` so one panicking job can't kill the STA thread and take all COM (shortcut/wrapper/scan) down for the rest of the process. | Windows wallpaper/scan/apply adapters | Each of the 4 traced to a concrete data-loss or full-outage scenario (not theoretical); msvc cross-check green post-fix. |
| `7091c77` | Four icon-core edge-case hardenings, all reproduced with PoCs: `segment_subject` underflows `(h-1)*w` and panics OOB at a 0-dimension raster (reachable through the crate's own public `RenderSession::register/render` surface); `filters::gloss` at `size==1` computes `0/(1-1)` = NaN, propagating to `clamp_byte(NaN)` and silently blacking out the one opaque pixel; `marks::NATIVE_ARROW`'s `RwLock` could poison-cascade a single render-thread panic into every later shortcut render (switched to `unwrap_or_else(into_inner)`); `codec::ladder::resample_ladder` only filtered rungs on the width axis, so a non-square source could drive `scale_y` as an upsample (soft y-axis) — now filters both axes. | `crates/dm-icon-core` | Each PoC'd with a dedicated regression test; the 1487-cell parity-determinism cert is unchanged (none of these sizes appear in the corpus — confirming these were real gaps, not corpus blind spots). |

### Dead code removed (audit-confirmed zero call sites)

| Commit | Removed | Verification |
|---|---|---|
| `a339196` | Legacy WebView2 transport (superseded by the P1 fix above) + `assertSchema` (zero callers — it compared `BRIDGE_SCHEMA_VERSION` against itself via the mock, never against a real host) + `FrameMeta` DTO. | grep-confirmed zero remaining references post-removal; `tsc -b` + bun test green. |
| `d95c4ba` | `KindBucket` enum (never constructed or matched — the TS side's `IconKindBucket` carries the real logic) + `raster::{make_raster, clone_raster, TRANSPARENT}` (orphaned TS-parity aliases, everything uses `Raster::new`/`.clone()`/`WHITE`). | grep-confirmed zero call sites in `crates/dm-icon-core`. |
| `5103db6` | `wallpaper.getSource` (returned the HOST's active screen, which the client-only `selectScreen` never syncs — **3 call sites carried explicit "NEVER use getSource, it resolves the wrong monitor" warnings**; superseded when per-screen source moved onto `screens[]`, schema 6) + `diagnostics_ping` (an M2 liveness-probe scaffold, never called from the app) + `SystemInfoDto.dotnetVersion` (a .NET holdover from the retired C# host, meaningless under Tauri/Rust). | Removed end-to-end: `types.ts`/`tauri.ts`/mock/Rust command/`WallpaperHost::primary_source`/`dm-contracts::DiagnosticsPing`; `generated.ts` regenerated, bindings-drift guard green. |

### Other

| Commit | Change | Note |
|---|---|---|
| `5e34d7e` | `webview-hardening` detected the packaged app via a raw `window.chrome.webview` check — **misfired** on Windows (WebView2 injects it on every page, see the P1 above) and **missed macOS entirely** (WKWebView never sets it, so the kiosk-chord/context-menu guards silently never armed there). Switched to `isTauri()`, the real "are we the packaged app" signal. | Same root-cause class as the `a339196` P1 bridge bug — one shared detection primitive, two independent bugs from trusting it. |
| `ca4e186` | Regression **self-caught by the audit's own re-review**: the `de1617e` owner fix (`D:P...` → `O:BAD:P...`) left `the_dacl_is_protected` asserting `starts_with("D:P")`, which now fails against the new string. Updated the assertion to check the owner is pinned to Administrators (`O:BA`) AND the DACL is protected (`D:P`). Also corrected a now-stale `resample_ladder` doc comment (post-`7091c77`) to say a rung must fit BOTH source dimensions. | Proof point for "verify every fix" discipline — a same-night fix introduced its own test staleness, caught before the audit closed instead of shipping. |
| `9e9e56a` | Audit item **#3** (owner go): `fonts.list` returned 3 hardcoded families regardless of what's actually installed. Wallpaper zone titles render in the web layer, so switched to the real `queryLocalFonts()` (WebView2/Chromium) with a fallback to the curated list when the API is absent (WKWebView) or permission is denied. | Owner-approved go item, not a defensive fix. |
| `f4c1853` | Audit item **#1** (owner go): About/support links (GitHub, releases, issues, X, Bilibili, Douyin, crash-report issue+mailto) and "open data folder" were mock-only — silent no-ops on the packaged app (`window.open` blocked by the strict CSP). Wired `tauri-plugin-opener` (`opener:allow-open-url` + `opener:allow-open-path`), routed `shell.openExternal`→`openUrl`, `shell.openDataFolder`→`openPath(appDataDir())`. | Owner-approved go item. |

## 3. Disposition summary

- **Fixed + committed (all 11, all gates green):** every row in §2.
- **Fixed but folded into M6-WIRE Wave B / M7 scope, not a standalone item tonight:** the
  apply commit→ledger reconcile gap (**F4 / audit #5**) is the same entry point as the eventual
  Wave B `apply` command wiring — tracked there, not duplicated. `ICON-6`/`ICON-7` (batch/
  `output_cache` native optimization layers, landed earlier this window on the M6 kernel-speed
  line) carry forward as the render path M7's `dm-resident` will call (`render_icons_par`), NOT
  deleted.
- **HELD — owner has NOT ruled, do NOT touch:** `ICON-5` (`clamp_u8_round_half_even`, suspected
  dead code + a possible rounding concern), `ICON-9` (a suspected-dead mono branch), `ICON-11`
  (`color.rs`, 5 functions suspected superseded by the field-plate path). Described to the owner
  in audit item **#6**; no disposition yet. Treat all three as live code until the owner rules.
- **Confirmed NOT dead code, kept:** `SHELL-2` `read_target()` — an unwired "resolve a
  shortcut's target's own icon" fallback; the M7 resident desktop scan will need it.
- **Owner-approved go, not started tonight (independent of M7/Wave B):** audit item **#7** —
  `diagnostics.getInfo` wired to real system info (Rust, `[WIN]`), plus 3 marginal P3s the
  audit's own scan under-triaged: `ELEV-3` (elevated command-line escaping), `APPLY-3`
  (Recycle Bin registry value type), `CORE-1` (`WaitForSingleObject` usage). Owner:
  "那个 AI 很傻，在这里修" — fix here, not in the crate the finding was originally raised against.

## 4. blake3 / msvc cross-check block (environment gap, not a code defect)

`dm-elevated`'s `cargo check --target x86_64-pc-windows-msvc` is blocked **on the Mac host
only** by blake3's asm build requiring `ml64.exe` (Microsoft's x64 assembler) — pulled in
transitively via `dm-icon-core`'s Phase-4a `output_cache` (content-addressed cache, M6
kernel-speed line). On a real Windows box with the MSVC toolchain, `ml64.exe` is present and
this compiles clean — verify there, this is not a code defect. **If Wave B wires
`output_cache` into the production icon-apply path**, blake3 stops being a cert-time-only
dependency and becomes load-bearing — keep it, do not strip it looking for a quick msvc-check
workaround on Mac.

## 5. codex 5-minute death — root cause (owner asked to confirm)

**Not a codex timeout.** Root cause: a **Bash background process gets reaped by the harness
at agent-turn end (yield)**, regardless of the child process's own configured lifetime budget.
Evidence: a codex run kept in the **foreground** ran the full 9m20s before hitting Bash's own
10-minute cap (its actual limit, not a codex-side one); every codex run launched into the
**background** died shortly after the turn yielded, well under any codex-side timeout. A codex
run wrapped in an **Agent subagent** (which the harness keeps alive across turns) survived
normally.

**Rule going forward:** dispatching codex via `/multi-ai solo` must run either **in the
foreground of the current turn**, or **inside an Agent subagent** — never as a bare Bash
`run_in_background` process. This is a harness-lifecycle fact, not a codex reliability issue.

## 6. Gate evidence

`cargo test` (workspace) green · `cargo check -p dm-elevated -p dm-domain -p dm-windows
--target x86_64-pc-windows-msvc` green (blake3/`dm-elevated` exception noted §4) · `tsc -b`
green · `bun test` 516 green · `generated.ts` bindings-drift guard green (post
`5103db6`/`a339196` surface changes).
