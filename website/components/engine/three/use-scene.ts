"use client";

import { useEffect, useRef, useState } from "react";
import { getLab, type Lab } from "../lab";
import type { LabelPoint, SceneHandle, SceneInit } from "./contract";

export type SceneState = "idle" | "ready" | "failed";

/**
 * Shared lifecycle for the /engine/ three.js scenes: lazy-boot on approach
 * (lab + assets + dynamic scene import), stream label anchors into positioned
 * HTML chips, dispose on unmount. Wrappers own all other DOM.
 */
export function useScene<A>(
  prepare: (lab: Lab) => Promise<A | null>,
  loadInit: () => Promise<SceneInit<A>>,
) {
  const hostRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const labelEls = useRef<Map<string, HTMLElement>>(new Map());
  const handleRef = useRef<SceneHandle | null>(null);
  const [state, setState] = useState<SceneState>("idle");

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    let cancelled = false;
    const io = new IntersectionObserver(
      (records) => {
        if (!records.some((r) => r.isIntersecting)) return;
        io.disconnect();
        (async () => {
          try {
            const lab = await getLab();
            const [assets, init] = await Promise.all([prepare(lab), loadInit()]);
            const canvas = canvasRef.current;
            if (!assets || !canvas || cancelled) throw new Error("scene assets unavailable");
            handleRef.current = init(canvas, assets, {
              reduceMotion: window.matchMedia("(prefers-reduced-motion: reduce)").matches,
              onLabel: (id: string, pt: LabelPoint) => {
                const el = labelEls.current.get(id);
                if (!el) return;
                el.style.transform = `translate(${pt.x.toFixed(1)}px, ${pt.y.toFixed(1)}px)`;
                el.style.opacity = pt.visible ? "1" : "0";
              },
            });
            setState("ready");
          } catch {
            if (!cancelled) setState("failed");
          }
        })();
      },
      { rootMargin: "400px" },
    );
    io.observe(host);
    return () => {
      cancelled = true;
      io.disconnect();
      handleRef.current?.dispose();
      handleRef.current = null;
    };
    // prepare/loadInit are stable module-level functions in every wrapper
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const bindLabel = (id: string) => (el: HTMLElement | null) => {
    if (el) labelEls.current.set(id, el);
    else labelEls.current.delete(id);
  };

  return { hostRef, canvasRef, bindLabel, handleRef, state };
}

/** The floating label chip used over every 3D scene. */
export const LABEL_CLASS =
  "pointer-events-none absolute left-0 top-0 whitespace-nowrap border border-line bg-canvas/90 px-2 py-0.5 font-mono text-[10.5px] tracking-[0.06em] text-ink-2 transition-opacity duration-300";
