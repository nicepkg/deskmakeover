import { drawChatTile, drawGearTile, drawNoteTile, makeCanvas } from "../sample-art";
import { MASTER_SIZE } from "./renderer";

export interface SampleDef {
  id: string;
  draw: (ctx: CanvasRenderingContext2D, size: number) => void;
}

/** The built-in, self-made sample set — no third-party marks. */
export const SAMPLES: SampleDef[] = [
  { id: "note", draw: drawNoteTile },
  { id: "chat", draw: drawChatTile },
  { id: "gear", draw: drawGearTile },
];

/** Rasterize a sample drawer to the 256² straight-alpha RGBA the ABI takes. */
export function rasterizeSample(draw: SampleDef["draw"]): Uint8ClampedArray {
  const { ctx } = makeCanvas(MASTER_SIZE);
  draw(ctx, MASTER_SIZE);
  return ctx.getImageData(0, 0, MASTER_SIZE, MASTER_SIZE).data;
}

/** Contain-fit a user image into the 256² master (transparent padding). */
export function rasterizeUserImage(bitmap: ImageBitmap): Uint8ClampedArray {
  const { canvas, ctx } = makeCanvas(MASTER_SIZE);
  ctx.clearRect(0, 0, MASTER_SIZE, MASTER_SIZE);
  const scale = Math.min(MASTER_SIZE / bitmap.width, MASTER_SIZE / bitmap.height);
  const w = Math.max(1, Math.round(bitmap.width * scale));
  const h = Math.max(1, Math.round(bitmap.height * scale));
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(bitmap, (MASTER_SIZE - w) / 2, (MASTER_SIZE - h) / 2, w, h);
  void canvas;
  return ctx.getImageData(0, 0, MASTER_SIZE, MASTER_SIZE).data;
}
