# Spec 01 — DeskMakeover Product Architecture (v3)

Living spec. Rewritten 2026-07-10 (post ADR-0011/0013/0014/0015): the old
prototype-derived v1.0 text is superseded — the prototype is historical, **the
specs + `docs/STATE.md` are the truth**. Module behaviour lives in spec 06
(icons), spec 04 (wallpaper), spec 03 (shell/settings); visuals in spec 02;
host/bridge in spec 05. This file holds the product identity, scope, trust
model, and system architecture.

## Product Identity

DeskMakeover is the English product name; Chinese name **桌面美颜** (ADR-0002).
Slogan: 「一键美颜你的 Windows 桌面，随时完整还原」 / "Give your Windows desktop a
one-click makeover. Restore everything anytime." Narrative (ADR-0004): 「让
Windows 回到它本该的样子」 — a beauty camera for the desktop, never a
管家/cleaner. Positioning: 本地运行 · 无账号 · 无遥测 · 全程可还原 · 免费开源.

**Tone rule (ADR-0003):** in-product voice never judges the user's desktop;
additive voice only; no anxiety/cleaner/fear language.
*Owner-decreed exception (2026-07-10):* the first-run welcome-gate survey ritual
(ADR-0013 amendment — the send-off, the "go uninstall" bluff-call, the typed
confession) is a deliberate brand ceremony authored verbatim by the owner. It is
NOT subject to the tone rule and must not be softened; the rule continues to
govern everything else. (The native-arrow 60s penance sheet RETIRED per ADR-0021
— its object no longer exists; the rest of the ritual is untouched.)

## Target Users

Non-technical Chinese Windows users first; design-sensitive users satisfied
through restraint, not options. Windows 10 + 11; Win7 excluded. Ships zh-Hans +
English; language defaults to the Windows UI culture (fallback English).

## Product Scope (v3)

Two beautify modules + settings, in one window (module rail, spec 03):

1. **图标 (icons, spec 06)** — scan → live styled preview on a desktop-mirror
   canvas → 一键美化 (snapshot → journaled bake) → tweak → update → restore
   anytime. Style axes: 11-shape catalog · colour treatments (原彩/黑白/极致单色)
   · shortcut marks · filters · per-icon overrides · per-bucket participation
   (`kindPolicy`: apps/folders/files/system).
2. **壁纸 (wallpaper, spec 04)** — zone panels painted INTO the wallpaper
   (five materials, four title styles), clarity dim, import/export; original
   wallpaper backed up, one-click return.
3. **设置 (spec 03)** — appearance (theme/language), local data, about + changelog.

**Participation model (owner decision 2026-07-10):** ordinary files ARE included
by default — the product is fully reversible and every apply is user-clicked, so
default-on with a one-click per-bucket opt-out (`kindPolicy`) beats a buried
opt-in. (This supersedes the old "wrapping stays opt-in and off by default".)

**Also in scope (v1, ADR-0020)**: 新图标自动美化 (background resident auto-format,
spec 07) — a tray-resident native process formats NEW desktop icons per the saved
style behind the consent ladder (default OFF · first-run proposal · opt-in silent ·
every run undoable). The setting stays HIDDEN until the M7 build ships.

**Not in scope / not yet:** AI icon generation; system-cleaner anything;
auto-update framework (v1 ships without an updater — ADR-0019 defaults); Win7.

## The Window (IA)

Canvas-first: **desktop-mirror canvas LEFT, inspector RIGHT** (280px; 248px
compact), left module rail (图标/壁纸/设置, Ctrl+1/2/3), 46px web titlebar.
Modules stay mounted across switches (spec 05 §4). Min window 1024×700.
Full navigation spec: spec 03. (The old left-300px control panel + settings
drawer + compact slide-in overlay are superseded.)

## Canvas Behaviour (shared grammar)

- Background = the user's real wallpaper; icons at OBSERVED desktop positions;
  decorative Win11-style taskbar completes the mirror.
- Scanning: shape-clipped shimmer. Applying: progress + celebration (one
  first-apply-per-launch confetti, ADR-0013 amendment). Restore: settle.
- **Hold-Space / hold-pill = compare originals** (global gesture); per-tile
  press-to-peek; per-icon right-click overrides (保留原样 / 跟随全局 / 单独配色 /
  whole-bucket keep).
- Kept-original shortcuts render the BAKED classic arrow (ADR-0021: the native
  overlay is globally transparent by default; 「保留原样」 = original subject
  pixels + a DeskMakeover-baked classic arrow, matching what the desktop shows).

## UI Language Rules (binding)

Banned in user-facing strings: 快照*, 应用计划, dry-run, 注册表, 缓存, HKLM,
journal, raw enums (*「正在扫描桌面…」/「还原快照」 are approved product copy).
No dashes in any user-facing string (ADR-0013). Skipped items get human reasons.
Errors = what happened / what changed / what next; tech detail behind an expander.

## Trust Flow (privileged work)

