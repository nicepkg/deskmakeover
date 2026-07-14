# DeskMakeover Docs

**Now:** DeskMakeover, Chinese product name "桌面美颜" (ADR-0002), is a Windows 10/11 desktop
beautifier: one window, a left module rail (**图标 / 壁纸 / 清爽 / 设置**), a live desktop-mirror
canvas + right inspector, fully reversible. It is a **Tauri 2 + Rust** desktop app (ADR-0019); the
UI is **React** in the Tauri webview (WebView2 on Windows) in the v3 **"Premium Flat"** visual
language (ADR-0013, light-first — supersedes ADR-0012). **One Rust icon core** (`dm-icon-core`,
WASM preview/bake + native resident/background) is the single pixel truth; wallpaper compositing is Pixi in
the web; the bridge contract is generated from `dm-contracts` via tauri-specta. The old .NET/C# host
is **retired** — quarantined under `legacy/` as the frozen parity oracle only. The in-app version
narrative is restored (ADR-0013 amendment). The interactive prototype under `references/prototype/`
is **historical reference only** — the specs are the intended source of truth.

## Doc Map

- [STATE](STATE.md) — current work, next steps, blockers, open questions. **Start here.**
- [development.md](development.md) — **dev & build runbook**: how to develop, test, and package on
  the Tauri 2 + Rust stack (Bun web loop + Cargo + `bun run tauri:dev`), plus the gotchas
  (bindings drift guard, the `dm-elevated` helper security boundary, i18n source-of-truth in `src/lib/i18n`,
  owner-supervised gates). The `legacy/` .NET tree is oracle-only and does not ship.
- [decisions/](decisions/) — ADRs. Current governing: **0013** (v3 Premium Flat — supersedes 0012),
  **0016** (icon colour-field), **0017** (per-type distinction), **0018** (two-axis colour),
  **0019** (Tauri + Rust replatform — supersedes 0001/0011, amends 0014/0015), **0020** (background
  resident v1), **0021** (global arrow overlay default), **0022** (M7 appearance model + consent),
  **0023** (calm-Windows module). Older ADRs 0001–0012/0014/0015 are superseded or amended — each
  status header carries the pointer.
- [specs/](specs/) — capability specs: 00 roadmap · 01 product architecture ·
  **02 visual language (v3)** · 03 shell + settings + IA · 04 wallpaper module · 05 bridge (Tauri /
  Rust) · **06 icons module** · 07 background resident · 08 calm-Windows.
- [reviews/](reviews/) — design-panel reviews and audit records.
- [plans/](plans/) — point-in-time implementation plans (older plans are history).
- [verification/](verification/) — the owner-supervised live-run checklist (real icon-bake +
  wallpaper-apply + resident audit + calm writes — never auto-triggered).
- [conventions/](conventions/) — project-local engineering conventions (Web + Rust).
- [references/prototype/](references/prototype/) — the owner's original design prototype;
  **historical reference only** (superseded by the specs).
- [references/windows-settings-rust/](references/windows-settings-rust/) — researched Windows 11
  calm-settings capability matrix and compile-checked platform-adapter reference. A handoff artifact,
  not a declaration that the direct setters have passed the required Windows VM matrix.
