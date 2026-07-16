"use client";

import { useEffect, useState } from "react";
import { img } from "@/lib/manifest";

function FullPicture({ id, alt }: { id: string; alt: string }) {
  const entry = img(id);
  const avif = entry.variants.map((v) => `${v.avif} ${v.w}w`).join(", ");
  const webp = entry.variants.map((v) => `${v.webp} ${v.w}w`).join(", ");
  const fallback = entry.variants[0].webp;
  return (
    <picture>
      <source type="image/avif" srcSet={avif} sizes="98vw" />
      <source type="image/webp" srcSet={webp} sizes="98vw" />
      {/* eslint-disable-next-line @next/next/no-img-element */}
      <img
        src={fallback}
        width={entry.w}
        height={entry.h}
        alt={alt}
        decoding="async"
        className="max-h-[92svh] w-auto max-w-full border border-line bg-white object-contain"
      />
    </picture>
  );
}

/**
 * Fullscreen lightbox for product captures — laptop viewers cannot read
 * desktop icons in a half-column image. Esc or any click closes it.
 */
export function Lightbox({
  id,
  alt,
  closeLabel,
  onClose,
}: {
  id: string;
  alt: string;
  closeLabel: string;
  onClose: () => void;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    const prev = document.documentElement.style.overflow;
    document.documentElement.style.overflow = "hidden";
    return () => {
      window.removeEventListener("keydown", onKey);
      document.documentElement.style.overflow = prev;
    };
  }, [onClose]);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={alt}
      className="fixed inset-0 z-[100] flex cursor-zoom-out items-center justify-center bg-canvas/95 p-4 backdrop-blur-sm md:p-8"
      onClick={onClose}
    >
      <FullPicture id={id} alt={alt} />
      <button
        type="button"
        onClick={onClose}
        aria-label={closeLabel}
        className="absolute right-4 top-4 flex h-10 w-10 items-center justify-center border border-line bg-white text-ink transition-colors hover:border-ink"
      >
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden>
          <path d="M1 1l12 12M13 1L1 13" stroke="currentColor" strokeWidth="1.5" />
        </svg>
      </button>
    </div>
  );
}

/** A product capture that opens fullscreen on click. */
export function Zoomable({
  id,
  alt,
  sizes,
  zoomHint,
  closeLabel,
}: {
  id: string;
  alt: string;
  sizes: string;
  zoomHint: string;
  closeLabel: string;
}) {
  const [open, setOpen] = useState(false);
  const entry = img(id);
  const avif = entry.variants.map((v) => `${v.avif} ${v.w}w`).join(", ");
  const webp = entry.variants.map((v) => `${v.webp} ${v.w}w`).join(", ");
  const fallback = entry.variants[entry.variants.length - 1].webp;

  return (
    <>
      <button
        type="button"
        onClick={() => setOpen(true)}
        className="group relative block w-full cursor-zoom-in overflow-hidden border border-line bg-white text-left transition-colors hover:border-ink-3"
        aria-label={`${alt} — ${zoomHint}`}
      >
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
            className="block h-auto w-full"
          />
        </picture>
        <span className="pointer-events-none absolute bottom-3 right-3 border border-line bg-white/92 px-2 py-1 font-mono text-[11px] tracking-[0.1em] text-ink-3 opacity-0 transition-opacity group-hover:opacity-100">
          {zoomHint}
        </span>
      </button>
      {open ? <Lightbox id={id} alt={alt} closeLabel={closeLabel} onClose={() => setOpen(false)} /> : null}
    </>
  );
}
