import { MASTER_SIZE } from "./renderer";

/** Contain-fit a user image into the 256² straight-alpha master the ABI takes. */
export function rasterizeUserImage(bitmap: ImageBitmap): Uint8ClampedArray {
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
  return ctx.getImageData(0, 0, MASTER_SIZE, MASTER_SIZE).data;
}
