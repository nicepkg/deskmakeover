"use client";

import { useEffect, useRef, useState } from "react";
import { img } from "@/lib/manifest";

function Layer({ id, alt, sizes }: { id: string; alt: string; sizes: string }) {
  const entry = img(id);
  const avif = entry.variants.map((v) => `${v.avif} ${v.w}w`).join(", ");
  const webp = entry.variants.map((v) => `${v.webp} ${v.w}w`).join(", ");
  const fallback = entry.variants[entry.variants.length - 1].webp;
  return (
    <picture>
      <source type="image/avif" srcSet={avif} sizes={sizes} />
      <source type="image/webp" srcSet={webp} sizes={sizes} />
      {/* eslint-disable-next-line @next/next/no-img-element */}
      <img
        src={fallback}
        width={entry.w}
        height={entry.h}
        alt={alt}
        loading="lazy"
        decoding="async"
        className="block h-full w-full object-cover"
      />
    </picture>
  );
}

/**
 * The proof wipe: both frames are real app captures of the same desktop.
 * AFTER lives LEFT of the divider, BEFORE right (the styled desktop leads,
 * matching the 3D hero's left-to-right scan). On first view the divider
 * sweeps left -> right to the resting point, then a full-surface range input
 * drives it — draggable anywhere, keyboard-accessible, no pointer math.
 */
export function BeforeAfter({
  dragHint,
  altBefore,
  altAfter,
}: {
  dragHint: string;
  altBefore: string;
  altAfter: string;
}) {
  const REST = 58;
  const [pos, setPos] = useState(REST);
  const rootRef = useRef<HTMLDivElement>(null);
  const sizes = "(min-width: 1280px) 1136px, 92vw";
  // the native 44px range thumb travels [22px, 100% - 22px]; the divider and
  // the clip edge must use the same coordinate or they drift near the edges
  const cut = `calc(22px + (100% - 44px) * ${pos / 100})`;

  // entrance: sweep the divider from the left edge to its resting point once
  useEffect(() => {
    const el = rootRef.current;
    if (!el) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    let raf = 0;
    const io = new IntersectionObserver(
      (records) => {
        for (const r of records) {
          if (!r.isIntersecting) continue;
          io.disconnect();
          const start = performance.now();
          const dur = 1400;
          const tick = (now: number) => {
            const t = Math.min(1, (now - start) / dur);
            const e = 1 - Math.pow(1 - t, 3);
            setPos(REST * e);
            if (t < 1) raf = requestAnimationFrame(tick);
          };
          raf = requestAnimationFrame(tick);
        }
      },
      { threshold: 0.45 },
    );
    io.observe(el);
    return () => {
      io.disconnect();
      cancelAnimationFrame(raf);
    };
  }, []);

  return (
    <div
      ref={rootRef}
      className="relative select-none overflow-hidden border border-line bg-card"
    >
      <div className="relative aspect-[16/9]">
        <div className="absolute inset-0">
          <Layer id="desk-before" alt={altBefore} sizes={sizes} />
        </div>
        <div
          className="absolute inset-0"
          style={{ clipPath: `inset(0 calc(100% - (${cut})) 0 0)` }}
        >
          <Layer id="desk-squircle" alt={altAfter} sizes={sizes} />
        </div>

        <input
          type="range"
          min={0}
          max={100}
          step={0.2}
          value={pos}
          onChange={(e) => setPos(Number(e.target.value))}
          aria-label={dragHint}
          className="wipe-range absolute inset-0 z-10 h-full w-full"
        />

        <div
          aria-hidden
          className="wipe-handle pointer-events-none absolute inset-y-0 z-[5]"
          style={{ left: cut }}
        >
          <div className="absolute inset-y-0 w-px -translate-x-1/2 bg-white" />
          <div className="wipe-knob absolute top-1/2 flex h-9 w-9 -translate-x-1/2 -translate-y-1/2 items-center justify-center bg-coral text-white">
            <svg width="14" height="10" viewBox="0 0 14 10" fill="none" aria-hidden>
              <path d="M4 1 1 5l3 4M10 1l3 4-3 4" stroke="currentColor" strokeWidth="1.6" />
            </svg>
          </div>
        </div>
      </div>
    </div>
  );
}
