# DeskMakeover website (dm.nicepkg.cn)

Bilingual static landing page: `/` English, `/zh/` Chinese. Next.js 16
(`output: 'export'`) + Tailwind v4, deployed to Cloudflare Workers Static
Assets. Spec: [`docs/specs/10-website.md`](../docs/specs/10-website.md).

## Develop

```bash
bun install
bun run dev        # regenerates images + verifies fonts, then next dev
bun run build      # static export into out/
bun run typecheck
```

## How it fits together

- `content/{en,zh}.ts` — every visible string, typed by `content/types.ts`.
  Chinese copy contains no dashes and no exclamation marks (brand rule).
- `scripts/build-images.mjs` — prebuild; reads brand assets from
  `../.github/assets` (never mutates them), extracts the hero before/after
  frames from `hero-beforeafter.svg`, emits AVIF/WebP variants into
  `public/img/` (gitignored) and writes `lib/image-manifest.json`.
- `scripts/subset-zh-font.mjs` — prebuild; verifies the committed zh
  display-font subset (`app/fonts/misans-display-zh.woff2`) covers every
  heading glyph. After changing zh headings, download MiSans into
  `fonts-src/misans/` and re-run the script to regenerate the subset.
- `lib/site.ts` — `RELEASE_READY` flips the Download CTA from
  "watch the release" to `releases/latest` once the first installer ships.
- `app/(en)/` + `app/(zh)/` — two root layouts (correct `<html lang>` per
  locale); shared sections live in `components/sections/`.

## Deploy

```bash
bun run deploy     # build + wrangler deploy (Workers Static Assets)
```

CI (`.github/workflows/website.yml`) deploys on every push to main touching
`website/**` or `.github/assets/**`, using the org-level
`CLOUDFLARE_API_TOKEN` / `CLOUDFLARE_ACCOUNT_ID` secrets. Pull requests build
and upload a preview version.

Custom domain `dm.nicepkg.cn` requires the `nicepkg.cn` zone in the same
Cloudflare account; the `routes` block in `wrangler.jsonc` is commented until
the zone lands (see comment there). Until then the site serves from
`deskmakeover-site.<subdomain>.workers.dev`.
