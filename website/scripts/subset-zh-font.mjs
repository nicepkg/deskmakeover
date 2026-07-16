/**
 * Subsets MiSans Semibold down to exactly the glyphs used by Chinese display
 * headings, emitting app/fonts/misans-display-zh.woff2 (committed, ~15-25 KB)
 * plus a .chars.json sidecar recording the covered glyph set.
 *
 * - With fonts-src/misans present (dev machine): regenerates the subset.
 * - Without it (CI): verifies the committed subset still covers every heading
 *   glyph and fails the build loudly if copy drifted.
 *
 * Run: bun scripts/subset-zh-font.mjs
 */
import { readFile, writeFile, access } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import subsetFont from "subset-font";
import { zh } from "../content/zh.ts";

const here = path.dirname(fileURLToPath(import.meta.url));
const websiteRoot = path.resolve(here, "..");
const sourceTtf = path.join(websiteRoot, "fonts-src/misans/MiSans 开发下载字重/MiSans-Semibold.ttf");
const outWoff2 = path.join(websiteRoot, "app/fonts/misans-display-zh.woff2");
const outChars = path.join(websiteRoot, "app/fonts/misans-display-zh.chars.json");

/** Every string that renders in the zh display face (headings + hero stage UI). */
const displayStrings = [
  zh.hero.headline1,
  zh.hero.headline2,
  zh.hero.putBack,
  zh.hero.beautify,
  zh.hero.stageBefore,
  zh.hero.stageAfter,
  zh.hero.statusRefreshed,
  zh.promise.title,
  zh.promise.toggleStyled,
  zh.promise.toggleOriginal,
  zh.looks.title,
  zh.customize.title,
  ...zh.customize.rows.map((r) => r.title),
  zh.zones.title,
  zh.zones.arrowTitle,
  ...zh.zones.zoneLabels,
  zh.download.title,
  zh.beta.title,
  zh.faq.title,
  zh.footer.tagline,
  ...zh.looks.presets.map((p) => p.name),
];

const chars = [...new Set(displayStrings.join(""))].sort().join("");

async function exists(p) {
  try {
    await access(p);
    return true;
  } catch {
    return false;
  }
}

if (await exists(sourceTtf)) {
  const ttf = await readFile(sourceTtf);
  const woff2 = await subsetFont(ttf, chars, { targetFormat: "woff2" });
  await writeFile(outWoff2, woff2);
  await writeFile(outChars, JSON.stringify({ chars }, null, 2) + "\n");
  console.log(`zh display subset: ${chars.length} glyphs, ${(woff2.length / 1024).toFixed(1)} KB -> app/fonts/misans-display-zh.woff2`);
} else {
  if (!(await exists(outWoff2)) || !(await exists(outChars))) {
    console.error("Missing committed zh display subset and no MiSans source available.");
    console.error("Download MiSans into website/fonts-src/misans and re-run this script.");
    process.exit(1);
  }
  const covered = new Set(JSON.parse(await readFile(outChars, "utf8")).chars);
  const missing = [...chars].filter((c) => !covered.has(c));
  if (missing.length > 0) {
    console.error(`zh display copy uses ${missing.length} glyph(s) not in the committed subset: ${missing.join(" ")}`);
    console.error("Regenerate: download MiSans into website/fonts-src/misans, then `bun scripts/subset-zh-font.mjs`.");
    process.exit(1);
  }
  console.log(`zh display subset verified: ${chars.length} glyphs all covered.`);
}
