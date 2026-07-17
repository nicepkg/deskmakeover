"use client";

import { useEffect, useRef } from "react";
import { drawNoteTile } from "./sample-art";

const SCAN_MS = 1200;

/**
 * 01 PORTRAIT — a coral scanline sweeps the sample icon once (linear:
 * scanners don't ease) while edge-ring and shape-ring probes light up as it
 * passes; the five pipeline-step chips flip on in sequence. All states are
 * CSS keyed off the surrounding [data-fx] scope (.fx-wait / .fx-in); this
 * component only paints the sample artwork.
 */
export function PortraitScene({
  steps,
  probeCaption,
  iouLabel,
}: {
  steps: { key: string; label: string }[];
  probeCaption: string;
  iouLabel: string;
}) {
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    drawNoteTile(ctx, canvas.width);
  }, []);

  // edge-ring probes (square border) + shape-ring probes (inscribed circle),
  // each delayed to when the scanline reaches its x position
  const edge: { x: number; y: number }[] = [];
  for (let i = 0; i < 4; i++) {
    const t = 0.14 + (i / 3) * 0.72;
    edge.push({ x: t, y: 0.045 }, { x: t, y: 0.955 });
  }
  edge.push({ x: 0.045, y: 0.35 }, { x: 0.045, y: 0.65 }, { x: 0.955, y: 0.35 }, { x: 0.955, y: 0.65 });
  const ring: { x: number; y: number }[] = [];
  for (let i = 0; i < 12; i++) {
    const a = (i / 12) * Math.PI * 2 - Math.PI / 2;
    ring.push({ x: 0.5 + Math.cos(a) * 0.34, y: 0.5 + Math.sin(a) * 0.34 });
  }

  return (
    <div>
      <div className="relative mx-auto aspect-square w-full max-w-[340px] border border-line bg-card">
        <canvas ref={ref} width={256} height={256} className="absolute inset-[9%] h-[82%] w-[82%]" aria-hidden />
        {/* the scanline */}
        <div className="eng-scan pointer-events-none absolute inset-y-0 w-[2px] bg-coral" aria-hidden />
        {/* probes */}
        {edge.map((p, i) => (
          <span
            key={`e${i}`}
            aria-hidden
            className="eng-lit absolute h-[7px] w-[7px] -translate-x-1/2 -translate-y-1/2 rounded-full bg-coral"
            style={{ left: `${p.x * 100}%`, top: `${p.y * 100}%`, ["--fxd" as string]: `${Math.round(p.x * SCAN_MS)}ms` }}
          />
        ))}
        {/* shape-ring probes sit ON the artwork — canvas fill + teal ring stays visible on any tile colour */}
        {ring.map((p, i) => (
          <span
            key={`r${i}`}
            aria-hidden
            className="eng-lit absolute h-[7px] w-[7px] -translate-x-1/2 -translate-y-1/2 rounded-full border-[1.5px] border-teal bg-canvas"
            style={{ left: `${p.x * 100}%`, top: `${p.y * 100}%`, ["--fxd" as string]: `${Math.round(p.x * SCAN_MS)}ms` }}
          />
        ))}
      </div>

      {/* pipeline-step chips */}
      <div className="mx-auto mt-5 flex max-w-[340px] flex-wrap justify-center gap-2">
        {steps.map((s, i) => (
          <span
            key={s.key}
            className="eng-lit inline-flex items-center gap-1.5 border border-line bg-card px-2.5 py-1 font-mono text-[10.5px] tracking-[0.08em] text-ink-2"
            style={{ ["--fxd" as string]: `${SCAN_MS + 140 + i * 220}ms` }}
          >
            <span className="text-coral-ink">{String(i + 1).padStart(2, "0")}</span>
            {s.label}
          </span>
        ))}
      </div>

      <p className="mx-auto mt-4 max-w-[340px] text-center text-[12px] leading-[1.6] text-ink-3">
        {probeCaption}
      </p>
      <p className="mx-auto mt-2 max-w-[340px] text-center font-mono text-[12px] text-ink-2">
        {iouLabel}
        {" IoU ≥ "}
        <span data-fx-count className="font-semibold text-ink tabular-nums">
          0.985
        </span>
      </p>
    </div>
  );
}
