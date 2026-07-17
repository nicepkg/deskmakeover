"use client";

import { useState } from "react";
import type { EngineDict } from "@/content/engine-types";
import type { Lab } from "./lab";
import { diffLayer, renderTile } from "./lab";
import type { RescueAssets } from "./three/contract";
import { LABEL_CLASS, useScene } from "./three/use-scene";

const RESCUE_ID = "mail";
const FALLBACK_PLATE = "#2f7cd6";

async function prepare(lab: Lab): Promise<RescueAssets | null> {
  const plate = lab.seeds.get(RESCUE_ID) ?? FALLBACK_PLATE;
  const off = renderTile(lab, RESCUE_ID, { plateColor: plate, autoSeparation: false });
  const on = renderTile(lab, RESCUE_ID, { plateColor: plate, autoSeparation: true });
  return off && on ? { off, on, rescueLayer: diffLayer(on, off) } : null;
}

const loadInit = () => import("./three/scenes3d").then((m) => m.createRescueScene);

/**
 * 03 RESCUE — a real A/B told in layers: the colliding tile, and floating
 * above it the EXACT pixels the engine computed as the rescue (outline +
 * shadow, extracted from the two real renders). Toggling merges the layer in.
 */
export function RescueScene({ engine }: { engine: EngineDict }) {
  const r = engine.rescue;
  const { hostRef, canvasRef, bindLabel, handleRef, state } = useScene(prepare, loadInit);
  const [on, setOn] = useState(true); // the entrance choreography ends merged (rescue on)
  const set = (v: boolean) => {
    setOn(v);
    handleRef.current?.setState?.(v ? "on" : "off");
  };
  return (
    <div ref={hostRef}>
      <div className="relative mx-auto aspect-square w-full max-w-[480px]">
        <canvas ref={canvasRef} className="absolute inset-0 h-full w-full cursor-grab active:cursor-grabbing" aria-hidden />
        {[
          { id: "tile", text: r.layers.tile },
          { id: "rescue", text: r.layers.rescue },
        ].map((l) => (
          <div key={l.id} ref={bindLabel(l.id)} className={LABEL_CLASS} style={{ opacity: 0 }}>
            {l.text}
          </div>
        ))}
      </div>

      <div className="mx-auto mt-4 flex max-w-[420px] items-center justify-center gap-2">
        {[
          { label: r.offLabel, value: false },
          { label: r.onLabel, value: true },
        ].map((o) => (
          <button
            key={o.label}
            type="button"
            onClick={() => set(o.value)}
            aria-pressed={on === o.value}
            disabled={state !== "ready"}
            className={`border px-3 py-1.5 font-mono text-[11.5px] transition-colors disabled:opacity-40 ${
              on === o.value
                ? "border-coral bg-coral text-white"
                : "border-line bg-card text-ink-2 hover:border-ink-3"
            }`}
          >
            {o.label}
          </button>
        ))}
      </div>

      <div className="mx-auto mt-5 grid max-w-[420px] grid-cols-3 gap-px border border-line bg-line">
        {r.beats.map((b) => (
          <div key={b.key} className="bg-card px-2.5 py-2.5">
            <p className="font-mono text-[9.5px] tracking-[0.14em] text-coral-ink">{b.key}</p>
            <p className="mt-1 text-[12.5px] font-semibold text-ink">{b.title}</p>
            <p className="mt-0.5 text-[10.5px] leading-[1.5] text-ink-3">{b.detail}</p>
          </div>
        ))}
      </div>

      <p className="mx-auto mt-3 max-w-[420px] text-center text-[12px] leading-[1.6] text-ink-3">{r.caption}</p>
    </div>
  );
}
