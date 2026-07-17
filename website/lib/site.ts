import { RELEASE } from "@/lib/release";

export const SITE_URL = "https://dm.xiaominglab.com";

/** Bump when page copy/facts materially change; feeds sitemap lastmod + llms.txt. */
export const CONTENT_UPDATED = "2026-07-17";
export const GITHUB_URL = "https://github.com/nicepkg/deskmakeover";
export const RELEASES_LATEST_URL = `${GITHUB_URL}/releases/latest`;

/**
 * Download CTA dual state, resolved at BUILD TIME from the latest GitHub
 * Release (scripts/fetch-release.mjs -> lib/release.ts). Publishing a release
 * rebuilds and redeploys the site automatically — never flip this by hand.
 */
export const RELEASE_READY = RELEASE.ready;

export const DOWNLOAD_URL = RELEASE_READY ? RELEASES_LATEST_URL : GITHUB_URL;

/** Newest of the hand-maintained content date and the latest release date. */
export const LAST_CONTENT_DATE =
  RELEASE.publishedAt && RELEASE.publishedAt > CONTENT_UPDATED
    ? RELEASE.publishedAt
    : CONTENT_UPDATED;

/** Cloudflare Web Analytics beacon token; beacon is omitted when unset. */
export const CF_BEACON_TOKEN = process.env.NEXT_PUBLIC_CF_BEACON_TOKEN ?? "";
