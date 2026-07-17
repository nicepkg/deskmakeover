"use client";

import { useState } from "react";
import type { StyleEntry } from "@/content/types";
import { img } from "@/lib/manifest";
import { Lightbox } from "@/components/zoomable";

const SIZES = "(min-width: 1024px) 62vw, 92vw";

function WallImage({
  id,
  alt,
  active,
  onFadedOut,
}: {
  id: string;
  alt: string;
  active: boolean;
  onFadedOut?: () => void;
}) {
  const entry = img(id);
  const avif = entry.variants.map((v) => `${v.avif} ${v.w}w`).join(", ");
  const webp = entry.variants.map((v) => `${v.webp} ${v.w}w`).join(", ");
  const fallback = entry.variants[entry.variants.length - 1].webp;
  return (
    <picture
      className={`absolute inset-0 transition-opacity duration-300 ${active ? "opacity-100" : "opacity-0"}`}
      aria-hidden={!active}
      onTransitionEnd={active ? undefined : onFadedOut}
    >
      <source type="image/avif" srcSet={avif} sizes={SIZES} />
      <source type="image/webp" srcSet={webp} sizes={SIZES} />
      {/* eslint-disable-next-line @next/next/no-img-element */}
      <img
        src={fallback}
        width={entry.w}
        height={entry.h}
        alt={active ? alt : ""}
        loading="lazy"
        decoding="async"
        className="block h-full w-full object-cover"
      />
    </picture>
  );
}

/**
 * Nine real desktops, one per look. A flat index rail on the left drives the
 * big frame; on small screens the rail becomes a scrollable chip row.
 */
export function StyleWall({
  styles,
  altPrefix,
  zoomHint,
  closeLabel,
}: {
  styles: StyleEntry[];
  altPrefix: string;
  zoomHint: string;
  closeLabel: string;
}) {
  const [zoomed, setZoomed] = useState(false);
  // only the active render and the one fading out stay mounted — nine
  // full-size desktops decoded at once is real memory and network cost
  const [frame, setFrame] = useState<{ active: string; prev: string | null }>({
    active: styles[0].key,
    prev: null,
  });
  const active = frame.active;
  const setActive = (key: string) =>
    setFrame((f) => (key === f.active ? f : { active: key, prev: f.active }));

  return (
    <div className="grid grid-cols-1 gap-6 lg:grid-cols-12">
      <div className="order-2 min-w-0 lg:order-1 lg:col-span-4">
        <div className="-mx-5 flex gap-2 overflow-x-auto px-5 pb-1 lg:mx-0 lg:flex-col lg:gap-0 lg:overflow-visible lg:border-t lg:border-line lg:px-0">
          {styles.map((s, i) => {
            const isActive = s.key === active;
            return (
              <button
                key={s.key}
                type="button"
                onMouseEnter={() => setActive(s.key)}
                onFocus={() => setActive(s.key)}
                onClick={() => setActive(s.key)}
                aria-pressed={isActive}
                className={`flex shrink-0 items-baseline gap-3 whitespace-nowrap border border-line px-3 py-2 text-left transition-colors lg:w-full lg:border-x-0 lg:border-t-0 lg:border-b lg:px-1 lg:py-3 ${
                  isActive ? "bg-panel" : "bg-transparent hover:bg-panel/60"
                }`}
              >
                <span
                  className={`font-mono text-[11px] tracking-[0.1em] ${isActive ? "text-coral-ink" : "text-ink-3"}`}
                >
                  {String(i + 1).padStart(2, "0")}
                </span>
                <span className={`text-[15px] font-semibold ${isActive ? "text-ink" : "text-ink-2"}`}>
                  {s.name}
                </span>
                <span className="hidden text-[13px] text-ink-3 lg:inline">{s.tagline}</span>
              </button>
            );
          })}
        </div>
      </div>
      <div className="order-1 min-w-0 lg:order-2 lg:col-span-8">
        <button
          type="button"
          onClick={() => setZoomed(true)}
          aria-label={`${altPrefix} ${styles.find((s) => s.key === active)?.name ?? active} — ${zoomHint}`}
          className="group relative block w-full cursor-zoom-in overflow-hidden border border-line bg-card transition-colors hover:border-ink-3"
        >
          <div className="relative aspect-[16/9]">
            {[
              styles.find((s) => s.key === frame.active),
              styles.find((s) => s.key === frame.prev),
            ]
              .filter((s): s is StyleEntry => s !== undefined)
              .map((s) => (
                // prev renders last (on top) so it fades OUT over the new image
                <WallImage
                  key={s.key}
                  id={`desk-${s.key}`}
                  alt={`${altPrefix} ${s.name}`}
                  active={s.key === active}
                  onFadedOut={
                    s.key === frame.prev
                      ? () => setFrame((f) => (f.prev === s.key ? { ...f, prev: null } : f))
                      : undefined
                  }
                />
              ))}
          </div>
          <span className="pointer-events-none absolute bottom-3 right-3 border border-line bg-card/90 px-2 py-1 font-mono text-[11px] tracking-[0.1em] text-ink-3 opacity-0 transition-opacity group-hover:opacity-100">
            {zoomHint}
          </span>
        </button>
      </div>
      {zoomed ? (
        <Lightbox
          id={`desk-${active}`}
          alt={`${altPrefix} ${styles.find((s) => s.key === active)?.name ?? active}`}
          closeLabel={closeLabel}
          onClose={() => setZoomed(false)}
        />
      ) : null}
    </div>
  );
}
