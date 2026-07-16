/**
 * Build-time image pipeline.
 *
 * Reads brand assets from ../.github/assets (never mutated), extracts the hero
 * before/after frames embedded in hero-beforeafter.svg, and emits AVIF + WebP
 * variants with intrinsic dimensions into public/img/ (gitignored). Writes
 * lib/image-manifest.json consumed by the <Pic> component.
 *
 * Run: bun scripts/build-images.mjs   (wired as prebuild)
 */
import { mkdir, readFile, writeFile, copyFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const here = path.dirname(fileURLToPath(import.meta.url));
const websiteRoot = path.resolve(here, "..");
const assetsDir = path.resolve(websiteRoot, "../.github/assets");
const outDir = path.join(websiteRoot, "public/img");
const manifestPath = path.join(websiteRoot, "lib/image-manifest.json");

/** key -> { src, widths } — widths are output candidates, capped at source width. */
const STILLS = {
  "specimen-nine-styles": { src: "specimen-nine-styles.webp", widths: [1760, 880] },
  "preset-squircle": { src: "preset-squircle.webp", widths: [500] },
  "preset-blueprint": { src: "preset-blueprint.webp", widths: [500] },
  "preset-pixel-era": { src: "preset-pixel-era.webp", widths: [500] },
  "preset-gleam": { src: "preset-gleam.webp", widths: [500] },
  "preset-glaze": { src: "preset-glaze.webp", widths: [500] },
  "preset-die-cut": { src: "preset-die-cut.webp", widths: [500] },
  "preset-porthole": { src: "preset-porthole.webp", widths: [500] },
  "preset-scrapbook": { src: "preset-scrapbook.webp", widths: [500] },
  "preset-creekstone": { src: "preset-creekstone.webp", widths: [500] },
  "feature-combine": { src: "feature-combine.webp", widths: [1520, 760] },
  "feature-zones": { src: "feature-zones.webp", widths: [1520, 760] },
  "feature-stylepack": { src: "feature-stylepack.webp", widths: [880] },
  "app-studio": { src: "app-studio.webp", widths: [1760, 880] },
};

const AVIF = { quality: 62 };
const WEBP = { quality: 84 };

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

async function extractHeroFrames() {
  const svg = await readFile(path.join(assetsDir, "hero-beforeafter.svg"), "utf8");
  const frames = {};
  for (const id of ["imgBefore", "imgAfter"]) {
    const m = svg.match(new RegExp(`<image id="${id}" href="data:image/jpeg;base64,([^"]+)"`));
    if (!m) throw new Error(`hero frame ${id} not found in hero-beforeafter.svg`);
    frames[id] = Buffer.from(m[1], "base64");
  }
  return frames;
}

async function main() {
  await mkdir(outDir, { recursive: true });
  const manifest = {};

  const { imgBefore, imgAfter } = await extractHeroFrames();
  manifest["hero-before"] = await emitVariants("hero-before", imgBefore, [1600, 800]);
  manifest["hero-after"] = await emitVariants("hero-after", imgAfter, [1600, 800]);

  for (const [key, { src, widths }] of Object.entries(STILLS)) {
    manifest[key] = await emitVariants(key, path.join(assetsDir, src), widths);
  }

  await copyFile(path.join(assetsDir, "social-card.png"), path.join(websiteRoot, "public/social-card.png"));
  await copyFile(path.join(assetsDir, "logo.png"), path.join(websiteRoot, "public/logo.png"));

  await writeFile(manifestPath, JSON.stringify(manifest, null, 2) + "\n");

  const total = Object.keys(manifest).length;
  const heroBytes = await Promise.all(
    ["hero-before", "hero-after"].flatMap((k) =>
      manifest[k].variants.map(async (v) => (await readFile(path.join(websiteRoot, "public", v.avif.slice(1)))).length),
    ),
  );
  console.log(`images: ${total} keys emitted to public/img/`);
  console.log(`hero avif payload (all variants): ${(heroBytes.reduce((a, b) => a + b, 0) / 1024).toFixed(0)} KB`);
}

await main();
