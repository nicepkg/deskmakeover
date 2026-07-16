"use client";

import { useCallback, useRef, useState } from "react";
import type { Dict } from "@/content/types";
import { imageEntry, imageMeta } from "@/components/pic";
import { Reveal } from "@/components/reveal";

/**
 * One desktop, nine outfits: a big live stage crossfades between real
 * full-desktop renders on chip click. Only the active and previously shown
 * frames are mounted; the rest load on demand (decode before swap).
 */
export function Looks({ dict }: { dict: Dict }) {
  const l = dict.looks;
  const { chips } = imageMeta();
  const [active, setActive] = useState("preset-squircle");
  const [mounted, setMounted] = useState<string[]>(["preset-squircle"]);
  const pending = useRef<string | null>(null);

  const styleOf = (img: string) => img.replace("preset-", "");
  const entryOf = (img: string) => imageEntry(`desk-${styleOf(img)}`);

  const activate = useCallback(
    (img: string) => {
      if (img === active) return;
      pending.current = img;
      const entry = entryOf(img);
      const probe = new Image();
      probe.src = entry.variants[0].avif;
      probe
        .decode()
        .catch(() => undefined)
        .then(() => {
          if (pending.current !== img) return;
          setMounted((m) => (m.includes(img) ? m : [...m, img]));
          setActive(img);
        });
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [active],
  );

  const prefetch = useCallback((img: string) => {
    const entry = entryOf(img);
    const probe = new Image();
    probe.src = entry.variants[0].avif;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const activePreset = l.presets.find((p) => p.img === active) ?? l.presets[0];

  return (
    <section id="looks" className="scroll-mt-20">
      <div className="mx-auto max-w-[1240px] px-5 py-[clamp(5rem,10vw,8rem)] md:px-8">
        <Reveal>
          <div className="text-center">
            <h2 className="font-display text-[clamp(2rem,3.6vw,3.2rem)] font-semibold leading-[1.05] tracking-[-0.02em]">
              {l.title}
            </h2>
            <p className="mx-auto mt-3 max-w-[58ch] text-[clamp(1.02rem,1.3vw,1.2rem)] text-text-mid">{l.sub}</p>
          </div>
        </Reveal>
        <Reveal delay={80}>
          <div className="relative mx-auto mt-10 max-w-[1080px]">
            <figure className="relative overflow-hidden rounded-card border border-hairline bg-surface-1 shadow-card">
              <div className="relative" style={{ aspectRatio: "2000 / 1124" }}>
                {mounted.map((img) => {
                  const entry = entryOf(img);
                  const v = entry.variants[0];
                  return (
                    <picture key={img} className={`switch-frame absolute inset-0 ${img === active ? "active" : ""}`}>
                      <source type="image/avif" srcSet={v.avif} />
                      <source type="image/webp" srcSet={v.webp} />
                      <img
                        src={v.webp}
                        width={entry.w}
                        height={entry.h}
                        alt={img === active ? `${activePreset.name}: ${l.specimenAlt}` : ""}
                        loading="lazy"
                        decoding="async"
                        className="absolute inset-0 h-full w-full object-cover"
                      />
                    </picture>
                  );
                })}
                <span
                  aria-hidden="true"
                  className="pointer-events-none absolute bottom-2 right-5 select-none font-display text-[clamp(3rem,8vw,7rem)] font-bold leading-none text-[#2f363d]/[0.05]"
                >
                  {activePreset.name}
                </span>
              </div>
            </figure>
          </div>
        </Reveal>
        <Reveal delay={140}>
          <div
            role="tablist"
            aria-label={l.title}
            className="mx-auto mt-6 flex max-w-[1080px] snap-x gap-2 overflow-x-auto pb-2 [scrollbar-width:thin] md:justify-center"
          >
            {l.presets.map((p) => {
              const style = styleOf(p.img);
              const isActive = p.img === active;
              const chip = chips[style];
              return (
                <button
                  key={p.img}
                  role="tab"
                  aria-selected={isActive}
                  onClick={() => activate(p.img)}
                  onPointerEnter={() => prefetch(p.img)}
                  onFocus={() => prefetch(p.img)}
                  className={`group flex shrink-0 snap-start flex-col items-center gap-1.5 rounded-[14px] border px-3 pb-2.5 pt-3 transition-[border-color,background-color,transform] duration-150 hover:-translate-y-0.5 ${
                    isActive
                      ? "border-coral/70 bg-surface-2 ring-1 ring-coral/60"
                      : "border-hairline bg-surface-1 hover:bg-surface-2"
                  }`}
                >
                  <picture>
                    <source type="image/avif" srcSet={chip.avif} />
                    <img src={chip.webp} width={56} height={56} alt="" loading="lazy" decoding="async" className="rounded-[10px]" />
                  </picture>
                  <span className={`text-[13.5px] font-semibold ${isActive ? "text-coral" : "text-text-hi"}`}>{p.name}</span>
                  <span className="max-w-[9.5rem] text-center text-[11.5px] leading-tight text-text-dim">{p.tagline}</span>
                </button>
              );
            })}
          </div>
        </Reveal>
      </div>
    </section>
  );
}
