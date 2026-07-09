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
confession, and the native-arrow 60s penance sheet) is a deliberate brand
ceremony authored verbatim by the owner. It is NOT subject to the tone rule and
must not be softened; the rule continues to govern everything else.

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

**Not in scope / not yet:** 新图标自动美化 (keep-up) — the setting is HIDDEN and
defaults OFF until a real watcher/catch-up exists (owner decision 2026-07-10;
never promise an absent capability). AI icon generation; system-cleaner
anything; auto-update framework; Win7.

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
- Kept-original shortcuts render the classic Windows arrow, matching the desktop.

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

## System Architecture (post-inversion, ADR-0014/0015)

**The web renders the pixels; C# is the system hand.** Details: spec 05 §1.

| Piece | Role |
|---|---|
| `DeskMakeover.Web` | The visible UI (React 19 + Tailwind 4 + Motion, Bun-only). Renders ALL preview + bake pixels: CPU TS icon compositor (Worker) + Pixi wallpaper compositor. Zustand state, undo/redo, i18n (resx-generated) |
| `DeskMakeover.App` | WPF host: frameless window + WebView2 + JSON-RPC bridge (schema 3 contract; host wiring = F8), settings/changelog/diagnostics, orchestration of bake/apply |
| `DeskMakeover.Core` | Pure domain types — no Win32/WPF |
| `DeskMakeover.IconRendering` | ICO ladder/packaging + the frozen C# TileRenderer as the golden parity oracle (ADR-0015 D3); legacy renderers pending F8 deletion |
| `DeskMakeover.Shell` | Win32/Shell adapters: scanner, shortcut COM, Explorer refresh, wallpaper, restore metadata |
| `DeskMakeover.Operations` | Journaled runner, planner, snapshot factory/store, history ledger |
| `DeskMakeover.ElevatedHelper` | Whitelisted privileged verbs, one-UAC batching |

**Config truth**: `src/DeskMakeover.Web/src/bridge/types.ts` (BRIDGE_SCHEMA_VERSION
= 3) — ConfigDto axes: shape (11-shape catalog) × colour {orig, bw, mono(+tint,
monoStyle Tonal|Flat, plateColor)} × distinction {mark, keep, none} × markStyle
{Glass, Shadow, Halo, Satin, Arc, Fold, Ring} × markColor × filter {None, Gloss,
Glass, Pixel, Sticker} × kindPolicy. `size` is a READ-ONLY observed field —
the size control was removed (owner, `d708f87`); nothing may write the real
desktop icon size, incl. history replay (guard = F8).

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

- Web: `bun test` (297 at HEAD — compositor fixtures, stores, zone math, i18n
  parity, banned-colour + copy gates) + `tsc -b` + browser visual evidence
  (`docs/plans/evidence/`).
- C#: `dotnet test` (277 pre-v3; re-verify at F8) + golden parity fixtures
  (web renderer vs frozen C# oracle: flat ΔE<2 / SSIM≥0.995, filters ≥0.98).
- E2E: raw CDP client (Bun), opt-in, applies stubbed (`DESKMAKEOVER_FAKE_APPLY=1`).
- Manual matrix on clean VMs (Win10+11, mixed DPI, OneDrive, UAC denial,
  interrupted apply) + the owner-supervised live runs
  (`docs/verification/owner-supervised-live-runs.md`, pending F8 rewrite).
- A bug fix ships a regression test reproducing the failure.
