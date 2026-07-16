"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { Dict } from "@/content/types";
import { imageEntry, imageMeta } from "@/components/pic";
import { Hero3d, type Hero3dHandle } from "@/components/hero-3d";

type Phase = "before" | "assembling" | "settled" | "scattering";

const ASSEMBLE_MS = 2000;
const SCATTER_MS = 900;

/**
 * State machine around the 3D makeover scene. Styled icon cards fly onto a
 * real desktop; "put it back" sends them home. No WebGL or reduced motion:
 * a static frame pair with an instant toggle.
 */
export function HeroTheater({ dict }: { dict: Dict }) {
  const h = dict.hero;
  const before = imageEntry("desk-before");
  const after = imageEntry("desk-squircle");
  const { cells } = imageMeta();
  const [phase, setPhase] = useState<Phase>("before");
  const [mode, setMode] = useState<"3d" | "flat">("3d");
  const [ready, setReady] = useState(false);
  const reducedRef = useRef(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const sceneRef = useRef<Hero3dHandle>(null);

  useEffect(() => {
    reducedRef.current = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reducedRef.current) {
      setMode("flat");
      setPhase("settled");
    }
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);

  const onReady = useCallback(() => {
    setReady(true);
    timerRef.current = setTimeout(() => {
      setPhase("assembling");
      sceneRef.current?.assemble(ASSEMBLE_MS);
      timerRef.current = setTimeout(() => setPhase("settled"), ASSEMBLE_MS + 60);
    }, 700);
  }, []);

  const onFail = useCallback(() => {
    setMode("flat");
    setPhase("settled");
  }, []);

  const toggle = useCallback(() => {
    if (timerRef.current) clearTimeout(timerRef.current);
    if (mode === "flat") {
      setPhase((p) => (p === "settled" ? "before" : "settled"));
      return;
    }
    if (phase === "settled") {
      setPhase("scattering");
      sceneRef.current?.scatter(SCATTER_MS);
      timerRef.current = setTimeout(() => setPhase("before"), SCATTER_MS + 60);
    } else if (phase === "before") {
      setPhase("assembling");
      sceneRef.current?.assemble(ASSEMBLE_MS);
      timerRef.current = setTimeout(() => setPhase("settled"), ASSEMBLE_MS + 60);
    }
  }, [mode, phase]);

  const styled = phase === "settled" || phase === "assembling";
  const busy = phase === "assembling" || phase === "scattering";
  const srcSet = (entry: typeof before, kind: "avif" | "webp") =>
    entry.variants.map((v) => `${v[kind]} ${v.w}w`).join(", ");
  const sizes = "(max-width: 900px) 94vw, min(76vw, 1240px)";

  return (
    <div className="relative mx-auto w-[min(94vw,1240px)]">
      <div className="relative aspect-[16/9] w-full">
        {/* LCP frame; fades out once the 3D scene takes over */}
        <picture
          className={`absolute inset-x-0 top-1/2 -translate-y-1/2 transition-opacity duration-300 ${mode === "3d" && ready ? "opacity-0" : "opacity-100"}`}
        >
          <source type="image/avif" srcSet={srcSet(mode === "flat" && styled ? after : before, "avif")} sizes={sizes} />
          <source type="image/webp" srcSet={srcSet(mode === "flat" && styled ? after : before, "webp")} sizes={sizes} />
          <img
            src={before.variants[0].webp}
            width={before.w}
            height={before.h}
            alt={h.imgAlt}
            loading="eager"
            decoding="sync"
            fetchPriority="high"
            className="mx-auto w-[86%] rounded-[12px] border border-hairline"
          />
        </picture>
        {mode === "3d" && (
          <Hero3d
            ref={sceneRef}
            beforeUrl={before.variants.at(-1)!.webp}
            afterUrl={after.variants.at(-1)!.webp}
            cells={cells}
            onReady={onReady}
            onFail={onFail}
            className="absolute inset-0"
          />
        )}
      </div>
      <div className="pointer-events-none absolute inset-x-0 bottom-1 flex items-center justify-between px-[7%]">
        <span
          className="rounded-[8px] border border-hairline bg-white/85 px-3 py-1 text-[12.5px] font-medium text-text-mid backdrop-blur-sm"
          aria-live="polite"
        >
          {styled ? h.statusRefreshed : h.stageBefore}
        </span>
        <button
          type="button"
          onClick={toggle}
          disabled={busy}
          className="pointer-events-auto rounded-[8px] border border-hairline bg-white/85 px-4 py-1.5 text-[13px] font-semibold text-text-hi backdrop-blur-sm transition-colors duration-150 hover:border-coral-deep hover:text-coral-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-coral active:scale-[0.98] disabled:opacity-60"
        >
          {styled ? h.putBack : h.beautify}
        </button>
      </div>
    </div>
  );
}
