"use client";

import type { EngineDict } from "@/content/engine-types";
import type { Lab } from "./lab";
import { PLATE_ONLY_ID, rawImage, renderTile } from "./lab";
import type { HeroAssets } from "./three/contract";
import { LABEL_CLASS, useScene } from "./three/use-scene";

/** The hero icon being exploded: the Pictures folder — complex, colourful,
 *  instantly recognisable as a real Windows icon. */
const HERO_ID = "pics";

async function prepare(lab: Lab): Promise<HeroAssets | null> {
  const seed = lab.seeds.get(HERO_ID);
  const raw = rawImage(lab, HERO_ID);
  const plate = renderTile(lab, PLATE_ONLY_ID, seed ? { plateColor: seed } : {});
  const final = renderTile(lab, HERO_ID, {});
  return raw && plate && final ? { raw, plate, final } : null;
}

const loadInit = () => import("./three/scenes3d").then((m) => m.createHeroScene);

/**
 * Hero visual: a real icon exploded into its three real layers (raw artwork /
 * derived plate / finished tile) in a drag-orbitable three.js scene.
 * Collapsed on arrival, it bursts apart on entry; double-click replays.
 */
export function StackScene({ engine }: { engine: EngineDict }) {
  const { hostRef, canvasRef, bindLabel, state } = useScene(prepare, loadInit);
  const labels: { id: string; text: string }[] = [
    { id: "raw", text: engine.hero.stack.raw },
    { id: "plate", text: engine.hero.stack.plate },
    { id: "final", text: engine.hero.stack.final },
  ];
  return (
    <div ref={hostRef}>
      <div className="relative aspect-square w-full max-w-[540px]">
        {state === "failed" ? (
          // eslint-disable-next-line @next/next/no-img-element
          <img src="/engine/fallback.webp" alt="" className="absolute inset-[12%] h-[76%] w-[76%]" />
        ) : (
          <>
            <canvas ref={canvasRef} className="absolute inset-0 h-full w-full cursor-grab active:cursor-grabbing" aria-hidden />
            {labels.map((l, i) => (
              <div key={l.id} ref={bindLabel(l.id)} className={LABEL_CLASS} style={{ opacity: 0 }}>
                <span className="mr-1.5 text-coral-ink">{String(i + 1).padStart(2, "0")}</span>
                {l.text}
              </div>
            ))}
          </>
        )}
      </div>
      <p className="mt-2 max-w-[500px] text-[12.5px] leading-[1.6] text-ink-3">{engine.hero.stack.caption}</p>
    </div>
  );
}
