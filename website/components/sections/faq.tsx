import type { Dict } from "@/content/types";
import { Reveal } from "@/components/reveal";

/**
 * Quiet accordion. Answers stay in the static DOM (details/summary), so
 * crawlers and reader modes see the full text.
 */
export function Faq({ dict }: { dict: Dict }) {
  const f = dict.faq;
  const mid = Math.ceil(f.items.length / 2);
  const cols = [f.items.slice(0, mid), f.items.slice(mid)];
  return (
    <section id="faq" className="scroll-mt-20">
      <div className="mx-auto max-w-[1240px] px-5 pb-[clamp(5rem,10vw,8rem)] md:px-8">
        <Reveal>
          <h2 className="font-display text-[clamp(2rem,3.6vw,3.2rem)] font-semibold leading-[1.05] tracking-[-0.02em]">
            {f.title}
          </h2>
        </Reveal>
        <div className="mt-8 grid items-start gap-x-12 md:grid-cols-2">
          {cols.map((col, ci) => (
            <div key={ci}>
              {col.map((item, i) => (
                <Reveal key={item.q} delay={i * 40}>
                  <details className="faq-item border-t border-hairline">
                    <summary className="flex items-center justify-between gap-4 py-4 text-[16px] font-semibold text-text-hi transition-colors hover:text-coral-ink">
                      {item.q}
                      <span aria-hidden="true" className="faq-plus shrink-0 text-[20px] font-normal text-coral">
                        +
                      </span>
                    </summary>
                    <p className="pb-5 pr-8 text-[15px] leading-relaxed text-text-mid">{item.a}</p>
                  </details>
                </Reveal>
              ))}
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
