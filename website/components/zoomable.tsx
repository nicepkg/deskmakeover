"use client";

import { useCallback, useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { img } from "@/lib/manifest";

const ZOOM_STEPS = [1, 1.6, 2.4, 3.2];

function srcSets(id: string) {
  const entry = img(id);
  return {
    entry,
    avif: entry.variants.map((v) => `${v.avif} ${v.w}w`).join(", "),
    webp: entry.variants.map((v) => `${v.webp} ${v.w}w`).join(", "),
    full: entry.variants[0].webp,
    fallback: entry.variants[entry.variants.length - 1].webp,
  };
}

/**
 * Fullscreen viewer on the native <dialog> top layer: backdrop fade + scale
 * entrance, a floating bottom toolbar (zoom out / level / zoom in / close),
 * wheel + click zoom, drag to pan when zoomed. Esc and backdrop-click close;
 * focus containment and restoration come free with showModal().
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
  const dialogRef = useRef<HTMLDialogElement>(null);
  const [step, setStep] = useState(0);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [dragging, setDragging] = useState(false);
  const dragState = useRef({ startX: 0, startY: 0, panX: 0, panY: 0, moved: false });
  const { entry, avif, webp, full } = srcSets(id);
  const scale = ZOOM_STEPS[step];

  useEffect(() => {
    const d = dialogRef.current;
    if (!d) return;
    d.showModal();
    d.focus(); // keep the auto-focus ring off the close button on open
    const handleClose = () => onClose();
    d.addEventListener("close", handleClose);
    const prevOverflow = document.documentElement.style.overflow;
    document.documentElement.style.overflow = "hidden";
    return () => {
      d.removeEventListener("close", handleClose);
      document.documentElement.style.overflow = prevOverflow;
    };
  }, [onClose]);

  const setZoom = useCallback((next: number) => {
    const clamped = Math.min(Math.max(next, 0), ZOOM_STEPS.length - 1);
    setStep(clamped);
    if (clamped === 0) setPan({ x: 0, y: 0 });
  }, []);

  const onWheel = (e: React.WheelEvent) => {
    setZoom(step + (e.deltaY < 0 ? 1 : -1));
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLImageElement>) => {
    dragState.current = { startX: e.clientX, startY: e.clientY, panX: pan.x, panY: pan.y, moved: false };
    if (scale > 1) {
      setDragging(true);
      e.currentTarget.setPointerCapture(e.pointerId);
    }
  };
  const onPointerMove = (e: ReactPointerEvent<HTMLImageElement>) => {
    if (!dragging) return;
    const dx = e.clientX - dragState.current.startX;
    const dy = e.clientY - dragState.current.startY;
    if (Math.abs(dx) + Math.abs(dy) > 6) dragState.current.moved = true;
    setPan({ x: dragState.current.panX + dx, y: dragState.current.panY + dy });
  };
  const onPointerUp = (e: ReactPointerEvent<HTMLImageElement>) => {
    if (dragging) {
      setDragging(false);
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
    // click (not drag) toggles zoom
    if (!dragState.current.moved) setZoom(step === 0 ? 1 : 0);
  };

  return (
    <dialog
      ref={dialogRef}
      aria-label={alt}
      tabIndex={-1}
      className="dm-lightbox m-0 h-full max-h-none w-full max-w-none bg-transparent p-0 outline-none"
      onClick={(e) => {
        if (e.target === dialogRef.current) dialogRef.current?.close();
      }}
    >
      {/* the toolbar zone (bottom ~5.5rem) is reserved via padding so the
          FULL image is always visible above it, never covered or cut */}
      <div className="pointer-events-none flex h-full w-full items-center justify-center overflow-hidden px-4 pb-24 pt-4">
        <picture className="pointer-events-auto flex max-h-full max-w-full items-center justify-center">
          <source type="image/avif" srcSet={avif} sizes="96vw" />
          <source type="image/webp" srcSet={webp} sizes="96vw" />
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img
            src={full}
            width={entry.w}
            height={entry.h}
            alt={alt}
            decoding="async"
            draggable={false}
            onPointerDown={onPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            onWheel={onWheel}
            style={{
              // inline (survives any CSS pipeline hiccup): full fit against
              // the viewport minus the reserved toolbar strip
              maxWidth: "min(96vw, calc(100vw - 2rem))",
              maxHeight: "calc(100vh - 7.5rem)",
              width: "auto",
              height: "auto",
              transform: `translate3d(${pan.x}px, ${pan.y}px, 0) scale(${scale})`,
              transition: dragging ? "none" : "transform 260ms var(--ease-swift)",
              cursor: scale > 1 ? (dragging ? "grabbing" : "grab") : "zoom-in",
            }}
            className="dm-lightbox-img select-none object-contain"
          />
        </picture>
      </div>

      {/* close floats OVER the image layer, top-right, no reserved space */}
      <button
        type="button"
        onClick={() => dialogRef.current?.close()}
        aria-label={closeLabel}
        className="dm-lightbox-bar pointer-events-auto fixed right-4 top-4 flex h-10 w-10 items-center justify-center border border-line bg-white/92 text-ink backdrop-blur-sm transition-colors hover:bg-panel"
      >
        <svg width="13" height="13" viewBox="0 0 12 12" fill="none" aria-hidden>
          <path d="M1 1l10 10M11 1L1 11" stroke="currentColor" strokeWidth="1.5" />
        </svg>
      </button>

      <div className="pointer-events-auto fixed bottom-6 left-1/2 -translate-x-1/2">
        <div className="dm-lightbox-bar flex items-center border border-line bg-white/95 p-1 backdrop-blur-sm">
        <button
          type="button"
          onClick={() => setZoom(step - 1)}
          disabled={step === 0}
          aria-label="zoom out"
          className="flex h-9 w-9 items-center justify-center text-ink transition-colors hover:bg-panel disabled:opacity-30"
        >
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden>
            <path d="M1 6h10" stroke="currentColor" strokeWidth="1.5" />
          </svg>
        </button>
        <span className="w-14 text-center font-mono text-[12px] tracking-[0.06em] text-ink-2">
          {Math.round(scale * 100)}%
        </span>
        <button
          type="button"
          onClick={() => setZoom(step + 1)}
          disabled={step === ZOOM_STEPS.length - 1}
          aria-label="zoom in"
          className="flex h-9 w-9 items-center justify-center text-ink transition-colors hover:bg-panel disabled:opacity-30"
        >
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden>
            <path d="M6 1v10M1 6h10" stroke="currentColor" strokeWidth="1.5" />
          </svg>
        </button>
        <span aria-hidden className="mx-1 h-5 w-px bg-line" />
        <button
          type="button"
          onClick={() => setZoom(0)}
          disabled={step === 0 && pan.x === 0 && pan.y === 0}
          aria-label="reset zoom"
          className="flex h-9 items-center justify-center px-2 font-mono text-[11px] tracking-[0.08em] text-ink transition-colors hover:bg-panel disabled:opacity-30"
        >
          FIT
        </button>
        <a
          href={full}
          download
          aria-label="download image"
          className="flex h-9 w-9 items-center justify-center text-ink transition-colors hover:bg-panel"
        >
          <svg width="13" height="13" viewBox="0 0 13 13" fill="none" aria-hidden>
            <path d="M6.5 1v7.5M3 5.5l3.5 3.5L10 5.5M1.5 11.5h10" stroke="currentColor" strokeWidth="1.4" />
          </svg>
        </a>
        </div>
      </div>
    </dialog>
  );
}

/** A product capture: the WHOLE image is the zoom trigger. */
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
  const { entry, avif, webp, fallback } = srcSets(id);

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
            className="block h-auto w-full transition-transform duration-500 ease-[var(--ease-swift)] group-hover:scale-[1.012]"
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
