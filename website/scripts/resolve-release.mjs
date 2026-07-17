// Refreshes lib/release.json from the GitHub Releases API at build time, so
// publishing a release updates the site on the next CI build with zero manual
// edits (release facts are baked into the static export; no Worker logic).
//
// Rules:
// - newest non-draft release wins (prereleases count: the beta IS the launch)
// - no releases yet          -> { ready: false } (the honest default)
// - API/network failure      -> keep the committed snapshot and warn; a flaky
//   api.github.com must neither break deploys nor silently un-release
// - file is only rewritten when the facts change, so local builds stay clean
import { readFileSync, writeFileSync } from "node:fs";

const REPO = "nicepkg/deskmakeover";
const OUT = new URL("../lib/release.json", import.meta.url);

const headers = { "user-agent": "deskmakeover-site-build" };
if (process.env.GITHUB_TOKEN) headers.authorization = `Bearer ${process.env.GITHUB_TOKEN}`;

let next = null;
try {
  const res = await fetch(`https://api.github.com/repos/${REPO}/releases?per_page=15`, { headers });
  if (res.status === 404) {
    next = { ready: false, tag: null, version: null, publishedAt: null, url: null };
  } else if (!res.ok) {
    throw new Error(`GitHub API ${res.status}`);
  } else {
    const releases = (await res.json()).filter((r) => !r.draft);
    if (releases.length === 0) {
      next = { ready: false, tag: null, version: null, publishedAt: null, url: null };
    } else {
      const r = releases[0];
      next = {
        ready: true,
        tag: r.tag_name,
        version: r.tag_name.replace(/^v/, ""),
        publishedAt: (r.published_at ?? "").slice(0, 10) || null,
        url: r.html_url,
      };
    }
  }
} catch (err) {
  console.warn(`resolve-release: keeping committed snapshot (${err.message})`);
  process.exit(0);
}

const nextText = `${JSON.stringify(next, null, 2)}\n`;
let current = "";
try {
  current = readFileSync(OUT, "utf8");
} catch {
  /* first run */
}
if (current !== nextText) {
  writeFileSync(OUT, nextText);
  console.log(`resolve-release: ${next.ready ? `release ${next.tag} (${next.publishedAt})` : "no release yet"}`);
} else {
  console.log("resolve-release: unchanged");
}
