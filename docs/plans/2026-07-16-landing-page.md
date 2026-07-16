# Plan — dm.nicepkg.cn landing page (2026-07-16)

Spec: `docs/specs/10-website.md`. Executor: Commander session, sequential slices with
per-slice verify. Reviewer: cross-vendor (codex) two-stage, then real-browser + designer
acceptance.

## Decisions (owner Q&A, 2026-07-16)

| # | Decision | Choice |
|---|----------|--------|
| 1 | Hosting | Cloudflare Workers Static Assets (free static, full-CLI custom domain) |
| 2 | Download empty-window | dual build-state; pre-release = Watch/Star capture, post = releases/latest |
| 3 | Art direction | "Transformation theater + specimen wall" (A + B blend, C warmth) |
| 4 | Hero interaction | auto scan once → "put it back" restore button; no drag slider in v1 |
| 5 | Code location | `deskmakeover/website/` (own package.json, root keeps no workspaces) |
| 6 | Locales | `/` = English, `/zh` = Chinese (owner overrode zh-root recommendation) |
| 7 | Display font | modern sans (zh display subset; no serif) |
| 8 | Analytics | Cloudflare Web Analytics + download-click event |

Panel synthesis + full seat reports live in the Commander session (2026-07-16); the
durable summary is this table plus the spec.

## Global constraints

- All copy sourced/adapted from README.md / README.zh-CN.md voice; zh copy: no dashes,
  no exclamation marks.
- No animation libraries; no left accent bars; no handwriting fonts; coral ≤ 10%.
- Never mutate `.github/assets/` originals; the asset script reads from there and writes
  into `website/public/`(generated, gitignored) or `website/assets-src/` intermediates.
- Verify commands run from `website/` unless stated.

## Slices

1. **Scaffold** — `website/` Next 16 + Tailwind v4 + TS, `output:'export'`,
   `trailingSlash:true`, `images.unoptimized:true`, `color-scheme:light`.
   Verify: `bun run build` emits `out/index.html` + `out/zh/index.html`.
2. **Asset pipeline** — `scripts/build-images.mjs` (sharp): extract hero before/after
   JPEGs from `hero-beforeafter.svg` base64, emit AVIF/WebP variants + manifest with
   intrinsic sizes; re-encode gallery/feature/social shots to display widths.
   Verify: script idempotent; total hero payload ≤ 400 KB across both frames.
3. **Design tokens + layout shell** — Tailwind v4 `@theme` OKLCH tokens per spec, header
   (logo + language switch + GitHub), footer, section rhythm, rule-gradient divider.
   Verify: build + visual smoke in browser.
4. **Sections EN** — 7-screen IA with dictionary-driven copy, hero scan + restore
   interaction, gallery stagger, FAQ accordion, dual-state Download.
   Verify: JS-disabled render shows full content; reduced-motion path manual check.
5. **zh locale** — dictionary + zh display-font subset build step + hreflang.
   Verify: `out/zh/index.html` full render, no dash/exclamation in zh copy (grep gate).
6. **SEO/GEO** — metadata, JSON-LD, sitemap, robots, llms.txt, OG.
   Verify: JSON parses; robots lists AI crawlers; `out/sitemap.xml` has both locales.
7. **Deploy** — `wrangler.jsonc` static assets + custom domain; first manual
   `wrangler deploy`; smoke https://dm.nicepkg.cn.
   Verify: `curl -sI https://dm.nicepkg.cn` 200 + content spot-check.
8. **CI** — `.github/workflows/website.yml` path-filtered build+deploy; document the two
   owner-created secrets. Verify: workflow green on a website/** push.
9. **Review + acceptance** — codex two-stage review; chrome-cdp walkthrough (locales ×
   reduced-motion × mobile viewport); designer-seat pixel acceptance loop; Lighthouse
   budget check on the live URL.

## Rollback

Static site: rollback = redeploy previous `out/` (wrangler keeps versions). No data, no
migrations; CI deploy is safe to re-run.
