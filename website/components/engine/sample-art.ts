/**
 * Deterministic, self-made sample "icons" for the /engine/ scenes and
 * playground — no third-party marks anywhere. Each drawer paints straight
 * onto a 2D context at the given size; colours are fixed artwork colours
 * (icons are content, not chrome, so they do not flip with the theme).
 */

function roundedRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
) {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
}

/** A teal music-app tile with a white note glyph (transparent-edged icon). */
export function drawNoteTile(ctx: CanvasRenderingContext2D, s: number) {
  ctx.clearRect(0, 0, s, s);
  const pad = s * 0.08;
  roundedRect(ctx, pad, pad, s - pad * 2, s - pad * 2, s * 0.2);
  ctx.fillStyle = "#128577";
  ctx.fill();
  // eighth-note glyph
  ctx.fillStyle = "#ffffff";
  const nx = s * 0.42;
  const ny = s * 0.62;
  const headR = s * 0.09;
  ctx.beginPath();
  ctx.ellipse(nx, ny, headR * 1.2, headR * 0.9, -0.35, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillRect(nx + headR * 0.85, s * 0.26, s * 0.035, ny - s * 0.26);
  ctx.beginPath();
  ctx.moveTo(nx + headR * 0.85, s * 0.26);
  ctx.quadraticCurveTo(s * 0.62, s * 0.3, s * 0.6, s * 0.42);
  ctx.quadraticCurveTo(s * 0.56, s * 0.34, nx + headR * 0.85 + s * 0.035, s * 0.34);
  ctx.closePath();
  ctx.fill();
}

/**
 * An OPAQUE white-paper document with a coral fold and grey text bars —
 * exactly the kind of icon the border flood has to eat its background from.
 */
export function drawOpaqueDoc(ctx: CanvasRenderingContext2D, s: number) {
  // full-bleed opaque background (this is what the flood removes)
  ctx.fillStyle = "#eef0f3";
  ctx.fillRect(0, 0, s, s);
  // the paper subject
  const x = s * 0.24;
  const y = s * 0.14;
  const w = s * 0.52;
  const h = s * 0.72;
  // the outline must survive the demo flood: dark enough that even its
  // anti-aliased ramp exceeds the per-step tolerance (see scene-flood.tsx)
  ctx.fillStyle = "#ffffff";
  ctx.strokeStyle = "#5f6a78";
  ctx.lineWidth = Math.max(1.5, s * 0.03);
  ctx.beginPath();
  ctx.moveTo(x, y);
  ctx.lineTo(x + w * 0.68, y);
  ctx.lineTo(x + w, y + h * 0.2);
  ctx.lineTo(x + w, y + h);
  ctx.lineTo(x, y + h);
  ctx.closePath();
  ctx.fill();
  ctx.stroke();
  // dog-ear fold
  ctx.fillStyle = "#ff6f5e";
  ctx.beginPath();
  ctx.moveTo(x + w * 0.68, y);
  ctx.lineTo(x + w, y + h * 0.2);
  ctx.lineTo(x + w * 0.68, y + h * 0.2);
  ctx.closePath();
  ctx.fill();
  // text bars
  ctx.fillStyle = "#aeb6c0";
  const bx = x + w * 0.14;
  for (let i = 0; i < 4; i++) {
    const by = y + h * (0.36 + i * 0.14);
    ctx.fillRect(bx, by, w * (i === 3 ? 0.42 : 0.7), h * 0.05);
  }
}

/** A coral chat-bubble tile (transparent-edged) for the playground set. */
export function drawChatTile(ctx: CanvasRenderingContext2D, s: number) {
  ctx.clearRect(0, 0, s, s);
  const pad = s * 0.08;
  roundedRect(ctx, pad, pad, s - pad * 2, s - pad * 2, s * 0.2);
  ctx.fillStyle = "#ff6f5e";
  ctx.fill();
  ctx.fillStyle = "#ffffff";
  const bx = s * 0.26;
  const by = s * 0.3;
  const bw = s * 0.48;
  const bh = s * 0.32;
  roundedRect(ctx, bx, by, bw, bh, bh * 0.45);
  ctx.fill();
  ctx.beginPath();
  ctx.moveTo(bx + bw * 0.22, by + bh * 0.9);
  ctx.lineTo(bx + bw * 0.18, by + bh + s * 0.1);
  ctx.lineTo(bx + bw * 0.46, by + bh * 0.96);
  ctx.closePath();
  ctx.fill();
  ctx.fillStyle = "#ff6f5e";
  for (let i = 0; i < 3; i++) {
    ctx.beginPath();
    ctx.arc(bx + bw * (0.28 + i * 0.22), by + bh * 0.5, s * 0.032, 0, Math.PI * 2);
    ctx.fill();
  }
}

/** A slate gear tile (transparent-edged) for the playground set. */
export function drawGearTile(ctx: CanvasRenderingContext2D, s: number) {
  ctx.clearRect(0, 0, s, s);
  const pad = s * 0.08;
  roundedRect(ctx, pad, pad, s - pad * 2, s - pad * 2, s * 0.2);
  ctx.fillStyle = "#3f6796";
  ctx.fill();
  const cx = s / 2;
  const cy = s / 2;
  const outer = s * 0.24;
  ctx.fillStyle = "#ffffff";
  ctx.beginPath();
  for (let i = 0; i < 8; i++) {
    const a0 = (i / 8) * Math.PI * 2;
    const a1 = a0 + Math.PI / 8;
    const rOut = outer * 1.32;
    ctx.arc(cx, cy, outer, a0, a0 + Math.PI / 16);
    ctx.arc(cx, cy, rOut, a0 + Math.PI / 16, a1 - Math.PI / 16);
    ctx.arc(cx, cy, outer, a1 - Math.PI / 16, a1);
  }
  ctx.closePath();
  ctx.fill();
  ctx.globalCompositeOperation = "destination-out";
  ctx.beginPath();
  ctx.arc(cx, cy, outer * 0.42, 0, Math.PI * 2);
  ctx.fill();
  ctx.globalCompositeOperation = "source-over";
}

export function makeCanvas(size: number): { canvas: HTMLCanvasElement; ctx: CanvasRenderingContext2D } {
  const canvas = document.createElement("canvas");
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) throw new Error("2d context unavailable");
  return { canvas, ctx };
}
