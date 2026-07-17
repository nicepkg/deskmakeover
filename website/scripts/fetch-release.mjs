/**
 * Resolve the latest GitHub Release at BUILD TIME and bake its facts into the
 * site, so shipping a release never requires touching the website by hand:
 *
 *   - writes lib/release-data.json (gitignored) — lib/release.ts types it and
 *     lib/site.ts derives RELEASE_READY / download state from it;
 *   - renders public/llms.txt + public/llms-full.txt (gitignored) from
 *     scripts/templates/, injecting the honest release status either way.
 *
 * States:
 *   HTTP 200  -> ready: true, with version/tag/date/installer facts
 *   HTTP 404  -> ready: false (no release yet — authoritative, not an error)
 *   error     -> RELEASE_FETCH_STRICT=1 fails the build (release-triggered CI);
 *                otherwise keeps the previous snapshot if one exists, else
 *                falls back to ready: false with a loud warning.
 *
 * Env: GITHUB_TOKEN (rate limits / private repo), RELEASE_FETCH_STRICT=1,
 *      DM_FAKE_RELEASE=1 (local dual-state testing — never in CI).
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const OUT_JSON = path.join(ROOT, "lib", "release-data.json");
const TEMPLATES = path.join(ROOT, "scripts", "templates");
const REPO = "nicepkg/deskmakeover";
const RELEASES_URL = `https://github.com/${REPO}/releases`;

const strict = process.env.RELEASE_FETCH_STRICT === "1";

function contentUpdated() {
  const site = readFileSync(path.join(ROOT, "lib", "site.ts"), "utf8");
  const m = site.match(/CONTENT_UPDATED = "(\d{4}-\d{2}-\d{2})"/);
  if (!m) throw new Error("CONTENT_UPDATED not found in lib/site.ts");
  return m[1];
}

async function fetchLatestRelease() {
  if (process.env.DM_FAKE_RELEASE === "1") {
    console.warn("[fetch-release] DM_FAKE_RELEASE=1 — using a FAKE release (testing only)");
    return {
      ready: true,
      version: "0.1.0",
      tag: "v0.1.0",
      publishedAt: "2026-07-20",
      notesUrl: `${RELEASES_URL}/tag/v0.1.0`,
      installer: { name: "DeskMakeover_0.1.0_x64-setup.exe", url: `${RELEASES_URL}/download/v0.1.0/DeskMakeover_0.1.0_x64-setup.exe`, sizeMB: 12.4 },
    };
  }
  const headers = { accept: "application/vnd.github+json", "user-agent": "deskmakeover-website-build" };
  if (process.env.GITHUB_TOKEN) headers.authorization = `Bearer ${process.env.GITHUB_TOKEN}`;
  const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, { headers });
  if (res.status === 404) return { ready: false }; // no releases yet — a fact, not a failure
  if (!res.ok) throw new Error(`GitHub API ${res.status}: ${(await res.text()).slice(0, 200)}`);
  const rel = await res.json();
  if (rel.draft || rel.prerelease) return { ready: false }; // only full releases count
  const asset = (rel.assets ?? []).find((a) => /-setup\.exe$|\.msi$/i.test(a.name));
  return {
    ready: true,
    version: String(rel.tag_name).replace(/^v/, ""),
    tag: rel.tag_name,
    publishedAt: String(rel.published_at).slice(0, 10),
    notesUrl: rel.html_url,
    ...(asset
      ? { installer: { name: asset.name, url: asset.browser_download_url, sizeMB: Math.round((asset.size / 1048576) * 10) / 10 } }
      : {}),
  };
}

function renderLlms(release, updated) {
  const lastReviewed =
    release.ready && release.publishedAt > updated ? release.publishedAt : updated;
  const statusEn = release.ready
    ? `Release status: ${release.tag} released ${release.publishedAt}. Download the installer from ${RELEASES_URL}/latest.`
    : `Release status: beta, pre-release. No public installer is available yet as of ${lastReviewed}; the first release will ship via GitHub Releases.`;
  const releasesLine = release.ready
    ? `- [Releases](${RELEASES_URL}): installers and release notes (latest: ${release.tag}, ${release.publishedAt})`
    : `- [Releases](${RELEASES_URL}): future installers and release notes; empty until the first release lands`;
  const availabilityEn = release.ready
    ? `- Release status: ${release.tag}, released ${release.publishedAt}. Installer: ${RELEASES_URL}/latest${release.installer ? ` (${release.installer.name}, ${release.installer.sizeMB} MB)` : ""}.`
    : `- Release status: beta, pre-release. No public installer is available yet as of\n  ${lastReviewed}. The first installer will ship at\n  ${RELEASES_URL} (empty until then).`;
  const availabilityZh = release.ready
    ? `已发布 ${release.tag}（${release.publishedAt}），安装包见 ${RELEASES_URL}/latest。`
    : `Beta 预发布阶段，截至 ${lastReviewed} 暂无公开安装包，首个安装包将发布在 GitHub Releases。`;

  for (const name of ["llms.txt", "llms-full.txt"]) {
    const tpl = readFileSync(path.join(TEMPLATES, name), "utf8");
    const rendered = tpl
      .replaceAll("__RELEASE_STATUS_EN__", statusEn)
      .replaceAll("__RELEASES_LINE__", releasesLine)
      .replaceAll("__AVAILABILITY_EN__", availabilityEn)
      .replaceAll("__AVAILABILITY_ZH__", availabilityZh)
      .replaceAll("__LAST_REVIEWED__", lastReviewed);
    if (rendered.includes("__")) {
      const leftover = rendered.match(/__[A-Z_]+__/g);
      if (leftover) throw new Error(`${name}: unrendered placeholders ${leftover.join(", ")}`);
    }
    writeFileSync(path.join(ROOT, "public", name), rendered);
  }
}

let release;
try {
  release = await fetchLatestRelease();
} catch (err) {
  if (strict) {
    console.error(`[fetch-release] FAILED (strict): ${err.message}`);
    process.exit(1);
  }
  if (existsSync(OUT_JSON)) {
    console.warn(`[fetch-release] WARN: ${err.message} — keeping the previous snapshot`);
    release = JSON.parse(readFileSync(OUT_JSON, "utf8"));
  } else {
    console.warn(`[fetch-release] WARN: ${err.message} — falling back to ready:false`);
    release = { ready: false };
  }
}

mkdirSync(path.dirname(OUT_JSON), { recursive: true });
writeFileSync(OUT_JSON, JSON.stringify(release, null, 2) + "\n");
renderLlms(release, contentUpdated());
console.log(
  release.ready
    ? `[fetch-release] ${release.tag} (${release.publishedAt}) — download CTA live`
    : "[fetch-release] no published release — coming-soon state"
);
