# 10 — Website (dm.nicepkg.cn landing page)

Status: approved (owner, 2026-07-16 — product-studio four-chief panel + owner Q&A)
Owner decisions recorded in `docs/plans/2026-07-16-landing-page.md` §Decisions.

## Scope

One static marketing page for DeskMakeover at `dm.nicepkg.cn`, bilingual (`/` English,
`/zh` Chinese), with a Download call-to-action, deployed to Cloudflare Workers Static
Assets via wrangler, auto-deployed from CI.

## Non-scope

- No docs site, blog, or comparison page (candidate for a later iteration).
- No dark mode (`color-scheme: light` is locked; the product is light-first).
- No server code of any kind: no SSR, no middleware/proxy, no API routes, no Worker script
  logic. `next build` with `output: 'export'` must produce plain files.
- No cookie banner and nothing that would require one.

## Assumptions

- The repo is public by the time the site is announced (Star/Watch links 404 otherwise).
- v0.1.0 installer may not exist at first deploy — the Download CTA has a build-time
  dual state (see §Download).
- `CLOUDFLARE_API_TOKEN` + `CLOUDFLARE_ACCOUNT_ID` GitHub secrets are created by the owner
  once; everything else is automated.

## Dependencies

- Brand assets in `.github/assets/` (hero frames, preset gallery, feature shots,
  social card, logo). The website build re-encodes copies; it never mutates the originals.
- App visual language `docs/specs/02-visual-language.md`: coral `#FF6F5E` is the only
  accent ("saturation is an event"), `--coral-ink` deeper register for large fills/text,
  `--cta-ink` warm white `#FFF7F3`. The website extends, never contradicts, this language.

## Information architecture (both locales, 7 screens max)

1. **Hero** — one-line value + primary Download button + "you can always go back" subline.
   Before/after desktop transformation with coral scan-line reveal.
2. **The safety promise** — snapshot-first / one-click restore / local-only / nothing
   technical. This is the conversion gate for this category; it never moves below fold 3.
3. **Nine built-in looks** — specimen wall: nine preset cards (existing gallery webp).
4. **Deep customization** — `feature-combine` shot: live preview + per-axis controls.
5. **Wallpaper zones + shortcut arrow** — `feature-zones` shot + arrow paragraph.
6. **Inside the studio + style packs** — `app-studio` + `feature-stylepack`.
7. **Honest beta + Download (second CTA) + FAQ + open-source footer.**

Copy inherits the README voice verbatim where possible (calm, consumer, zero tech vocab
in product sections). Chinese copy contains no dashes and no exclamation marks.

## Routing & i18n

- `/` = English, `/zh` = Chinese; both fully static-prerendered. No automatic locale
  redirect of any kind (no meta refresh, no JS sniffing). Header carries an explicit
  language switch.
- Dictionaries are plain TypeScript objects; no i18n library.
- `hreflang` alternates on both pages (`en`, `zh-CN`, `x-default` → `/`).

## Hero motion contract

- Two frames (before/after) as AVIF with WebP fallback, explicit width/height,
  `fetchpriority=high` on the LCP frame.
- On load: coral scan-line (clip-path inset reveal) plays the transformation ONCE,
  then hands control to a "put it back" button that elastically restores the before frame
  (the signature moment). Replay affordance stays visible.
- `prefers-reduced-motion: reduce` → static after-frame, button swaps frames with no
  animation. The scan animation is CSS-only; JS only toggles classes.

## Motion discipline (site-wide)

- Zero animation libraries. CSS transitions/keyframes + one IntersectionObserver util.
- Exactly two signature animations: hero scan reveal + gallery staggered fade-up
  (40–60 ms stagger). Everything else is hover/focus micro-transition.
- CSS scroll-driven animations only inside `@supports (animation-timeline: view())`,
  with the IO reveal as fallback.

## Design tokens (website layer)

- Neutrals are cool-tinted OKLCH, not pure gray: bg `oklch(0.99 0.004 255)`, surface
  `oklch(0.975 0.006 250)`, border `oklch(0.92 0.008 250)`; body ink `#2f363d`.
- Coral coverage ≤ 10% of any viewport: one primary CTA + active states + scan line.
  Coral→deep-coral 135° gradient allowed ONLY on the primary CTA and hero scan.
- Display type: `clamp(2.5rem, 6vw, 4.5rem)`, line-height 1.05, tracking −0.02em;
  body 17–18 px, line-height 1.6.
