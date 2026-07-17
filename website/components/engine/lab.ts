/**
 * The page-wide engine lab: ONE wasm renderer + the real icon cast, shared by
 * every scene and the playground. All scene artwork on /engine/ comes out of
 * this — real Windows icons rendered by the shipping pipeline, never mockups.
 */
import { EngineRenderer, MASTER_SIZE, type PlaygroundConfig } from "./playground/renderer";

export interface CastIcon {
  id: string;
  /** served asset under public/engine/icons/ */
  url: string;
}

/** The real-icon cast (Windows system + first-party icons from the app's own
 *  fixture pack — no hand-drawn stand-ins). */
export const CAST: CastIcon[] = [
  { id: "folder", url: "/engine/icons/folder.png" },
  { id: "pics", url: "/engine/icons/pics.png" },
  { id: "bin", url: "/engine/icons/bin.png" },
  { id: "thispc", url: "/engine/icons/thispc.png" },
  { id: "camera", url: "/engine/icons/camera.png" },
  { id: "mail", url: "/engine/icons/mail.png" },
  { id: "maps", url: "/engine/icons/maps.png" },
  { id: "panel", url: "/engine/icons/panel.png" },
];

/** A fully transparent master — registering it renders the PLATE alone. */
export const PLATE_ONLY_ID = "__plate__";

export interface Lab {
  renderer: EngineRenderer;
  /** raw 256² straight-alpha RGBA per cast id (the decoded artwork) */
  raw: Map<string, Uint8ClampedArray>;
  /** decode-time hue seed per cast id ("#RRGGBB" or null) */
  seeds: Map<string, string | null>;
}

let labPromise: Promise<Lab> | null = null;

async function fetchIconRGBA(url: string): Promise<Uint8ClampedArray> {
  const blob = await (await fetch(url)).blob();
  const bitmap = await createImageBitmap(blob);
  const canvas = document.createElement("canvas");
  canvas.width = MASTER_SIZE;
  canvas.height = MASTER_SIZE;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) throw new Error("2d context unavailable");
  const scale = Math.min(MASTER_SIZE / bitmap.width, MASTER_SIZE / bitmap.height);
  const w = Math.max(1, Math.round(bitmap.width * scale));
  const h = Math.max(1, Math.round(bitmap.height * scale));
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(bitmap, (MASTER_SIZE - w) / 2, (MASTER_SIZE - h) / 2, w, h);
  bitmap.close();
  return ctx.getImageData(0, 0, MASTER_SIZE, MASTER_SIZE).data;
}

/** Boot (once) the wasm engine and register the whole cast. */
export function getLab(): Promise<Lab> {
  if (!labPromise) {
    labPromise = (async () => {
      if (typeof WebAssembly !== "object") throw new Error("no wasm");
      const renderer = await EngineRenderer.create();
      const raw = new Map<string, Uint8ClampedArray>();
      const seeds = new Map<string, string | null>();
      const buffers = await Promise.all(CAST.map((c) => fetchIconRGBA(c.url)));
      CAST.forEach((c, i) => {
        raw.set(c.id, buffers[i]);
        renderer.registerSource(c.id, buffers[i]);
        seeds.set(c.id, renderer.seedOf(c.id));
      });
      renderer.registerSource(PLATE_ONLY_ID, new Uint8ClampedArray(MASTER_SIZE * MASTER_SIZE * 4));
      return { renderer, raw, seeds };
    })().catch((e) => {
      labPromise = null; // allow a retry on transient fetch failures
      throw e;
    });
  }
  return labPromise;
}

/** The factory-default look every scene starts from. */
export function baseConfig(): PlaygroundConfig {
  return {
    shape: "Apple",
    subject: "Original",
    monoStyle: "Tonal",
    plateBand: "Vivid",
    distinction: "None",
    markStyle: "Glass",
    filter: "None",
    plateFallback: "derived",
    shortcutShape: null,
    markColor: null,
    plateColor: null,
    autoSeparation: true,
    tint: "#ff6f5e",
  };
}

/** Render a cast tile to ImageData (256²). */
export function renderTile(
  lab: Lab,
  id: string,
  patch: Partial<PlaygroundConfig> = {},
  showOriginal = false,
): ImageData | null {
  const rgba = lab.renderer.render(id, { ...baseConfig(), ...patch }, showOriginal, MASTER_SIZE);
  return rgba ? new ImageData(rgba, MASTER_SIZE, MASTER_SIZE) : null;
}

