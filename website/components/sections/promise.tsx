"use client";

import { useState } from "react";
import type { Dict } from "@/content/types";
import { imageEntry } from "@/components/pic";
import { Reveal } from "@/components/reveal";

/**
 * The promise as a working control: a mini desktop that flips between
 * styled and original, because "you can always go back" is better proven
 * than claimed.
 */
export function Promise({ dict }: { dict: Dict }) {
  const p = dict.promise;
  const [styled, setStyled] = useState(true);
  const before = imageEntry("desk-before");
  const after = imageEntry("desk-squircle");
  const v = (e: typeof before) => e.variants.at(-1)!;

  return (
    <section className="relative">
      <div className="mx-auto max-w-[1240px] px-5 pb-[clamp(5rem,10vw,8rem)] pt-[clamp(2rem,4vw,3.5rem)] md:px-8">
        <Reveal>
          <div className="mx-auto max-w-[780px] text-center">
            <h2 className="font-display text-[clamp(2rem,3.6vw,3.2rem)] font-semibold leading-[1.05] tracking-[-0.02em]">
              {p.title}
            </h2>
            <p className="mx-auto mt-4 max-w-[56ch] text-[clamp(1.02rem,1.3vw,1.2rem)] text-text-mid">{p.lead}</p>
          </div>
        </Reveal>
        <Reveal delay={80}>
          <div className="mx-auto mt-10 max-w-[760px]">
            <div className="overflow-hidden rounded-card border border-hairline shadow-card">
              <div className="relative" style={{ aspectRatio: "2000 / 1124" }}>
                <picture className={`switch-frame absolute inset-0 ${styled ? "" : "active"}`}>
                  <source type="image/avif" srcSet={v(before).avif} />
                  <source type="image/webp" srcSet={v(before).webp} />
                  <img src={v(before).webp} width={before.w} height={before.h} alt="" loading="lazy" decoding="async" className="absolute inset-0 h-full w-full object-cover" />
                </picture>
                <picture className={`switch-frame absolute inset-0 ${styled ? "active" : ""}`}>
                  <source type="image/avif" srcSet={v(after).avif} />
                  <source type="image/webp" srcSet={v(after).webp} />
                  <img src={v(after).webp} width={after.w} height={after.h} alt={dict.hero.imgAlt} loading="lazy" decoding="async" className="absolute inset-0 h-full w-full object-cover" />
                </picture>
              </div>
            </div>
            <div className="mt-5 flex justify-center">
              <div role="group" className="inline-flex rounded-full border border-hairline bg-surface-1 p-1">
                <button
                  type="button"
                  aria-pressed={!styled}
                  onClick={() => setStyled(false)}
                  className={`rounded-full px-5 py-2 text-[14px] font-semibold transition-colors duration-150 ${
                    !styled ? "bg-surface-2 text-text-hi" : "text-text-dim hover:text-text-mid"
                  }`}
                >
                  {p.toggleOriginal}
                </button>
                <button
                  type="button"
                  aria-pressed={styled}
                  onClick={() => setStyled(true)}
                  className={`rounded-full px-5 py-2 text-[14px] font-semibold transition-colors duration-150 ${
                    styled ? "bg-coral-ink text-white" : "text-text-dim hover:text-text-mid"
                  }`}
                >
                  {p.toggleStyled}
                </button>
              </div>
            </div>
          </div>
        </Reveal>
        <div className="mx-auto mt-14 grid max-w-[1080px] gap-x-10 gap-y-8 sm:grid-cols-2 lg:grid-cols-4">
          {p.items.map((item, i) => (
            <Reveal key={item.title} delay={i * 60}>
              <div className="border-t border-hairline pt-4">
                <h3 className="text-[16px] font-semibold text-text-hi">{item.title}</h3>
                <p className="mt-1.5 text-[14px] leading-relaxed text-text-dim">{item.body}</p>
              </div>
            </Reveal>
          ))}
        </div>
      </div>
    </section>
  );
}
