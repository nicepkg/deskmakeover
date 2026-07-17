/**
 * Subsets MiSans Semibold down to exactly the glyphs used by Chinese display
 * headings, emitting one committed woff2 per page tree (each with a
 * .chars.json sidecar recording the covered glyph set):
 *
 *   app/fonts/misans-display-zh.woff2     — the /zh/ landing headings
 *   app/fonts/misans-display-story.woff2  — the /story/ headings (larger set,
 *                                           loaded only on /story/)
 *
 * - With fonts-src/misans present (dev machine): regenerates the subsets.
 * - Without it (CI): verifies each committed subset still covers every
 *   heading glyph and fails the build loudly if copy drifted.
 *
 * Run: bun scripts/subset-zh-font.mjs
 */
import { readFile, writeFile, access } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import subsetFont from "subset-font";
import { zh } from "../content/zh.ts";
import { ENGINE_ZH_DISPLAY_STRINGS } from "../content/engine-zh.ts";
import { STORY_DISPLAY_STRINGS } from "../content/story.ts";

const here = path.dirname(fileURLToPath(import.meta.url));
const websiteRoot = path.resolve(here, "..");
const sourceTtf = path.join(websiteRoot, "fonts-src/misans/MiSans 开发下载字重/MiSans-Semibold.ttf");

/** Every string that renders in the zh display face (h1/h2/h3 only). */
const landingStrings = [
  zh.hero.title,
  zh.proof.title,
  zh.looks.title,
  zh.zones.title,
  zh.studio.title,
  zh.engineBand.title,
  zh.download.title,
  zh.faq.title,
  ...zh.faq.items.map((i) => i.q),
  zh.footer.tagline,
  // /zh/engine/ lives in the same (zh) tree and shares this face
  ...ENGINE_ZH_DISPLAY_STRINGS,
];

const SUBSETS = [
  { name: "landing", strings: landingStrings, out: "misans-display-zh" },
  { name: "story", strings: STORY_DISPLAY_STRINGS, out: "misans-display-story" },
];

async function exists(p) {
  try {
    await access(p);
    return true;
  } catch {
    return false;
  }
}

const haveSource = await exists(sourceTtf);
const ttf = haveSource ? await readFile(sourceTtf) : null;

for (const subset of SUBSETS) {
  const chars = [...new Set(subset.strings.join(""))].sort().join("");
  const outWoff2 = path.join(websiteRoot, `app/fonts/${subset.out}.woff2`);
  const outChars = path.join(websiteRoot, `app/fonts/${subset.out}.chars.json`);

  if (haveSource) {
    const woff2 = await subsetFont(ttf, chars, { targetFormat: "woff2" });
    await writeFile(outWoff2, woff2);
    await writeFile(outChars, JSON.stringify({ chars }, null, 2) + "\n");
    console.log(
      `zh display subset [${subset.name}]: ${chars.length} glyphs, ${(woff2.length / 1024).toFixed(1)} KB -> app/fonts/${subset.out}.woff2`
    );
  } else {
    if (!(await exists(outWoff2)) || !(await exists(outChars))) {
      console.error(`Missing committed subset app/fonts/${subset.out}.woff2 and no MiSans source available.`);
      console.error("Download MiSans into website/fonts-src/misans and re-run this script.");
      process.exit(1);
    }
    const covered = new Set(JSON.parse(await readFile(outChars, "utf8")).chars);
    const missing = [...chars].filter((c) => !covered.has(c));
    if (missing.length > 0) {
      console.error(
        `zh display copy [${subset.name}] uses ${missing.length} glyph(s) not in the committed subset: ${missing.join(" ")}`
      );
      console.error("Regenerate: download MiSans into website/fonts-src/misans, then `bun scripts/subset-zh-font.mjs`.");
      process.exit(1);
    }
    console.log(`zh display subset [${subset.name}] verified: ${chars.length} glyphs all covered.`);
  }
}
