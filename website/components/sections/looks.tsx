import type { Dict } from "@/content/types";
import { Pic } from "@/components/pic";
import { Reveal } from "@/components/reveal";

export function Looks({ dict }: { dict: Dict }) {
  const l = dict.looks;
  return (
    <section id="looks" className="scroll-mt-20 bg-mist">
      <div className="mx-auto max-w-[1200px] px-5 py-[clamp(5rem,10vw,7.5rem)] md:px-8">
        <Reveal>
          <h2 className="font-display text-[clamp(1.9rem,3.6vw,2.6rem)] font-semibold leading-[1.12] tracking-[-0.01em]">
            {l.title}
          </h2>
          <p className="mt-3 max-w-[58ch] text-[18px] text-ink-soft">{l.sub}</p>
        </Reveal>
        <Reveal delay={80}>
          <figure className="mt-10 rounded-card border border-hairline bg-paper px-6 py-7 shadow-lift">
            <Pic
              name="specimen-nine-styles"
              alt={l.specimenAlt}
              sizes="(max-width: 1240px) 92vw, 1100px"
              imgClassName="w-full"
            />
            <figcaption className="mt-4 text-center text-[14px] text-ink-soft">{l.specimenCaption}</figcaption>
          </figure>
        </Reveal>
        <div className="mt-14 grid grid-cols-2 gap-x-6 gap-y-10 sm:grid-cols-3">
          {l.presets.map((preset, i) => (
            <Reveal key={preset.img} delay={(i % 3) * 60 + Math.floor(i / 3) * 40}>
              <figure className="group">
                <div className="transition-transform duration-200 ease-out [@media(hover:hover)]:group-hover:-translate-y-1">
                  <Pic
                    name={preset.img}
                    alt={`${preset.name} preset`}
                    sizes="(max-width: 640px) 45vw, (max-width: 1240px) 30vw, 350px"
                    imgClassName="w-full"
                  />
                </div>
                <figcaption className="mt-3">
                  <span className="block text-[16px] font-semibold">{preset.name}</span>
                  <span className="block text-[14px] text-ink-soft">{preset.tagline}</span>
                </figcaption>
              </figure>
            </Reveal>
          ))}
        </div>
      </div>
    </section>
  );
}
