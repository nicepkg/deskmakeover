"use client";

import { useEffect, useRef, useState } from "react";
import { getLab, renderTile } from "./lab";
import { MASTER_SIZE } from "./playground/renderer";

const FINISH_ID = "folder";
const FILTER_TAG: Record<"glass" | "pixel" | "sticker", string> = {
  glass: "Glass",
  pixel: "Pixel",
  sticker: "Sticker",
};

/**
 * 05 FINISH — the real folder icon rendered by the engine under each finish.
 * No schematic art: the cards ARE the output.
 */
export function FinishCards({
  finishes,
}: {
  finishes: { key: "glass" | "pixel" | "sticker"; kicker: string; name: string; line: string }[];
}) {
  const refs = useRef<Map<string, HTMLCanvasElement>>(new Map());
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const lab = await getLab();
        for (const f of ["glass", "pixel", "sticker"] as const) {
          const img = renderTile(lab, FINISH_ID, { filter: FILTER_TAG[f] });
          if (img && !cancelled) refs.current.get(f)?.getContext("2d")?.putImageData(img, 0, 0);
        }
        if (!cancelled) setReady(true);
      } catch {
        // cards keep their text; the playground below still tells the story
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="grid gap-px border border-line bg-line sm:grid-cols-3">
      {finishes.map((f, i) => (
        <div key={f.key} data-fx-cell className="bg-card p-5" style={{ ["--fxd" as string]: `${i * 140}ms` }}>
          <div className="border border-line bg-panel p-4">
            <canvas
              ref={(el) => {
                if (el) refs.current.set(f.key, el);
              }}
              width={MASTER_SIZE}
              height={MASTER_SIZE}
              className={`mx-auto block aspect-square w-full max-w-[220px] transition-opacity duration-500 ${
                ready ? "opacity-100" : "opacity-0"
              }`}
              aria-label={f.name}
            />
          </div>
          <p className="mt-4 font-mono text-[10px] tracking-[0.18em] text-coral-ink">{f.kicker}</p>
          <h3 className="mt-1 text-[17px] font-bold text-ink">{f.name}</h3>
          <p className="mt-1.5 text-[12.5px] leading-[1.55] text-ink-2">{f.line}</p>
        </div>
      ))}
    </div>
  );
}
