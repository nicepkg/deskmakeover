# DeskMakeover Docs

**Now:** DeskMakeover, Chinese product name "桌面美颜" (ADR-0002), is a Windows
10/11 desktop-icon beautifier: one screen, one CTA, fully reversible. The owner
built a complete interactive prototype (`references/prototype/桌面美颜 v2.dc.html`)
which is the **binding UI/UX contract for v1.0** (ADR-0008). The foundation
(domain, rendering, scanning, snapshots, journaled ops, elevated helper) is built
and tested; the UI is being rebuilt to prototype parity per the 2026-07-06 plan.
v1.0 = icon beautification only; system-ads / Explorer / wallpaper modules are
design-locked for v1.1+.

## Version Timeline

- Unreleased → **v1.0.0** (renumbered from "v0.9 抢发" by ADR-0008) — foundation
  done (~105 tests green); prototype-parity UI rebuild in progress.

## Doc Map

- [STATE](STATE.md) — current work, next steps, blockers, open questions. **Start here.**
- [references/prototype/](references/prototype/) — the owner's Claude Design
  prototype: **the v1.0 source of truth** (open the .html in a browser).
- [plans/2026-07-06-v1-prototype-parity.md](plans/2026-07-06-v1-prototype-parity.md)
  — the executable rebuild plan (phased, task-level, for the executing AI).
- [specs/00-roadmap.md](specs/00-roadmap.md) — v1.0 原型复刻 → v1.1 净化 →
  v1.2 文管 → v2.0 壁纸/AI.
- [specs/01-product-architecture.md](specs/01-product-architecture.md) — product
  scope, IA, copy, system architecture.
- [specs/02-visual-language.md](specs/02-visual-language.md) — tokens, geometry,
  colour math, the 7 shortcut marks, motion.
- [decisions/](decisions/) — ADRs; 0008 is the governing one for v1.0.
- [conventions/](conventions/) — project-local engineering conventions.
- [plans/](plans/) — point-in-time implementation plans (older plans are history).
