"use client";

import { useState } from "react";
import type { EngineDict } from "@/content/engine-types";
import type { Lab } from "./lab";
import { PLATE_ONLY_ID, rawImage, renderTile } from "./lab";
import type { PromiseAssets } from "./three/contract";
import { LABEL_CLASS, useScene } from "./three/use-scene";

const TRIO = ["mail", "thispc", "pics"] as const;

function hexToHue(hex: string): number {
  const n = parseInt(hex.replace("#", ""), 16);
  const r = ((n >> 16) & 255) / 255;
  const g = ((n >> 8) & 255) / 255;
  const b = (n & 255) / 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const d = max - min;
  if (d === 0) return 0;
  let h: number;
  if (max === r) h = ((g - b) / d) % 6;
  else if (max === g) h = (b - r) / d + 2;
  else h = (r - g) / d + 4;
  return ((h * 60) % 360 + 360) % 360;
}

function hslToHex(h: number, s: number, l: number): string {
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => {
    const k = (n + h / 30) % 12;
    const c = l - a * Math.max(-1, Math.min(k - 3, Math.min(9 - k, 1)));
    return Math.round(255 * c)
      .toString(16)
      .padStart(2, "0");
  };
  return `#${f(0)}${f(8)}${f(4)}`;
}

/** Push colliding hues apart to a ≥12° gap, each moved at most ±18° — the
 *  engine's own hue-spread parameters, applied to the trio. */
function spreadHues(hues: number[]): number[] {
  const order = hues.map((h, i) => ({ h, i })).sort((a, b) => a.h - b.h);
  const out = [...hues];
  const clampDelta = (d: number) => Math.max(-18, Math.min(18, d));
  const [lo, mid, hi] = order;
  if (mid.h - lo.h < 12) out[lo.i] = lo.h + clampDelta(mid.h - 12 - lo.h);
  if (hi.h - mid.h < 12) out[hi.i] = hi.h + clampDelta(mid.h + 12 - hi.h);
  return out;
}

async function prepare(lab: Lab): Promise<PromiseAssets | null> {
  const seeds = TRIO.map((id) => lab.seeds.get(id) ?? "#2f7cd6");
  const hues = seeds.map(hexToHue);
  const collided = hues.map(() => hslToHex(hues[1], 0.55, 0.58));
  const spread = spreadHues(hues).map((h) => hslToHex(h, 0.55, 0.58));
  const items: PromiseAssets["items"] = [];
  for (let i = 0; i < TRIO.length; i++) {
    const art = rawImage(lab, TRIO[i]);
    const plateBefore = renderTile(lab, PLATE_ONLY_ID, { plateColor: collided[i] });
    const plateAfter = renderTile(lab, PLATE_ONLY_ID, { plateColor: spread[i] });
    if (!art || !plateBefore || !plateAfter) return null;
    items.push({ art, plateBefore, plateAfter });
  }
  return { items };
}

const loadInit = () => import("./three/scenes3d").then((m) => m.createPromiseScene);

/**
 * 04 PROMISE — three real tiles in 3D. The plate layers change colour and
 * step apart; the artwork layers never move and never change a pixel. That
 * immovability IS the iron law, made visible.
 */
export function PromiseScene({ engine }: { engine: EngineDict }) {
  const p = engine.promise;
  const { hostRef, canvasRef, bindLabel, handleRef, state } = useScene(prepare, loadInit);
  const [after, setAfter] = useState(true); // the scene auto-plays to "after"
  const set = (v: boolean) => {
    setAfter(v);
    handleRef.current?.setState?.(v ? "after" : "before");
  };
  return (
    <div ref={hostRef}>
      <div className="relative mx-auto aspect-[16/10] w-full max-w-[560px]">
        <canvas ref={canvasRef} className="absolute inset-0 h-full w-full cursor-grab active:cursor-grabbing" aria-hidden />
        {TRIO.map((id, i) => (
          <div key={id} ref={bindLabel(`a${i}`)} className={LABEL_CLASS} style={{ opacity: 0 }}>
            {engine.castNames[id] ?? id}
          </div>
        ))}
      </div>

      <div className="mx-auto mt-4 flex max-w-[460px] items-center justify-center gap-2">
        {[
          { label: p.before, value: false },
          { label: p.after, value: true },
        ].map((o) => (
          <button
            key={o.label}
            type="button"
            onClick={() => set(o.value)}
            aria-pressed={after === o.value}
            disabled={state !== "ready"}
            className={`border px-3 py-1.5 font-mono text-[11.5px] transition-colors disabled:opacity-40 ${
              after === o.value
                ? "border-coral bg-coral text-white"
                : "border-line bg-card text-ink-2 hover:border-ink-3"
            }`}
          >
            {o.label}
          </button>
        ))}
      </div>

      <div className="mx-auto mt-5 max-w-[460px] border border-line bg-panel px-4 py-3.5 text-center">
        <p className="font-mono text-[10px] tracking-[0.18em] text-coral-ink">IRON LAW</p>
        <p className="mt-1.5 text-[15px] font-bold text-ink">{p.rule}</p>
      </div>
      <p className="mx-auto mt-3 max-w-[460px] text-center text-[12px] leading-[1.6] text-ink-3">{p.caption}</p>
    </div>
  );
}
