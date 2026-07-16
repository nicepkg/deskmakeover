"use client";

import { useEffect, useRef, useState } from "react";
import { Pic } from "@/components/pic";

/**
 * Client shell around the lazy three.js scene. Reduced motion or any WebGL
 * failure falls back to the real styled render in a flat frame — the page
 * always shows the product.
 */
export function MonitorScene({
  before,
  after,
  alt,
  labelBefore,
  labelAfter,
}: {
  before: string;
  after: string;
  alt: string;
  labelBefore: string;
  labelAfter: string;
}) {
  const hostRef = useRef<HTMLDivElement>(null);
  const [failed, setFailed] = useState(false);
  const [phase, setPhase] = useState<"before" | "after">("before");

  useEffect(() => {
    let dispose: (() => void) | undefined;
    let cancelled = false;
    const host = hostRef.current;
    if (!host) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setFailed(true);
      return;
    }
    (async () => {
      try {
        const mod = await import("@/components/monitor-scene-impl");
        const d = await mod.mount(host, { before, after, onPhase: setPhase });
        if (cancelled) d();
        else dispose = d;
      } catch {
        if (!cancelled) setFailed(true);
      }
    })();
    return () => {
      cancelled = true;
      dispose?.();
    };
  }, [before, after]);

  if (failed) {
    return (
      <div className="relative h-full w-full">
        <div className="absolute inset-x-0 top-1/2 -translate-y-1/2 border border-line bg-white p-2">
          <Pic
            id="desk-squircle"
            alt={alt}
            sizes="(min-width: 1024px) 56vw, 92vw"
            priority
            imgClassName="block h-auto w-full"
          />
        </div>
      </div>
    );
  }

  return (
    <div className="relative h-full w-full">
      <div ref={hostRef} className="absolute inset-0" role="img" aria-label={alt} />
      <div
        aria-hidden
        className="pointer-events-none absolute bottom-3 left-1/2 -translate-x-1/2 border border-line bg-white/90 px-2.5 py-1 font-mono text-[11px] tracking-[0.14em]"
      >
        <span className={phase === "after" ? "text-coral-ink" : "text-ink-3"}>
          {phase === "after" ? labelAfter : labelBefore}
        </span>
      </div>
    </div>
  );
}
