# 10 — Website (dm.xiaominglab.com landing page)

Status: approved direction v4 (owner, 2026-07-17). v1 light-editorial, v2 dark-cinema and
v3 flat-shader takes were all owner-rejected; v3 lived in `website-legacy/` until the
owner ordered its deletion (2026-07-17, git history keeps it) and the v4 rewrite shares
no visual DNA with it. Original panel + owner Q&A decisions:
`docs/plans/2026-07-16-landing-page.md` §Decisions (routing/hosting decisions still stand;
the art direction sections of that plan are superseded by this spec).

## Scope

One static marketing page for DeskMakeover at `dm.xiaominglab.com` (originally planned
as dm.nicepkg.cn; the owner remapped 2026-07-17), bilingual (`/` English,
`/zh` Chinese), with a Download call-to-action, deployed to Cloudflare Workers Static
Assets via wrangler, auto-deployed from CI.

## Non-scope

- No docs site, blog, or comparison page (candidate for a later iteration).
- No dark mode (`color-scheme: light` is locked; the product is light-first).
- No server code of any kind: no SSR, no middleware/proxy, no API routes, no Worker script
  logic. `next build` with `output: 'export'` must produce plain files.
- No cookie banner and nothing that would require one.

## Assumptions

- The repo is public (went public 2026-07-17; Star/Watch/badge links and the site's
  GitHub CTAs 404'd for visitors while it was private).
- v0.1.0 installer may not exist at first deploy — the Download CTA has a build-time
  dual state (see §Download).
- `CLOUDFLARE_API_TOKEN` + `CLOUDFLARE_ACCOUNT_ID` GitHub secrets are created by the owner
  once; everything else is automated.

## Art direction (v4, owner-mandated)

- **Flat white future-tech.** Near-white canvas, ink text, hairline `#e4e7ec` rules,
  exactly one coral accent family. Zero gradients, zero soft shadows on chrome, flat
  rectangular buttons. Mono uppercase micro-labels + a faint masked engineering grid
  carry the technical register.
- **The 3D display IS the hero, and the camera moves.** A physically-shaded three.js
  all-in-one display (iMac-inspired: thin aluminum slab, white glass front, wedge
  stand, NO logo — never a black monitor) owns the ENTIRE hero as a full-bleed
  background layer (never a clipped column); the copy floats above it behind a
  white fade. The camera opens on a wide 3/4 product view, then dollies smoothly
  INTO the screen (easeInOutQuint, ~2.5 s) — the real desktop becomes the hero
  background. Only after arrival does the screen shader play: hold before → coral
  scan wipe to after → dwell → restore → loop. Pointer peek + breathing after
  arrival. `?dm3d=wide|before|scan|after` freezes states for visual tests.
- **The machine crosses the fold.** The scene canvas extends ~42vh below the hero;
  chin, stand and foot stand into screen two over a transparent background — never
  cropped by a section edge. The mono spec strip belongs to screen two, below the
  machine (`SpecStrip`, `lg:mt-[44vh]`), keeping screen one a clean stage.
- **The device is a real scanned model, not procedural geometry.** Hero device:
  "Apple Studio Display" by alboxer2000_, CC-BY-4.0, sourced via Sketchfab
  (`assets-src/model/studio-display.glb`, ~1.5 MB, quantized + WebP textures,
  GLTFLoader). Attribution is REQUIRED and lives in the site footer — never remove
  it while the model ships. The GLB's emissive-white panel (largest such mesh) is
  hidden and replaced by the wipe-shader plane, sized from its bounding box; the
  camera choreography reads panel dimensions from the model at runtime.
- **Lighting.** Real studio HDRI (Poly Haven brown_photostudio_02 1k, CC0,
  `assets-src/env/studio.hdr` → RGBELoader → PMREM, yaw 90°); a procedural softbox
  scene remains as fallback. `?dm3drot=<deg>` spins the HDRI for design review.
- **The icon side leads the frame.** The close framing anchors the screen's LEFT
  edge beside the copy column: the icon-dense side is always fully visible, the
  right side may bleed offstage; the machine stays small enough that screen two
  is one short scroll away (spec strip gap 8vh).
- **The product name is the largest text on screen.** H1 is "DeskMakeover" /
  「桌面美颜」; the category line ("The desktop studio for Windows" / "Windows
  桌面工作室") is a subordinate tagline, never the headline.
- **Preset names mirror the app.** The nine look names + taglines are copied
  verbatim from the app i18n (`src/lib/i18n/en.ts` / `zh-hans.ts` `Preset_*` keys),
  both locales. Never invent marketing names for in-product objects.
- **Captures are click-to-enlarge.** Every static product capture (style wall,
  zones, studio) opens a fullscreen lightbox — laptop viewers must be able to read
  the icons.
- **Asymmetric layouts everywhere.** Hero 5/7 split; zones and studio sections alternate
  image/text 7/5 splits; centered hero text is banned.
- Banned: left accent bars, handwriting fonts, glassmorphism, neon/mesh gradients,
  skeuomorphic buttons, dark theme, fabricated imagery.

## Information architecture (both locales)

1. **Hero (viewport 1)** — eyebrow, display headline, one sub-paragraph, primary
   Download/Coming-soon CTA + GitHub, the 3D monitor scene with BEFORE/AFTER state chip,
   mono spec strip on the fold line (Windows req · MIT · local-only · snapshot restore).
2. **01 Proof** — before/after wipe over the two real captures, range-input driven
   divider with coral knob.
3. **02 Looks** — style wall: mono-indexed rail of the nine looks driving one large
   real desktop render (scroll chips on mobile).
4. **03 Zones** — real zones-editor screenshot (applied Workbench template) + three
   fact points.
5. **04 Studio** — real icons-studio screenshot + three fact points, mirrored split.
6. **05 Download** — full-bleed flat coral block: dual-state CTA, watch-on-GitHub,
   pending note (or SmartScreen honesty when released), mono requirements line.
7. **FAQ + footer** — four plain-text Q/As (no accordion), hairline footer.

The header nav and footer both carry a **创作历程 / Story** entry to `/story/` — the
making-of page (see its section below).

Copy voice: short, spec-sheet confident, consumer-safe. Chinese copy contains no dashes
and no exclamation marks; zh display headings may carry a manual `\n` break point.

## Imagery contract

All product imagery is real capture, re-shot for this design (2026-07-17 set):
- `assets-src/desktop/*.webp` — before-system-default + nine style renders, same camera,
  2000 px, shot from the app's desktop mirror.
- `assets-src/app/*.webp` — icons studio + zones editor full-window shots.
- The 3D screen textures are 1600×900 WebP crops of the before/squircle renders.
Regenerating: dev server + the capture scripts (session scratchpad `pw/`); any future
re-shoot must go through the real app, never mockups.

## Routing & i18n

- `/` = English, `/zh` = Chinese; both fully static-prerendered.
- **`lib/locales.ts` is the single locale registry** (code, path, hreflang, html lang,
  og:locale, navigator prefixes). Metadata alternates, og locale + alternates, the
  sitemap, JSON-LD language tags, the first-visit redirect script and the shared root
  document (`components/locale-html.tsx`) all derive from it — never hardcode a locale
  ternary elsewhere. Adding a language = one registry entry + `content/<code>.ts`
  (+ its line in `content/index.ts`) + thin `app/(<code>)/<code>/{layout,page}.tsx`
  wrappers mirroring `app/(zh)` (locale-specific display fonts stay in that layout so
  next/font preloads them only there). The single header/footer switch link assumes two
  locales; at three or more replace it with a menu.
- Automatic first-visit locale routing (owner decision 2026-07-17, reversing the earlier
  no-redirect rule): the root page inlines a tiny blocking script (`lib/lang-redirect.ts`)
  that walks `navigator.languages` in preference order — first zh* match routes to `/zh/`
  before paint, first en* match stays. An explicit language-switch click stores
  `dm-lang` in localStorage (`components/lang.tsx`) and always wins afterwards; `/zh/`
  never auto-redirects, so shared links and crawlers keep stable, indexable URLs.
  Header and footer carry the explicit language switch.
- Dictionaries are plain TypeScript objects (`content/{types,en,zh}.ts`); no i18n library.
- `hreflang` alternates on both pages (`en`, `zh-CN`, `x-default` → `/`).
- `/story/` sits OUTSIDE the locale trees (own root layout, `html lang="zh-CN"`,
  canonical only, no alternates) — it is a single-language document, not a locale of
  the landing page. It never auto-redirects and the root redirect never targets it.

## /story/ — the making-of page (创作历程)

- A Chinese-language data-story: the full session analysis of how v1 was built in
  nine days (341 human messages, Claude Code session c69bf900). Content is migrated
  VERBATIM from `docs/session-analysis/speech-dashboard.html` in the owner's
  ai-command-center repo — the analysis copy, quotes, verdict and every dataset are
  preserved unabridged (nothing may be cut when editing this page).
- Data lives in `content/story-data.ts` (machine-extracted from the source JSON —
  re-extract rather than hand-edit numbers) + `content/story.ts` (plain-string copy;
  rich prose is JSX in `components/story/`).
- 12 numbered sections: word cloud (canvas), top-concern bars, sentiment spectrum,
  emotional tide, focus drift, activity heatmap, circadian rhythm, daily arc +
  message length, behavioral signals, five insights, verbatim quotes, the long-form
  article + independent verdict, methodology footer.
- Motion: every number counts up from zero (`components/story/fx.tsx` arms
  server-rendered final states; above-fold plays on load, below-fold on
  intersection); bars/columns/segments grow in; heat cells cascade; the tide chart
  plays like a live stock ticker (clip-window sweep + cursor dot riding the line
  head, radar ping at rest). All of it no-ops under `prefers-reduced-motion`, and
  the final state is complete without JS (crawlers, print).
- Entry points: header nav + footer on both locales, sitemap entry, Article JSON-LD
  (`storyJsonLdScript`), llms.txt link.

## Motion discipline

- No animation libraries. The three.js hero scene is the single scripted signature
  element; everything else is CSS transitions + one IntersectionObserver reveal util
  (arms only below the fold; reveals when intersecting OR already scrolled past).
- Scene lifecycle: lazy dynamic import, RAF pauses when offscreen or tab hidden,
  full dispose on unmount. `?dm3d=before|after|scan` freezes the wipe for visual tests.
- `prefers-reduced-motion: reduce` → no scene; the styled render in a flat frame.
  Any WebGL failure falls back the same way.

## Theming (light / dark / auto)

- Site-wide dark mode (owner request 2026-07-17). Preference model: AUTO follows
  `prefers-color-scheme` by default; an explicit choice is stored in localStorage
  `dm-theme` and stamped pre-paint as `html[data-theme]` by the blocking snippet in
  `lib/theme.ts` (inlined by every root layout — no flash).
- All color tokens live in `app/globals.css` `@theme`; the dark values are two
  IDENTICAL override blocks (system-dark-on-auto + forced-dark) that must stay in
  sync. Every Tailwind color utility and every chart flips with them — components
  never hardcode a themed hex. Raised surfaces use the `--color-card` token
  (white in light, a step above canvas in dark).
- Charts resolve tones at paint time: server markup embeds `var()` / `color-mix()`
  strings; canvas/SVG builders read computed values via
  `components/story/palette.ts#readTones` and re-render on `onThemeChange`
  (data-theme mutations + system scheme changes).
- The control is a sun / monitor / moon segmented radio (`components/theme-toggle.tsx`)
  in both navs. Deliberately fixed colors: the white CTA chip on the coral download
  block and the white wipe divider stay white in both themes.

## Fonts

- Latin: self-hosted Satoshi Variable (≈ 42 KB woff2), `font-display: swap`.
- Chinese body: system stack (`PingFang SC`, `Microsoft YaHei`, `Noto Sans SC`) — 0 bytes.
- Chinese display: build-time MiSans Semibold subsets covering exactly the zh h1–h3
  glyphs (`scripts/subset-zh-font.mjs`; regenerates when `fonts-src/misans/` exists,
  otherwise verifies the committed subsets and fails the build on copy drift). Two
  independent subsets so neither page tree pays for the other: `misans-display-zh`
  (landing, `lib/fonts-zh.ts`) and `misans-display-story` (/story/ headings,
  `lib/fonts-story.ts`).

## Images

- `images.unoptimized: true` (mandatory under export). `scripts/build-images.mjs` (sharp)
  emits AVIF+WebP variants with intrinsic dimensions into gitignored `public/img/` and
  writes `lib/image-manifest.json`; pages use `<picture>` with explicit width/height.
  Hero 3D texture payload is budget-gated (fails the build over 420 KB).

## Download modal (owner request 2026-07-17)

- Every download CTA opens a dialog instead of jumping to the Releases page;
  all downloads inside are DIRECT asset links. Progressive enhancement: the
  server-rendered CTA stays a plain link to releases/latest, upgraded on click
  (`components/download/button.tsx` -> `components/download/modal.tsx`).
- Fully static: `scripts/fetch-release.mjs` bakes the WHOLE release history
  (one `/releases` call, newest first) into `lib/release-data.json`; the modal
  renders the current version prominently and older versions in a collapsed
  history list. No runtime API calls.
- Advisory device detection (`components/download/detect.ts`): UA-CH high
  entropy where available, UA sniffing fallback. Win x64 -> "ready" line;
  ARM / 32-bit / old Windows / macOS / Linux / mobile get honest caution lines
  — the download is NEVER blocked. Windows 11 vs 10 is not claimed.
- Mainland-China mirrors (`RELEASE_MIRRORS` in lib/site.ts): third-party
  public proxies in prefix form, speed-tested before adoption (2026-07-17:
  gh-proxy.org 7.8 MB/s, ghfast.top 2.5 MB/s; ghproxy.net rejected). zh pages
  show them as prominent secondary buttons, en pages as one quiet line. A
  disclaimer marks them unaffiliated; rotate the list here if a proxy dies.
- The SmartScreen walkthrough surfaces only after a download actually starts.

## Download contract (dual state, fully automated)

- Release state is resolved at BUILD TIME from the latest GitHub Release —
  `scripts/fetch-release.mjs` runs first in `prebuild`, writes the gitignored
  `lib/release-data.json` (typed by `lib/release.ts`; `lib/site.ts` derives
  `RELEASE_READY`), and renders `public/llms{,-full}.txt` from
  `scripts/templates/`. **Nothing is flipped by hand, ever.**
  - no published release (API 404, drafts/prereleases ignored): hero + download CTAs
    read "Coming soon"; download block links Watch-on-GitHub. No dead link to an empty
    Releases page; JSON-LD and llms.txt carry no version/download facts.
  - release published: CTAs link `releases/latest` (never a direct .exe URL), the
    SmartScreen honesty note appears, JSON-LD gains softwareVersion/datePublished/
    downloadUrl/releaseNotes, llms files state the version + date, sitemap lastmod
    advances to the release date.
  - fetch failure: local builds keep the previous snapshot (warn); release-driven CI
    builds run with `RELEASE_FETCH_STRICT=1` and fail loudly instead of shipping stale
    facts. `DM_FAKE_RELEASE=1` fakes the released state for local testing only.
- Automation chain: tag push → `release.yml` (self-hosted signed build) publishes the
  Release, then dispatches `website.yml` (releases created with GITHUB_TOKEN cannot
  fire `release:` triggers — GitHub's recursion guard; workflow_dispatch is the
  documented exception). `website.yml` also listens to `release: [published, edited,
  deleted]` for human-published releases, and both paths rebuild + deploy.
- No client-side OS sniffing, no copy-link/mailto widgets (owner removed them in v4).

## SEO / GEO

- Next metadata API: per-locale title/description/canonical/OG/Twitter
  (`summary_large_image`, `social-card.png` with alt), `og:locale` + alternate.
- JSON-LD inline: one `@graph` per page — `Organization` (nicepkg, sameAs GitHub) →
  `WebSite` → `SoftwareApplication` (UtilitiesApplication, Windows 10/11, price 0, MIT,
  publisher by `@id`) → `FAQPage`, joined by stable `@id`s. Release facts (downloadUrl,
  softwareVersion) are emitted ONLY when `RELEASE_READY` is true — never publish a
  version or download link for an empty Releases page. No aggregateRating until real
  ratings exist.
- `app/sitemap.ts` (x-default alternate; `lastModified` from `CONTENT_UPDATED` in
  `lib/site.ts`, a source-maintained content date, never build time) + `app/robots.ts`
  (`force-static`); robots explicitly allows GPTBot, ClaudeBot, Claude-Web,
  PerplexityBot, OAI-SearchBot, Google-Extended and peers.
- Static `public/llms.txt` (short description + key facts + honest release status,
  last-reviewed date) linking `public/llms-full.txt` (bilingual quotation-ready corpus:
  descriptions, features, preset names en/zh, requirements, FAQ, privacy). Keep both in
  sync with `RELEASE_READY`.
- FAQ answers are self-contained (name the product + platform) so LLMs can quote them.
- Branded bilingual 404 via `app/global-not-found.tsx` (noindex, links to both locales);
  Cloudflare `not_found_handling: "404-page"` serves it with a real 404 status.

## Analytics

Cloudflare Web Analytics beacon only (no cookies), injected only when
`NEXT_PUBLIC_CF_BEACON_TOKEN` is set. No GA4.

## Deploy architecture

- `website/` inside this repo, own `package.json` (root has no `workspaces`; keep it so).
- Hosting: Cloudflare **Workers Static Assets** — `wrangler.jsonc` with
  `assets: { directory: "./out" }`, `workers_dev: true`. No Worker script file: requests
  are served from assets and bill nothing. Custom-domain route `dm.xiaominglab.com`
  (`custom_domain: true`; xiaominglab.com is a zone in the account) binds on deploy.
- CI: `.github/workflows/website.yml` — push to main (path-filtered to `website/**`),
  `release: [published, edited, deleted]`, `workflow_dispatch` (used by release.yml
  post-publish), PR preview versions. Builds with bun, deploys with wrangler 4; the
  deploy step reads `CLOUDFLARE_API_TOKEN` + `CLOUDFLARE_ACCOUNT_ID` from the **nicepkg
  org secrets** (visibility: all) — no repo-level copies, so rotating them once in the
  org covers every repo. ⚠️ Org secrets only reach this repo because it is PUBLIC: on
  GitHub Free, "all repositories" means all *public* repositories and private repos
  silently receive an empty string. If the repo ever goes private again, the deploy
  degrades to build-only until repo-level secrets are added.

## Performance budget (acceptance)

- LCP < 1.8 s (Fast 3G lab) — LCP is the static hero copy, not the canvas; the scene
  loads lazily behind it. CLS < 0.05, INP < 200 ms.
- Non-three JS ≤ 220 KB gz total; three.js chunk lazy-loaded only on non-reduced-motion,
  WebGL-capable clients, never blocking first paint. Screen textures ≤ 420 KB combined
  (build-gated).
- Every image has explicit dimensions; fonts preloaded only when used above the fold.

## Acceptance criteria

1. `bun run build` in `website/` produces `out/` with zero server artifacts.
2. Both locales render fully with JS disabled (content, images, FAQ text in static DOM);
   the hero area shows real product imagery via the no-JS/reduced-motion fallback.
3. The 3D monitor renders in viewport 1 at 1600×900 and 390×844; scan wipe, restore
   loop, state chip, pointer parallax and pause-offscreen verified in a real browser.
4. Lighthouse (mobile emulation) meets the performance budget on the deployed URL.
5. `wrangler deploy` from `website/` serves both locales on the workers.dev URL and
   dm.xiaominglab.com.
6. CI deploys on a `website/**` push to main and skips on unrelated pushes.
7. JSON-LD validates; hreflang pair resolves; zh subset build gate passes.
