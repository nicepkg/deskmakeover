# DeskMakeover Docs

**Now:** DeskMakeover, Chinese product name "桌面美颜" (ADR-0002), is a Windows 10/11
desktop-**icon** and **wallpaper** beautifier: one window, a left module rail (图标 /
壁纸 / 设置), live desktop-mirror canvas, fully reversible. The UI is a **WebView2 +
React** web app (ADR-0011) styled in the v2 "Quiet Material" (macOS-System-Settings)
visual language (ADR-0012); the engine stays C#. The old interactive prototype
(`references/prototype/桌面美颜 v2.dc.html`) is now a **historical reference only** —
the specs are the source of truth (ADR-0012). Pre-release; no version narrative in-app
until the first real release.

## Doc Map

- [STATE](STATE.md) — current work, next steps, blockers, open questions. **Start here.**
- [development.md](development.md) — **dev & build runbook**: how to develop, test, and
  package, plus the gotchas (the repo-local .NET SDK, the two publish scripts, the
  ElevatedHelper security boundary, i18n source-of-truth, owner-supervised gates).
- [decisions/](decisions/) — ADRs. Current governing: **0011** (WebView2 + React
  replatform), **0012** (premium UI redesign / prototype retired). 0009 (rail +
  wallpaper), 0010 (settings + i18n + shapes) still apply.
- [specs/](specs/) — capability specs: 00 roadmap · 01 product architecture ·
  **02 visual language (v2)** · 03 shell + settings + IA · 04 wallpaper module ·
  05 web-shell / bridge.
- [reviews/](reviews/) — design-panel reviews (e.g. the 2026-07-08 premium-UI panel).
- [plans/](plans/) — point-in-time implementation plans (older plans are history).
- [verification/](verification/) — the owner-supervised live-run checklist (real
  icon-bake + wallpaper-apply — never auto-triggered).
- [conventions/](conventions/) — project-local engineering conventions.
- [references/prototype/](references/prototype/) — the owner's original design prototype;
  **historical reference only** (superseded by the specs per ADR-0012).
