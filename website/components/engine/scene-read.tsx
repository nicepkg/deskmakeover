"use client";

import type { Lab } from "./lab";
import { outlineFromAlpha, rawImage } from "./lab";
import type { ReadAssets } from "./three/contract";
import { LABEL_CLASS, useScene } from "./three/use-scene";

const READ_ID = "thispc";

async function prepare(lab: Lab): Promise<ReadAssets | null> {
  const icon = rawImage(lab, READ_ID);
  if (!icon) return null;
  return {
    icon,
    outline: outlineFromAlpha(icon),
    seedHex: lab.seeds.get(READ_ID) ?? "#128577",
  };
}

const loadInit = () => import("./three/scenes3d").then((m) => m.createReadScene);

/**
 * 01 READ — the engine's eyes, in 3D: a scan sweeps the real This PC icon,
 * then the outline layer lifts, the dominant-colour chip flies out, and the
 * profile marks appear. Drag to orbit; the chips replay the checkup.
 */
export function ReadScene({
  steps,
  caption,
}: {
  steps: { key: string; label: string }[];
  caption: string;
}) {
  const { hostRef, canvasRef, bindLabel, handleRef, state } = useScene(prepare, loadInit);
  const labels: { id: string; text: string }[] = [
    { id: "outline", text: steps[3]?.label ?? "" },
    { id: "color", text: steps[2]?.label ?? "" },
    { id: "profile", text: steps[4]?.label ?? "" },
  ];
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

      <div className="mx-auto mt-4 flex max-w-[420px] flex-wrap items-center justify-center gap-2">
        {steps.map((s, i) => (
          <span
            key={s.key}
            className="inline-flex items-center gap-1.5 border border-line bg-card px-2.5 py-1 font-mono text-[10.5px] tracking-[0.08em] text-ink-2"
          >
            <span className="text-coral-ink">{String(i + 1).padStart(2, "0")}</span>
            {s.label}
          </span>
        ))}
        <button
          type="button"
          onClick={() => handleRef.current?.replay()}
          disabled={state !== "ready"}
          className="border border-line bg-card px-2.5 py-1 font-mono text-[10.5px] text-ink-2 transition-colors hover:border-ink-3 hover:text-ink disabled:opacity-40"
        >
          ↺
        </button>
      </div>
      <p className="mx-auto mt-3 max-w-[420px] text-center text-[12px] leading-[1.6] text-ink-3">{caption}</p>
    </div>
  );
}
