/**
 * Build-time image pipeline.
 *
 * Sources (all committed):
 *  - assets-src/desktop/*.webp — 2000px real product renders, same camera:
 *    before-system-default + the nine styles.
 *  - assets-src/app/*.webp — real app screenshots (icons studio, zones editor).
 *  - ../.github/assets — logo/social card only (never mutated).
 *
 * Emits AVIF+WebP variants into public/img/ (gitignored) plus two WebP screen
 * textures for the hero 3D monitor, and writes lib/image-manifest.json.
 * Fails the build if the hero payload exceeds budget.
 */
import { mkdir, readFile, writeFile, copyFile, rm, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const here = path.dirname(fileURLToPath(import.meta.url));
const websiteRoot = path.resolve(here, "..");
const ghAssets = path.resolve(websiteRoot, "../.github/assets");
const desktopSrc = path.join(websiteRoot, "assets-src/desktop");
const appSrc = path.join(websiteRoot, "assets-src/app");
const outDir = path.join(websiteRoot, "public/img");
const manifestPath = path.join(websiteRoot, "lib/image-manifest.json");

const STYLES = [
  "squircle",
  "blueprint",
  "pixel-era",
  "gleam",
  "glaze",
  "die-cut",
  "porthole",
  "scrapbook",
  "creekstone",
];

const SETS = {
  "desk-before": { src: "desktop/before-system-default.webp", widths: [2000, 1200] },
  ...Object.fromEntries(
    STYLES.map((s) => [`desk-${s}`, { src: `desktop/${s}.webp`, widths: [2000, 1200] }]),
  ),
  "studio-icons": { src: "app/studio-icons.webp", widths: [2000, 1200] },
  "studio-zones": { src: "app/studio-zones.webp", widths: [2000, 1200] },
};

/** Screen textures for the 3D monitor: exact 16:9, WebP only. */
const TEXTURES = {
  "tex-before": "desktop/before-system-default.webp",
  "tex-after": "desktop/squircle.webp",
};

const AVIF = { quality: 64 };
const WEBP = { quality: 84 };
/** Hero 3D payload: both screen textures together. */
const HERO_BUDGET_BYTES = 420 * 1024;

async function emitVariants(key, input, widths) {
  const meta = await sharp(input).metadata();
  const srcW = meta.width ?? 0;
  const srcH = meta.height ?? 0;
  const usable = [...new Set(widths.map((w) => Math.min(w, srcW)))].sort((a, b) => b - a);
  const variants = [];
  for (const w of usable) {
    const h = Math.round((srcH / srcW) * w);
    const base = `${key}-${w}`;
    await sharp(input).resize(w).avif(AVIF).toFile(path.join(outDir, `${base}.avif`));
    await sharp(input).resize(w).webp(WEBP).toFile(path.join(outDir, `${base}.webp`));
    variants.push({ w, h, avif: `/img/${base}.avif`, webp: `/img/${base}.webp` });
  }
  return { w: variants[0].w, h: variants[0].h, variants };
}

async function emitTexture(key, src) {
  const input = path.join(websiteRoot, "assets-src", src);
  const out = path.join(outDir, `${key}.webp`);
  await sharp(input).resize(1600, 900, { fit: "cover" }).webp({ quality: 88 }).toFile(out);
  return `/img/${key}.webp`;
}

async function main() {
  await rm(outDir, { recursive: true, force: true });
  await mkdir(outDir, { recursive: true });
  const manifest = {};

  for (const [key, { src, widths }] of Object.entries(SETS)) {
    manifest[key] = await emitVariants(key, path.join(websiteRoot, "assets-src", src), widths);
  }
  const textures = {};
  for (const [key, src] of Object.entries(TEXTURES)) {
    textures[key] = await emitTexture(key, src);
  }
  manifest.__meta = { textures };

  await copyFile(path.join(ghAssets, "social-card.png"), path.join(websiteRoot, "public/social-card.png"));
  await copyFile(path.join(ghAssets, "logo.png"), path.join(websiteRoot, "public/logo.png"));
  // studio HDRI for the hero 3D lighting (Poly Haven studio_small_09, CC0)
  await copyFile(path.join(websiteRoot, "assets-src/env/studio.hdr"), path.join(outDir, "studio.hdr"));

  await writeFile(manifestPath, JSON.stringify(manifest, null, 2) + "\n");

  let heroBytes = 0;
  for (const rel of Object.values(textures)) {
    heroBytes += (await stat(path.join(websiteRoot, "public", rel.slice(1)))).size;
  }
  console.log(`images: ${Object.keys(SETS).length} keys + ${Object.keys(TEXTURES).length} textures emitted`);
  console.log(`hero texture payload: ${(heroBytes / 1024).toFixed(0)} KB (budget ${HERO_BUDGET_BYTES / 1024} KB)`);
  if (heroBytes > HERO_BUDGET_BYTES) {
    console.error("hero payload exceeds budget — lower texture size/quality in scripts/build-images.mjs");
    process.exit(1);
  }
}

await main();