- Shadows are layered, cool-tinted, never pure black; hairline borders preferred.
- Radii echo the squircle family: cards 16–20 px, buttons 10–12 px.
- Section vertical rhythm `clamp(80px, 10vw, 120px)`; 8-pt grid.
- Banned: left accent bars, handwriting fonts, glassmorphism icon grids, neon/mesh
  gradients, giant decorative emoji, fabricated stats, stock screenshots.

## Fonts

- Latin: one self-hosted variable font (latin subset, ≤ 35 KB woff2), `font-display: swap`.
- Chinese body: system stack (`PingFang SC`, `Microsoft YaHei`, `Noto Sans SC`) — 0 bytes.
- Chinese display: build-time subset of a modern sans licensed for embedding, covering
  exactly the glyphs used in zh display headings (≤ 25 KB). Subsetting is a build step.

## Images

- `images.unoptimized: true` (mandatory under export). A build script (sharp) emits
  AVIF + WebP variants at display-appropriate widths with intrinsic dimensions; pages use
  `<picture>` with explicit width/height. First-screen images eager, the rest lazy.

## Download contract (dual state)

- Build-time constant `RELEASE_READY: boolean` in one config file.
  - `false`: button reads "first installer on the way" and links to the GitHub repo
    (Watch/Star capture); no dead link to an empty Releases page.
  - `true`: button links to `https://github.com/nicepkg/deskmakeover/releases/latest`
    (never a direct .exe URL — filename changes must not produce dead links).
- SmartScreen honesty line sits directly under the Download button (both CTAs), with an
  expandable "More info → Run anyway" walkthrough. Never hidden in FAQ only.
- Non-Windows user agents (client-side check): button swaps to "open dm.nicepkg.cn on
  your PC" + copy-link + mailto fallback. Page remains browsable.

## SEO / GEO

- Next metadata API: per-locale title/description/canonical/OG/Twitter
  (`summary_large_image`, existing `social-card.png`).
- JSON-LD inline: `SoftwareApplication` (applicationCategory UtilitiesApplication,
  operatingSystem Windows 10/11, price 0, license MIT, downloadUrl) + `FAQPage`.
  No aggregateRating until real ratings exist.
- `app/sitemap.ts` + `app/robots.ts`; robots explicitly allows GPTBot, ClaudeBot,
  Claude-Web, PerplexityBot, OAI-SearchBot, Google-Extended.
- Static `llms.txt` (short product description + key links).
- FAQ answers are self-contained (name the product + platform in the answer) so LLMs can
  quote them standalone. FAQ content: the nine questions listed in the plan.

## Analytics

Cloudflare Web Analytics beacon only (no cookies) + a custom event on Download clicks.
No GA4.

## Deploy architecture

- `website/` inside this repo, own `package.json` (root has no `workspaces`; keep it so).
- Hosting: Cloudflare **Workers Static Assets** — `wrangler.jsonc` with
  `assets: { directory: "./out" }` and
  `routes: [{ pattern: "dm.nicepkg.cn", custom_domain: true }]`. No Worker script file:
  requests are served from assets and bill nothing.
- CI: `.github/workflows/website.yml`, path-filtered to `website/**`; builds with bun,
  deploys with wrangler on push to main; PR runs build + preview upload.

## Performance budget (acceptance)

- LCP < 1.8 s (Fast 3G lab), CLS < 0.05, INP < 200 ms.
- JS: non-framework (site) JS ≤ 15 KB gz; total ≤ 200 KB gz. Measured floor of the
  Next 16 app-router runtime on this page is ~183 KB gz (deferred scripts — LCP renders
  from static HTML before any JS runs), so a lower total is not reachable on the
  owner-fixed stack.
- Every image has explicit dimensions; fonts preloaded only when actually used above
  the fold.

## Acceptance criteria

1. `bun run build` in `website/` produces `out/` with zero server artifacts.
2. Both locales render fully with JS disabled (content, images, FAQ text in static DOM).
3. Hero plays once, restore button works, reduced-motion path verified in a real browser.
4. Lighthouse (mobile emulation) meets the performance budget on the deployed URL.
5. `wrangler deploy` from `website/` serves the site on dm.nicepkg.cn over HTTPS.
6. CI deploys on a `website/**` push to main and skips on unrelated pushes.
7. JSON-LD validates (Rich Results test schema-parse level); hreflang pair resolves.
