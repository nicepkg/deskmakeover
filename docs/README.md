# DeskMakeover Docs

**Now:** DeskMakeover, Chinese product name "桌面美颜" (ADR-0002), is a Windows 10/11
desktop-**icon** and **wallpaper** beautifier: one window, a left module rail (图标 /
壁纸 / 设置), a live desktop-mirror canvas + right inspector, fully reversible. The UI is a
**WebView2 + React** web app (ADR-0011) in the v3 **"Premium Flat"** visual language
(ADR-0013, light-first — supersedes ADR-0012's "Quiet Material"). The web renders the preview
and bake pixels itself (CPU TS icons, Pixi wallpaper); C# keeps window / source decode / ICO /
shell write / backup. The in-app version narrative is RESTORED (ADR-0013 amendment). The old
interactive prototype is a **historical reference only** — the specs are the intended source of
truth, but several have drifted: see **[STATE.md](STATE.md) §Known doc drift** before trusting them.

## Doc Map

- [STATE](STATE.md) — current work, next steps, blockers, open questions. **Start here.**
- [development.md](development.md) — **dev & build runbook**: how to develop, test, and
  package, plus the gotchas (the repo-local .NET SDK, the two publish scripts, the
  ElevatedHelper security boundary, i18n source-of-truth, owner-supervised gates).
- [decisions/](decisions/) — ADRs. Current governing: **0011** (WebView2 + React
  replatform), **0013** (v3 Premium Flat redesign — supersedes 0012), **0014** (zone editor
  rebuild / wallpaper renderer ownership), **0015** (icon renderer ownership — web renders).
  0009/0010 partially apply. Many older ADRs are partially superseded — status map in STATE.md.
- [specs/](specs/) — capability specs: 00 roadmap · 01 product architecture ·
  **02 visual language (v3)** · 03 shell + settings + IA · 04 wallpaper module ·
  05 web-shell / bridge · **06 icons module**. ⚠️ 01 & 05 (and parts of 00/02/03/04/06) carry
  pre-inversion architecture — reconcile against STATE.md §Known doc drift.
- [reviews/](reviews/) — design-panel reviews (e.g. the 2026-07-08 premium-UI panel).
- [plans/](plans/) — point-in-time implementation plans (older plans are history).
- [verification/](verification/) — the owner-supervised live-run checklist (real
  icon-bake + wallpaper-apply — never auto-triggered).
- [conventions/](conventions/) — project-local engineering conventions.
- [references/prototype/](references/prototype/) — the owner's original design prototype;
  **historical reference only** (superseded by the specs per ADR-0012).
- [references/windows-settings-rust/](references/windows-settings-rust/) — researched Windows 11
  calm-settings capability matrix, reversible transaction reference, and compile-checked
  `winreg`/`windows-rs` platform adapters. This is a handoff artifact, not a declaration that the
  direct setters have passed the required Windows VM matrix.
