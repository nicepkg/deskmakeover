/**
 * Build-time image pipeline (dark-cinema redesign).
 *
 * Sources (all committed):
 *  - assets-src/desktop/*.webp — 2000px real product renders (before + nine
 *    styles, same camera) + cells.json / featured.json (icon-cell geometry
 *    detected by diffing frames; regenerate locally if the shots change).
 *  - assets-src/app/*.webp — clean full-window app shots (no baked chrome).
 *  - ../.github/assets — logo/social card only (never mutated).
 *
 * Emits AVIF+WebP variants into public/img/ (gitignored), crops the nine
 * style-chip icons, and writes lib/image-manifest.json (incl. cell geometry
 * for the hero theater). Fails the build if the hero payload exceeds budget.
 */
import { mkdir, readFile, writeFile, copyFile, rm } from "node:fs/promises";
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

const HERO_FRAMES = {
  "desk-before": { src: "desktop/before-system-default.webp", widths: [2000, 1200] },
  "desk-squircle": { src: "desktop/squircle.webp", widths: [2000, 1200] },
};

const APP_SHOTS = {
  "studio-icons": { src: "app/studio-icons.webp", widths: [2000, 1200] },
  "studio-wallpaper": { src: "app/studio-wallpaper.webp", widths: [2000, 1200] },
};

const AVIF = { quality: 64 };
const WEBP = { quality: 84 };
const HERO_BUDGET_BYTES = 500 * 1024;

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

async function emitChip(style, chipCell) {
  const input = path.join(desktopSrc, `${style === "before" ? "before-system-default" : style}.webp`);
  const meta = await sharp(input).metadata();
  const left = Math.round((chipCell.x / 100) * meta.width);
  const top = Math.round((chipCell.y / 100) * meta.height);
  const width = Math.round((chipCell.w / 100) * meta.width);
  const height = Math.round((chipCell.h / 100) * meta.height);
  const base = `chip-${style}`;
  const square = Math.min(width, height);
  await sharp(input)
    .extract({ left, top, width, height })
    .resize(square < 144 ? undefined : 144, square < 144 ? undefined : 144, { fit: "cover" })
    .avif({ quality: 70 })
    .toFile(path.join(outDir, `${base}.avif`));
  await sharp(input)
    .extract({ left, top, width, height })
    .resize(square < 144 ? undefined : 144, square < 144 ? undefined : 144, { fit: "cover" })
    .webp({ quality: 88 })
    .toFile(path.join(outDir, `${base}.webp`));
  return { avif: `/img/${base}.avif`, webp: `/img/${base}.webp` };
}

async function fileSize(publicPath) {
  return (await readFile(path.join(websiteRoot, "public", publicPath.slice(1)))).length;
}

async function main() {
  await rm(outDir, { recursive: true, force: true });
  await mkdir(outDir, { recursive: true });
  const manifest = {};

  for (const [key, { src, widths }] of Object.entries(HERO_FRAMES)) {
    manifest[key] = await emitVariants(key, path.join(websiteRoot, "assets-src", src), widths);
  }
  for (const style of STYLES.filter((s) => s !== "squircle")) {
    manifest[`desk-${style}`] = await emitVariants(`desk-${style}`, path.join(desktopSrc, `${style}.webp`), [1600]);
  }
  for (const [key, { src, widths }] of Object.entries(APP_SHOTS)) {
    manifest[key] = await emitVariants(key, path.join(websiteRoot, "assets-src", src), widths);
  }

  const geometry = JSON.parse(await readFile(path.join(desktopSrc, "featured.json"), "utf8"));
  const allCells = JSON.parse(await readFile(path.join(desktopSrc, "cells.json"), "utf8"));
  const chips = {};
  for (const style of STYLES) {
    chips[style] = await emitChip(style, geometry.chipCell);
  }
  manifest.__meta = { chips, featured: geometry.featured, cells: allCells.cells };

  await copyFile(path.join(ghAssets, "social-card.png"), path.join(websiteRoot, "public/social-card.png"));
  await copyFile(path.join(ghAssets, "logo.png"), path.join(websiteRoot, "public/logo.png"));

  await writeFile(manifestPath, JSON.stringify(manifest, null, 2) + "\n");

  const heroBytes =
    (await fileSize(manifest["desk-before"].variants[0].avif)) +
    (await fileSize(manifest["desk-squircle"].variants[0].avif));
  console.log(`images: ${Object.keys(manifest).length - 1} keys + 9 chips emitted`);
  console.log(`hero delivered pair (largest avif): ${(heroBytes / 1024).toFixed(0)} KB (budget ${HERO_BUDGET_BYTES / 1024} KB)`);
  if (heroBytes > HERO_BUDGET_BYTES) {
    console.error("hero payload exceeds budget — lower widths/quality in scripts/build-images.mjs");
    process.exit(1);
  }
}

await main();
