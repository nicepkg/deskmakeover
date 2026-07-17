"use client";

import { useEffect, useRef, useState } from "react";
import { drawOpaqueDoc, makeCanvas } from "./sample-art";

const GRID = 96; // flood resolution — big enough to read, small enough to precompute instantly
const STEP_MS = 34; // visibly discrete layer steps, ~2s total

/**
 * 02 SEPARATE — the border-seeded flood, run for real: a JS mirror of the
 * segment.rs mechanic (FIFO BFS from edge seeds, acceptance relative to the
 * pixel stepped FROM) over the sample artwork, replayed layer by layer in
 * visible discrete steps. Reduced motion (or no JS) shows the final frame.
 */
export function FloodScene({ caption, replay }: { caption: string; replay: string }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [layerLabel, setLayerLabel] = useState("");
  const animRef = useRef<{ start: () => void; raf: number } | null>(null);

  useEffect(() => {
    const view = canvasRef.current;
    if (!view) return;
    const vctx = view.getContext("2d");
    if (!vctx) return;
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    const { ctx } = makeCanvas(GRID);
    drawOpaqueDoc(ctx, GRID);
    const src = ctx.getImageData(0, 0, GRID, GRID);
    const { depth, maxDepth } = borderFlood(src);

    const drawFrame = (upTo: number) => {
      const frame = vctx.createImageData(GRID, GRID);
      for (let i = 0; i < GRID * GRID; i++) {
        const d = depth[i];
        const eaten = d >= 0 && d <= upTo;
        if (!eaten) {
          frame.data[i * 4] = src.data[i * 4];
          frame.data[i * 4 + 1] = src.data[i * 4 + 1];
          frame.data[i * 4 + 2] = src.data[i * 4 + 2];
          frame.data[i * 4 + 3] = src.data[i * 4 + 3];
        }
      }
      vctx.clearRect(0, 0, GRID, GRID);
      vctx.putImageData(frame, 0, 0);
      if (upTo >= 0 && upTo < maxDepth) {
        // the advancing front, one discrete coral ring
        vctx.fillStyle = "#ff6f5e";
        for (let i = 0; i < GRID * GRID; i++) {
          if (depth[i] === upTo) vctx.fillRect(i % GRID, Math.floor(i / GRID), 1, 1);
        }
      }
      setLayerLabel(`BFS ${Math.max(0, Math.min(upTo, maxDepth))} / ${maxDepth}`);
    };

    const anim = {
      raf: 0,
      start() {
        cancelAnimationFrame(anim.raf);
        let layer = -1;
        let last = 0;
        const tick = (now: number) => {
          if (now - last >= STEP_MS) {
            last = now;
            layer++;
            drawFrame(layer);
          }
          if (layer < maxDepth + 4) anim.raf = requestAnimationFrame(tick);
        };
        anim.raf = requestAnimationFrame(tick);
      },
    };
    animRef.current = anim;

    if (reduce) {
      drawFrame(maxDepth);
      return;
    }

    drawFrame(-1);
    let played = false;
    const io = new IntersectionObserver(
      (records) => {
        for (const r of records) {
          if (r.isIntersecting && !played) {
            played = true;
            anim.start();
            io.disconnect();
          }
        }
      },
      { threshold: 0.4 },
    );
    io.observe(view);
    return () => {
      io.disconnect();
      cancelAnimationFrame(anim.raf);
    };
  }, []);

  return (
    <div>
      <div className="relative mx-auto aspect-square w-full max-w-[340px] border border-line bg-card">
        <canvas
          ref={canvasRef}
          width={GRID}
          height={GRID}
          className="absolute inset-[9%] h-[82%] w-[82%] [image-rendering:pixelated]"
          aria-hidden
        />
      </div>
      <div className="mx-auto mt-4 flex max-w-[340px] items-center justify-between">
        <span className="font-mono text-[11.5px] text-ink-3 tabular-nums">{layerLabel}</span>
        <button
          type="button"
          onClick={() => animRef.current?.start()}
          className="border border-line bg-card px-3 py-1 font-mono text-[11.5px] text-ink-2 transition-colors hover:border-ink-3 hover:text-ink"
        >
          ↺ {replay}
        </button>
      </div>
      <p className="mx-auto mt-3 max-w-[340px] text-center text-[12px] leading-[1.6] text-ink-3">{caption}</p>
    </div>
  );
}

/**
 * Border-seeded local-tolerance flood — the same mechanic as
 * dm-icon-core/src/segment (FIFO queue, left/right/up/down neighbours,
 * acceptance relative to the pixel stepped FROM). Returns each background
 * pixel's BFS layer depth (-1 = subject, never reached).
 */
function borderFlood(img: ImageData): { depth: Int16Array; maxDepth: number } {
  const w = img.width;
  const h = img.height;
  const d = img.data;
  const depth = new Int16Array(w * h).fill(-1);
  const TOL = 26; // per-channel tolerance vs the pixel stepped from
  const near = (a: number, b: number) => {
    const dr = d[a * 4] - d[b * 4];
    const dg = d[a * 4 + 1] - d[b * 4 + 1];
    const db = d[a * 4 + 2] - d[b * 4 + 2];
    return dr * dr + dg * dg + db * db <= TOL * TOL * 3;
  };
  let queue: number[] = [];
  const seed = (i: number) => {
    if (depth[i] === -1) {
      depth[i] = 0;
      queue.push(i);
    }
  };
  for (let x = 0; x < w; x++) {
    seed(x);
    seed((h - 1) * w + x);
  }
  for (let y = 0; y < h; y++) {
    seed(y * w);
    seed(y * w + w - 1);
  }
  let maxDepth = 0;
  while (queue.length > 0) {
    const next: number[] = [];
    for (const i of queue) {
      const x = i % w;
      const y = (i - x) / w;
      const steps = [
        x > 0 ? i - 1 : -1,
        x < w - 1 ? i + 1 : -1,
        y > 0 ? i - w : -1,
        y < h - 1 ? i + w : -1,
      ];
      for (const n of steps) {
        if (n >= 0 && depth[n] === -1 && near(n, i)) {
          depth[n] = depth[i] + 1;
          maxDepth = Math.max(maxDepth, depth[n]);
          next.push(n);
        }
      }
    }
    queue = next;
  }
  return { depth, maxDepth };
}
