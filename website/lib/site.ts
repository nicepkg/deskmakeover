export const SITE_URL = "https://dm.xiaominglab.com";

/** Bump when page copy/facts materially change; feeds sitemap lastmod + llms.txt. */
export const CONTENT_UPDATED = "2026-07-17";
export const GITHUB_URL = "https://github.com/nicepkg/deskmakeover";
export const RELEASES_LATEST_URL = `${GITHUB_URL}/releases/latest`;

/**
 * Download CTA dual state. Flip to `true` once the first installer is published
 * on GitHub Releases, then redeploy — nothing else needs to change.
 */
export const RELEASE_READY = false;

export const DOWNLOAD_URL = RELEASE_READY ? RELEASES_LATEST_URL : GITHUB_URL;

/** Cloudflare Web Analytics beacon token; beacon is omitted when unset. */
export const CF_BEACON_TOKEN = process.env.NEXT_PUBLIC_CF_BEACON_TOKEN ?? "";