/** Draw raw cast artwork (the untouched icon) to ImageData. */
export function rawImage(lab: Lab, id: string): ImageData | null {
  const raw = lab.raw.get(id);
  return raw ? new ImageData(raw.slice(), MASTER_SIZE, MASTER_SIZE) : null;
}

/** A white edge-outline texture computed from the artwork's alpha channel. */
export function outlineFromAlpha(img: ImageData): ImageData {
  const w = img.width;
  const h = img.height;
  const out = new ImageData(w, h);
  const solid = (x: number, y: number) =>
    x >= 0 && y >= 0 && x < w && y < h && img.data[(y * w + x) * 4 + 3] > 96;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (!solid(x, y)) continue;
      const edge = !solid(x - 1, y) || !solid(x + 1, y) || !solid(x, y - 1) || !solid(x, y + 1);
      if (!edge) continue;
      // a 2px-thick white edge so the lifted layer reads at scene scale
      for (let dy = -1; dy <= 1; dy++) {
        for (let dx = -1; dx <= 1; dx++) {
          const i = ((y + dy) * w + (x + dx)) * 4;
          if (i < 0 || i >= out.data.length) continue;
          out.data[i] = 255;
          out.data[i + 1] = 255;
          out.data[i + 2] = 255;
          out.data[i + 3] = 235;
        }
      }
    }
  }
  return out;
}

/** Split a cast icon into background / artwork layers using the oracle's
 *  real subject mask (white = subject), contain-fitted like the raw asset. */
export async function maskLayers(
  lab: Lab,
  id: string,
  maskUrl: string,
): Promise<{ bgLayer: ImageData; artLayer: ImageData } | null> {
  const raw = lab.raw.get(id);
  if (!raw) return null;
  const blob = await (await fetch(maskUrl)).blob();
  const bmp = await createImageBitmap(blob);
  const c = document.createElement("canvas");
  c.width = c.height = MASTER_SIZE;
  const ctx = c.getContext("2d", { willReadFrequently: true });
  if (!ctx) return null;
  ctx.imageSmoothingEnabled = false;
  const scale = Math.min(MASTER_SIZE / bmp.width, MASTER_SIZE / bmp.height);
  const w = Math.round(bmp.width * scale);
  const h = Math.round(bmp.height * scale);
  ctx.drawImage(bmp, (MASTER_SIZE - w) / 2, (MASTER_SIZE - h) / 2, w, h);
  bmp.close();
  const m = ctx.getImageData(0, 0, MASTER_SIZE, MASTER_SIZE).data;
  const bgLayer = new ImageData(MASTER_SIZE, MASTER_SIZE);
  const artLayer = new ImageData(MASTER_SIZE, MASTER_SIZE);
  for (let i = 0; i < MASTER_SIZE * MASTER_SIZE; i++) {
    const opaque = raw[i * 4 + 3] > 24;
    if (!opaque) continue;
    const target = m[i * 4] > 127 ? artLayer : bgLayer;
    target.data[i * 4] = raw[i * 4];
    target.data[i * 4 + 1] = raw[i * 4 + 1];
    target.data[i * 4 + 2] = raw[i * 4 + 2];
    target.data[i * 4 + 3] = raw[i * 4 + 3];
  }
  return { bgLayer, artLayer };
}

/** on − off: the pixels a render pass ADDED (the rescue outline + shadow), alone. */
export function diffLayer(on: ImageData, off: ImageData): ImageData {
  const out = new ImageData(on.width, on.height);
  for (let i = 0; i < on.data.length; i += 4) {
    const d =
      Math.abs(on.data[i] - off.data[i]) +
      Math.abs(on.data[i + 1] - off.data[i + 1]) +
      Math.abs(on.data[i + 2] - off.data[i + 2]) +
      Math.abs(on.data[i + 3] - off.data[i + 3]);
    if (d > 24) {
      out.data[i] = on.data[i];
      out.data[i + 1] = on.data[i + 1];
      out.data[i + 2] = on.data[i + 2];
      out.data[i + 3] = on.data[i + 3];
    }
  }
  return out;
}
