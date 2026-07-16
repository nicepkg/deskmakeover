import type { Dict } from "@/content/types";
import { Reveal } from "@/components/reveal";

/**
 * All answers live in the static DOM (no accordion): better for visitors
 * skimming and for search / AI crawlers quoting them standalone.
 */
export function Faq({ dict }: { dict: Dict }) {
  const f = dict.faq;
  return (
    <section id="faq" className="scroll-mt-20">
      <div className="mx-auto max-w-[1200px] px-5 py-[clamp(5rem,10vw,7.5rem)] md:px-8">
        <Reveal>
          <h2 className="font-display text-[clamp(1.9rem,3.6vw,2.6rem)] font-semibold leading-[1.12] tracking-[-0.01em]">
            {f.title}
          </h2>
        </Reveal>
        <dl className="mt-10 grid gap-x-14 gap-y-9 md:grid-cols-2">
          {f.items.map((item, i) => (
            <Reveal key={item.q} delay={(i % 2) * 60}>
              <div className="border-t border-hairline pt-5">
                <dt className="text-[17px] font-semibold">{item.q}</dt>
                <dd className="mt-2 text-[15px] leading-relaxed text-ink-soft">{item.a}</dd>
              </div>
            </Reveal>
          ))}
        </dl>
      </div>
    </section>
  );
}