Explain → one batched UAC → on denial apply all non-privileged styling, mark the
privileged step skippable/retryable, never dead-end. Light Explorer refresh
first; disruptive refresh only after telling the user. The elevated helper stays
a fixed whitelist of named verbs and its own self-contained exe (a security
boundary — `docs/development.md` §6).

## System Architecture (Tauri 2 + Rust, ADR-0019)

**Rust owns every pixel and every system hand; the web is the interactive face.**
Endpoint layout (migration phases: `docs/plans/2026-07-10-tauri-migration.md`):

| Piece | Role |
|---|---|
| `src` (+ `index.html`, root configs) | The visible UI (React 19 + Tailwind 4 + Motion, Bun-only): Zustand state, undo/redo, i18n (TS dictionaries = source of truth); Worker pool hosting the WASM icon core; Pixi wallpaper compositor (stays web-only) |
| `src-tauri` | Thin composition root: window/tray/single-instance lifecycle (window close destroys the WebView; resident mode is windowless), commands, capabilities/CSP |
| `crates/dm-icon-core` | THE single pixel truth (ADR-0019): analysis/segmentation/colour/shapes/marks/filters/compose + planner + RenderSession; compiled to WASM (preview/bake) AND native (apply/background) |
| `crates/dm-icon-codec` + `dm-contracts` + `dm-domain` | Resample ladder + ICO assembly; generated TS bindings (tauri-specta — hand-mirrored schemas banned); pure domain types |
| `crates/dm-windows` | ALL windows-rs/COM/unsafe: STA actor, desktop scan/layout, icon extraction, .lnk/.url/desktop.ini/system-icon writers, IDesktopWallpaper, watcher, Explorer refresh |
| `crates/dm-operations` | Durable transaction ledger (rusqlite), snapshots, CAS restore, history |
| `crates/dm-resident` | Background reconciler/jobs/privileged queue (spec 07) |
| `apps/elevated-helper` | Whitelisted privileged verbs (overlay pair), one-UAC batching, Program Files install |

*Transition*: the legacy .NET tree is FROZEN as an executable oracle during the
port (BakeService invariants harvested into named Rust tests) and deleted at M8
(`last-dotnet` tag). The frozen TS compositor is the primary pixel oracle until
the M6 certification gate.

**Config truth**: `bridge/types.ts` (BRIDGE_SCHEMA_VERSION = 4, two-axis
subject×plate per ADR-0018) — ConfigDto axes: shape (11-shape catalog) × subject
{Original, BlackWhite, Mono(+tint, monoStyle Tonal|Flat)} × plate {plateColor,
plateBand Vivid|Quiet, plateFallback derived|white} × distinction/markStyle ×
markColor × filter × kindPolicy × typeOverrides. Under Tauri these DTOs are
GENERATED from `dm-contracts`. `size` is a READ-ONLY observed field — the size
control was removed (owner, `d708f87`); nothing may write the real desktop icon
size, incl. history replay (guard lands with the Rust host, M3).

**Snapshot/restore**: auto snapshot → journaled apply → one-click zero-residue
restore; version history (cap 10) persists across restore.

**WYSIWYG law**: preview pixels == bake pixels because they are the SAME web
code at different resolutions (ADR-0015 D5 allows tolerance bands only for the
C#-oracle parity fixtures, not between preview and bake).

## Safety Rules

No snapshot, no apply · no silent wrapping · no destructive deletion · no silent
Explorer kill · OneDrive warnings · uncertain restore → stop and show recovery ·
skipped items visible with reasons · bake/apply are owner/user-clicked only,
never auto-triggered.

**Network**: no account, no telemetry, no cloud. Outbound network = user-clicked
links only: GitHub (repo/releases/issues), the author's homepage (xiaominglab.com),
X, Bilibili, Douyin, and `mailto:` feedback — all via `shell.openExternal` with
an http(s)/mailto whitelist. (The old "the only network actions are GitHub
links" undercounted; this list is the truth.)

## Verification Strategy

- Web: `bun test` (356 at HEAD — compositor fixtures, stores, zone math, i18n
  parity, banned-colour + copy gates) + `tsc -b` + browser visual evidence
  (`docs/plans/evidence/`).
- Rust: `cargo test` + the ADR-0019 parity gates — TS↔Rust corpus (classification
  exact, SSIM≥0.995/bounded ΔE, stage-level differential dumps) and
  wasm↔native byte-equality in CI; kill-point recovery battery around every
  ledger transition; spec 07 resident battery (bursts/overflow/self-write/
  conflict/OneDrive/sleep matrix).
- Legacy C# runs only as a differential oracle during the port; it is not a
  shipping test surface.
- E2E: raw CDP client (Bun), opt-in, applies stubbed (`DESKMAKEOVER_FAKE_APPLY=1`).
- Manual matrix on clean VMs (Win10 22H2+/Win11, mixed DPI, OneDrive, standard
  user, UAC denial, interrupted apply) in a logged-in interactive session + the
  owner-supervised live runs (`docs/verification/owner-supervised-live-runs.md`,
  to be rewritten for the Tauri stack at M8).
- A bug fix ships a regression test reproducing the failure.
