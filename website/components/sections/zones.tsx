"use client";

import { useEffect, useRef } from "react";
import type { Dict } from "@/content/types";
import { Pic } from "@/components/pic";
import { Reveal } from "@/components/reveal";

/** Zone rectangles drawn by the site over a real styled desktop, in the
 * product's visual language (translucent panel + coral dashed boundary). */
const ZONES = [
  { x: 4, y: 8, w: 26, h: 62 },
  { x: 34, y: 8, w: 20, h: 38 },
  { x: 72, y: 46, w: 24, h: 42 },
];

export function Zones({ dict }: { dict: Dict }) {
  const z = dict.zones;
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const rect = el.getBoundingClientRect();
    if (rect.top <= window.innerHeight * 0.92) return;
    el.classList.add("armed");
    const io = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting || entry.boundingClientRect.top < 0) {
            el.classList.add("in-view");
            io.disconnect();
          }
        }
      },
      { rootMargin: "0px 0px -12% 0px", threshold: 0.2 },
    );
    io.observe(el);
    return () => io.disconnect();
  }, []);

  return (
    <section>
      <div className="mx-auto max-w-[1240px] px-5 pb-[clamp(5rem,10vw,8rem)] md:px-8">
        <Reveal>
          <div className="max-w-[640px]">
            <h2 className="font-display text-[clamp(2rem,3.6vw,3.2rem)] font-semibold leading-[1.05] tracking-[-0.02em]">
              {z.title}
            </h2>
            <p className="mt-3 text-[clamp(1.02rem,1.3vw,1.2rem)] text-text-mid">{z.body}</p>
          </div>
        </Reveal>
        <Reveal delay={80}>
          <div ref={ref} className="zone-draw relative mt-10 overflow-hidden rounded-card border border-hairline shadow-card">
            <Pic name="desk-squircle" alt={z.imgAlt} sizes="(max-width: 1240px) 92vw, 1176px" imgClassName="w-full" />
            <svg
              aria-hidden="true"
              viewBox="0 0 100 56.2"
              preserveAspectRatio="none"
              className="absolute inset-0 h-full w-full"
            >
              {ZONES.map((r, i) => (
                <rect
                  key={i}
                  className="zone-rect zone-path"
                  x={r.x}
                  y={(r.y * 56.2) / 100}
                  width={r.w}
                  height={(r.h * 56.2) / 100}
                  rx={1.2}
                  fill="rgb(255 255 255 / 0.07)"
                  stroke="#ff6f5e"
                  strokeWidth="0.28"
                  style={{ "--zone-delay": `${i * 180}ms` } as React.CSSProperties}
                  vectorEffect="non-scaling-stroke"
                />
              ))}
            </svg>
            {ZONES.map((r, i) => (
              <span
                key={i}
                className="absolute rounded-full bg-black/45 px-2.5 py-0.5 text-[11.5px] font-semibold text-text-hi backdrop-blur-sm"
                style={{ left: `${r.x + 1.2}%`, top: `${r.y + 2}%` }}
              >
                {z.zoneLabels[i]}
              </span>
            ))}
          </div>
        </Reveal>
        <Reveal delay={60}>
          <div className="mt-10 rounded-card border border-hairline bg-surface-1 px-7 py-6 md:px-9">
            <h3 className="text-[17px] font-semibold text-text-hi">{z.arrowTitle}</h3>
            <p className="mt-1.5 max-w-[72ch] text-[15px] text-text-dim">{z.arrowBody}</p>
          </div>
        </Reveal>
      </div>
    </section>
  );
}
