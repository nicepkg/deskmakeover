import type { Dict } from "@/content/types";
import { Pic } from "@/components/pic";
import { Reveal } from "@/components/reveal";

export function Zones({ dict }: { dict: Dict }) {
  const z = dict.zones;
  return (
    <section>
      <div className="mx-auto max-w-[1200px] px-5 pb-[clamp(5rem,10vw,7.5rem)] md:px-8">
        <div className="grid items-center gap-12 md:grid-cols-[7fr_5fr]">
          <Reveal className="order-2 md:order-1">
            <figure>
              <Pic
                name="feature-zones"
                alt={z.imgAlt}
                sizes="(max-width: 768px) 92vw, (max-width: 1240px) 55vw, 660px"
                imgClassName="w-full"
              />
              <figcaption className="mt-3 text-center text-[14px] text-ink-soft">{z.caption}</figcaption>
            </figure>
          </Reveal>
          <Reveal delay={100} className="order-1 md:order-2">
            <div>
              <h2 className="font-display text-[clamp(1.9rem,3.6vw,2.6rem)] font-semibold leading-[1.12] tracking-[-0.01em]">
                {z.title}
              </h2>
              <p className="mt-4 max-w-[46ch] text-[18px] text-ink-soft">{z.body}</p>
            </div>
          </Reveal>
        </div>
        <Reveal delay={60}>
          <div className="mt-16 rounded-card border border-hairline bg-mist px-7 py-6 md:px-9">
            <h3 className="text-[19px] font-semibold">{z.arrowTitle}</h3>
            <p className="mt-2 max-w-[72ch] text-ink-soft">{z.arrowBody}</p>
          </div>
        </Reveal>
      </div>
    </section>
  );
}
