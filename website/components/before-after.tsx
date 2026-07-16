"use client";

import { useState } from "react";
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
 * A full-surface range input drives the divider — draggable anywhere,
 * keyboard-accessible, no pointer math.
 */
export function BeforeAfter({
  beforeLabel,
  afterLabel,
  dragHint,
  altBefore,
  altAfter,
}: {
  beforeLabel: string;
  afterLabel: string;
  dragHint: string;
  altBefore: string;
  altAfter: string;
}) {
  const [pos, setPos] = useState(42);
  const sizes = "(min-width: 1280px) 1136px, 92vw";

  return (
    <div className="relative select-none overflow-hidden border border-line bg-white">
      <div className="relative aspect-[16/9]">
        <div className="absolute inset-0">
          <Layer id="desk-squircle" alt={altAfter} sizes={sizes} />
        </div>
        <div
          className="absolute inset-0"
          style={{ clipPath: `inset(0 ${100 - pos}% 0 0)` }}
        >
          <Layer id="desk-before" alt={altBefore} sizes={sizes} />
        </div>

        <span className="pointer-events-none absolute left-4 top-4 border border-line bg-white/92 px-2.5 py-1 font-mono text-[11px] tracking-[0.14em] text-ink">
          {beforeLabel}
        </span>
        <span className="pointer-events-none absolute right-4 top-4 border border-line bg-white/92 px-2.5 py-1 font-mono text-[11px] tracking-[0.14em] text-coral-ink">
          {afterLabel}
        </span>

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
          style={{ left: `${pos}%` }}
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
