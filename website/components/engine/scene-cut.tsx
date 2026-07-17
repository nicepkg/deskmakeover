"use client";

import type { EngineDict } from "@/content/engine-types";
import type { Lab } from "./lab";
import { maskLayers, renderTile } from "./lab";
import type { CutAssets } from "./three/contract";
import { LABEL_CLASS, useScene } from "./three/use-scene";

const CUT_ID = "panel";
const MASK_URL = "/engine/icons/panel-mask.png";

async function prepare(lab: Lab): Promise<CutAssets | null> {
  const layers = await maskLayers(lab, CUT_ID, MASK_URL);
  const final = renderTile(lab, CUT_ID, {});
  return layers && final ? { bgLayer: layers.bgLayer, artLayer: layers.artLayer, final } : null;
}

const loadInit = () => import("./three/scenes3d").then((m) => m.createCutScene);

/**
 * 02 CUT — the real Control Panel icon splits into the engine's own verdict
 * layers: the base flushes coral, peels away, and the finished plate rises in.
 * The per-pixel split IS the desktop engine's real output (oracle mask).
 */
export function CutScene({ engine }: { engine: EngineDict }) {
  const c = engine.cut;
  const { hostRef, canvasRef, bindLabel, handleRef, state } = useScene(prepare, loadInit);
  const labels: { id: string; text: string }[] = [
    { id: "bg", text: c.layers.bg },
    { id: "art", text: c.layers.art },
    { id: "final", text: c.layers.final },
  ];
  const beats = c.maskNote.split("·").map((s) => s.trim());
  return (
    <div ref={hostRef}>
      <div className="relative mx-auto aspect-square w-full max-w-[480px]">
        <canvas ref={canvasRef} className="absolute inset-0 h-full w-full cursor-grab active:cursor-grabbing" aria-hidden />
        {labels.map((l) => (
          <div key={l.id} ref={bindLabel(l.id)} className={LABEL_CLASS} style={{ opacity: 0 }}>
            {l.text}
          </div>
        ))}
      </div>

      <div className="mx-auto mt-4 flex max-w-[420px] items-center justify-between gap-3">
        <div className="flex flex-wrap gap-x-3 gap-y-1 font-mono text-[10.5px] tracking-[0.04em] text-ink-3">
          {beats.map((b) => (
            <span key={b}>{b}</span>
          ))}
        </div>
        <button
          type="button"
          onClick={() => handleRef.current?.replay()}
          disabled={state !== "ready"}
          className="flex-none border border-line bg-card px-3 py-1 font-mono text-[11.5px] text-ink-2 transition-colors hover:border-ink-3 hover:text-ink disabled:opacity-40"
        >
          ↺ {c.replay}
        </button>
      </div>
      <p className="mx-auto mt-3 max-w-[420px] text-center text-[12px] leading-[1.6] text-ink-3">{c.caption}</p>
    </div>
  );
}
