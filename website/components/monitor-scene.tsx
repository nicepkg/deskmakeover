"use client";

import { useEffect, useRef, useState } from "react";
import { Pic } from "@/components/pic";

/**
 * Client shell around the lazy three.js scene. The hero renders this in two
 * slots (desktop background layer / mobile block) with CSS deciding which is
 * visible — only the slot with real size mounts the scene, and a breakpoint
 * flip remounts on the other side. Reduced motion or any WebGL failure falls
 * back to the real styled render — the page always shows the product.
 */
export function MonitorScene({ before, after, alt }: { before: string; after: string; alt: string }) {
  const hostRef = useRef<HTMLDivElement>(null);
  const [failed, setFailed] = useState(false);
  const [breakpoint, setBreakpoint] = useState(0);

  useEffect(() => {
    const mq = window.matchMedia("(min-width: 1024px)");
    const onChange = () => setBreakpoint((b) => b + 1);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  useEffect(() => {
    let dispose: (() => void) | undefined;
    let cancelled = false;
    const host = hostRef.current;
    if (!host) return;
    if (host.offsetWidth === 0) return; // hidden slot at this breakpoint
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setFailed(true);
      return;
    }
    (async () => {
      try {
        const mod = await import("@/components/monitor-scene-impl");
        const d = await mod.mount(host, { before, after });
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
  }, [before, after, breakpoint]);

  if (failed) {
    return (
      <div className="relative h-full w-full">
        <div className="absolute inset-x-0 top-1/2 -translate-y-1/2 border border-line bg-white p-2 lg:left-[38%] lg:right-6">
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

  return <div ref={hostRef} className="absolute inset-0" role="img" aria-label={alt} />;
}
