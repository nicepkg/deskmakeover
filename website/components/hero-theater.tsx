"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { Dict } from "@/content/types";
import { imageEntry, imageMeta } from "@/components/pic";
import { DeskGl, type DeskGlHandle } from "@/components/desk-gl";

type Phase = "before" | "scanning" | "settled" | "restoring";

const SCAN_MS = 1800;
const RESTORE_MS = 460;

/**
 * The transformation theater. A perspective-tilted real desktop; when WebGL
 * is available the transformation runs as a shader (scan displacement,
 * chroma split, coral edge glow, pointer light), otherwise the CSS clip +
 * flip-tile path takes over. A live "put it back" control demonstrates the
 * core promise in place. Reduced motion: instant swaps, no GL.
 */
export function HeroTheater({ dict }: { dict: Dict }) {
  const h = dict.hero;
  const before = imageEntry("desk-before");
  const after = imageEntry("desk-squircle");
  const { featured } = imageMeta();
  const [phase, setPhase] = useState<Phase>("before");
  const [gl, setGl] = useState<"try" | "on" | "off">("try");
  const reducedRef = useRef(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const glRef = useRef<DeskGlHandle>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const afterImgRef = useRef<HTMLImageElement>(null);

  useEffect(() => {
    reducedRef.current = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reducedRef.current) {
      setGl("off");
      setPhase("settled");
    }
  }, []);

  // CSS fallback path: start the scan once the after frame is decodable
  useEffect(() => {
    if (gl !== "off" || reducedRef.current || phase !== "before") return;
    let cancelled = false;
    const img = afterImgRef.current;
    (img?.decode() ?? Promise.resolve())
      .catch(() => undefined)
      .then(() => {
        if (cancelled) return;
        timerRef.current = setTimeout(() => {
          setPhase("scanning");
          timerRef.current = setTimeout(() => setPhase("settled"), SCAN_MS + 80);
        }, 900);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [gl]);

  const glReady = useCallback(() => {
    setGl("on");
    if (reducedRef.current) {
      glRef.current?.set(1);
      return;
    }
    timerRef.current = setTimeout(() => {
      setPhase("scanning");
      glRef.current?.play(1, SCAN_MS);
      timerRef.current = setTimeout(() => setPhase("settled"), SCAN_MS + 80);
    }, 900);
  }, []);

  const glFail = useCallback(() => setGl("off"), []);

  // pointer parallax on the stage wrapper (fine pointers, motion allowed)
  useEffect(() => {
    const wrap = wrapRef.current;
    if (!wrap || reducedRef.current) return;
    if (!window.matchMedia("(hover: hover) and (pointer: fine)").matches) return;
    const onMove = (e: PointerEvent) => {
      const r = wrap.getBoundingClientRect();
      wrap.style.setProperty("--px", `${(((e.clientX - r.left) / r.width - 0.5) * 5).toFixed(2)}deg`);
      wrap.style.setProperty("--py", `${((0.5 - (e.clientY - r.top) / r.height) * 3).toFixed(2)}deg`);
    };
    const onLeave = () => {
      wrap.style.setProperty("--px", "0deg");
      wrap.style.setProperty("--py", "0deg");
    };
    wrap.addEventListener("pointermove", onMove);
    wrap.addEventListener("pointerleave", onLeave);
    return () => {
      wrap.removeEventListener("pointermove", onMove);
      wrap.removeEventListener("pointerleave", onLeave);
    };
  }, []);

  useEffect(
    () => () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    },
    [],
  );

  const toggle = useCallback(() => {
    if (timerRef.current) clearTimeout(timerRef.current);
    if (phase === "settled") {
      if (reducedRef.current) {
        glRef.current?.set(0);
        setPhase("before");
        return;
      }
      setPhase("restoring");
      glRef.current?.play(0, RESTORE_MS);
      timerRef.current = setTimeout(() => setPhase("before"), RESTORE_MS + 60);
    } else if (phase === "before") {
      if (reducedRef.current) {
        glRef.current?.set(1);
        setPhase("settled");
        return;
      }
      setPhase("scanning");
      glRef.current?.play(1, SCAN_MS);
      timerRef.current = setTimeout(() => setPhase("settled"), SCAN_MS + 80);
    }
  }, [phase]);

  const styled = phase === "settled" || phase === "scanning";
  const busy = phase === "scanning" || phase === "restoring";
  const useCss = gl === "off";
  const cls = [
    "theater",
    phase === "scanning" && "scanning revealed",
    phase === "settled" && "settled revealed pulsing",
    phase === "restoring" && "restoring",
  ]
    .filter(Boolean)
    .join(" ");
  const srcSet = (entry: typeof before, kind: "avif" | "webp") =>
    entry.variants.map((v) => `${v[kind]} ${v.w}w`).join(", ");
  const sizes = "(max-width: 900px) 94vw, min(72vw, 1180px)";
  const texUrl = (entry: typeof before) => entry.variants.at(-1)!.webp;
  const tileImage = `image-set(url("${before.variants.at(-1)!.avif}") type("image/avif"), url("${before.variants.at(-1)!.webp}") type("image/webp"))`;
  const tileImageAfter = `image-set(url("${after.variants.at(-1)!.avif}") type("image/avif"), url("${after.variants.at(-1)!.webp}") type("image/webp"))`;

  return (
    <div ref={wrapRef} className={cls} style={{ "--scan-ms": `${SCAN_MS}ms` } as React.CSSProperties}>
      <div className="relative mx-auto w-[min(94vw,1180px)] md:w-[min(72vw,1180px)]">
        <div className="theater-glow" aria-hidden="true" />
        <figure className="theater-stage relative" style={{ containerType: "inline-size" }}>
          <picture>
            <source type="image/avif" srcSet={srcSet(before, "avif")} sizes={sizes} />
            <source type="image/webp" srcSet={srcSet(before, "webp")} sizes={sizes} />
            <img
              src={before.variants[0].webp}
              width={before.w}
              height={before.h}
              alt={h.imgAlt}
              loading="eager"
              decoding="sync"
              fetchPriority="high"
              className="block w-full"
            />
          </picture>
          {gl !== "off" && (
            <DeskGl
              ref={glRef}
              beforeUrl={texUrl(before)}
              afterUrl={texUrl(after)}
              onReady={glReady}
              onFail={glFail}
              className={`absolute inset-0 h-full w-full transition-opacity duration-300 ${gl === "on" ? "opacity-100" : "opacity-0"}`}
            />
          )}
          {useCss && (
            <>
              <div className="theater-after absolute inset-0" aria-hidden="true">
                <picture>
                  <source type="image/avif" srcSet={srcSet(after, "avif")} sizes={sizes} />
                  <source type="image/webp" srcSet={srcSet(after, "webp")} sizes={sizes} />
                  <img
                    ref={afterImgRef}
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
              {featured.map((c, i) => (
                <span
                  key={i}
                  className="flip-tile"
                  aria-hidden="true"
                  style={
                    {
                      left: `${c.x}%`,
                      top: `${c.y}%`,
                      width: `${c.w}%`,
                      height: `${c.h}%`,
                      "--flip-delay": `${Math.round((c.x / 100) * SCAN_MS * 0.82)}ms`,
                      "--bg-size": `${(100 / c.w) * 100}% ${(100 / c.h) * 100}%`,
                      "--bg-pos": `${(c.x / (100 - c.w)) * 100}% ${(c.y / (100 - c.h)) * 100}%`,
                    } as React.CSSProperties
                  }
                >
                  <span className="flip-face" style={{ backgroundImage: tileImage }} />
                  <span className="flip-face flip-back" style={{ backgroundImage: tileImageAfter }} />
                </span>
              ))}
              <div className="pointer-events-none absolute inset-0 overflow-hidden" aria-hidden="true">
                <span className="theater-scanline" />
              </div>
            </>
          )}
          <figcaption
            className="absolute bottom-3 left-3 rounded-full bg-black/40 px-3 py-1 text-[12.5px] font-medium text-white backdrop-blur-sm"
            aria-live="polite"
          >
            {styled ? h.statusRefreshed : h.stageBefore}
          </figcaption>
          <button
            type="button"
            onClick={toggle}
            disabled={busy}
            className="absolute bottom-3 right-3 rounded-full border border-white/25 bg-black/40 px-4 py-1.5 text-[13px] font-semibold text-white backdrop-blur-sm transition-[background-color,border-color] duration-150 hover:border-coral hover:bg-black/60 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-coral active:scale-[0.97] disabled:opacity-60"
          >
            {styled ? h.putBack : h.beautify}
          </button>
        </figure>
        <div
          aria-hidden="true"
          className="pointer-events-none mx-auto mt-[2px] h-[22%] w-full opacity-[0.09] [mask-image:linear-gradient(to_bottom,black,transparent_72%)]"
        >
          <img src={after.variants.at(-1)!.webp} alt="" loading="lazy" decoding="async" className="w-full -scale-y-100" width={after.w} height={after.h} />
        </div>
      </div>
      <noscript>
        <style>{`.theater-after{clip-path:none!important}.hero-word{opacity:1!important;transform:none!important}`}</style>
      </noscript>
    </div>
  );
}
