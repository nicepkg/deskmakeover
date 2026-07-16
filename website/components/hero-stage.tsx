"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { Dict } from "@/content/types";
import { imageEntry } from "@/components/pic";

type Phase = "before" | "scanning" | "after" | "restoring";

const SCAN_MS = 2000;
const RESTORE_MS = 420;

interface HeroStageProps {
  dict: Dict["hero"];
}

/**
 * The transformation theater: the after-frame sweeps in behind a coral scan
 * line once on load, then control passes to the visitor via the
 * "put it back" / "beautify" button. Reduced motion swaps frames instantly.
 */
export function HeroStage({ dict }: HeroStageProps) {
  const [phase, setPhase] = useState<Phase>("before");
  const reducedRef = useRef(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const before = imageEntry("hero-before");
  const after = imageEntry("hero-after");

  useEffect(() => {
    reducedRef.current = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reducedRef.current) {
      setPhase("after");
      return;
    }
    const start = () => {
      timerRef.current = setTimeout(() => {
        setPhase("scanning");
        timerRef.current = setTimeout(() => setPhase("after"), SCAN_MS + 60);
      }, 500);
    };
    const img = new Image();
    img.src = after.variants[0].avif;
    let cancelled = false;
    img
      .decode()
      .catch(() => undefined)
      .then(() => {
        if (!cancelled) start();
      });
    return () => {
      cancelled = true;
      if (timerRef.current) clearTimeout(timerRef.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const toggle = useCallback(() => {
    if (timerRef.current) clearTimeout(timerRef.current);
    if (phase === "after") {
      if (reducedRef.current) {
        setPhase("before");
        return;
      }
      setPhase("restoring");
      timerRef.current = setTimeout(() => setPhase("before"), RESTORE_MS + 60);
    } else if (phase === "before") {
      if (reducedRef.current) {
        setPhase("after");
        return;
      }
      setPhase("scanning");
      timerRef.current = setTimeout(() => setPhase("after"), SCAN_MS + 60);
    }
  }, [phase]);

  const shown = phase === "after" || phase === "scanning";
  const stageClass =
    phase === "scanning"
      ? "stage scanning revealed"
      : phase === "after"
        ? "stage revealed"
        : phase === "restoring"
          ? "stage restoring"
          : "stage";
  const busy = phase === "scanning" || phase === "restoring";
  const srcSet = (entry: typeof before, kind: "avif" | "webp") =>
    entry.variants.map((v) => `${v[kind]} ${v.w}w`).join(", ");
  const sizes = "(max-width: 1240px) 94vw, 1160px";

  return (
    <figure className={stageClass} style={{ "--scan-ms": `${SCAN_MS}ms` } as React.CSSProperties}>
      <div className="stage-frame relative overflow-hidden rounded-card border border-hairline shadow-stage">
        <picture>
          <source type="image/avif" srcSet={srcSet(before, "avif")} sizes={sizes} />
          <source type="image/webp" srcSet={srcSet(before, "webp")} sizes={sizes} />
          <img
            src={before.variants[0].webp}
            width={before.w}
            height={before.h}
            alt={dict.imgAlt}
            loading="eager"
            decoding="sync"
            fetchPriority="high"
            className="block w-full"
          />
        </picture>
        <div className="stage-after absolute inset-0" aria-hidden="true">
          <picture>
            <source type="image/avif" srcSet={srcSet(after, "avif")} sizes={sizes} />
            <source type="image/webp" srcSet={srcSet(after, "webp")} sizes={sizes} />
            <img
              src={after.variants[0].webp}
              width={after.w}
              height={after.h}
              alt=""
              loading="eager"
              decoding="async"
              className="block w-full"
            />
          </picture>
        </div>
        <div className="pointer-events-none absolute inset-0 overflow-hidden" aria-hidden="true">
          <span className="scanline" />
        </div>
      </div>
      <figcaption className="mt-4 flex items-center justify-between gap-4">
        <span className="text-[15px] text-ink-soft" aria-live="polite">
          {shown ? dict.stageAfter : dict.stageBefore}
        </span>
        <button
          type="button"
          onClick={toggle}
          disabled={busy}
          className="rounded-btn border border-hairline bg-paper px-4 py-2 text-[15px] font-medium text-ink transition-colors duration-150 hover:border-coral hover:text-coral-text focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-coral active:scale-[0.98] disabled:opacity-60"
        >
          {shown || phase === "restoring" ? dict.putBack : dict.beautify}
        </button>
      </figcaption>
    </figure>
  );
}
