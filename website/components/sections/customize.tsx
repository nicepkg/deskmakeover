import { CheckIcon } from "@/components/icons";
import type { Dict } from "@/content/types";
import { Pic } from "@/components/pic";
import { Reveal } from "@/components/reveal";

export function Customize({ dict }: { dict: Dict }) {
  const c = dict.customize;
  return (
    <section id="features" className="scroll-mt-20">
      <div className="mx-auto grid max-w-[1200px] items-center gap-12 px-5 py-[clamp(5rem,10vw,7.5rem)] md:grid-cols-[5fr_7fr] md:px-8">
        <Reveal>
          <div>
            <h2 className="font-display text-[clamp(1.9rem,3.6vw,2.6rem)] font-semibold leading-[1.12] tracking-[-0.01em]">
              {c.title}
            </h2>
            <p className="mt-4 max-w-[46ch] text-[18px] text-ink-soft">{c.body}</p>
            <ul className="mt-6 space-y-3">
              {c.bullets.map((b) => (
                <li key={b} className="flex items-start gap-2.5 text-[16px]">
                  <CheckIcon size={18} className="mt-1 shrink-0 text-coral-text" aria-hidden="true" />
                  {b}
                </li>
              ))}
            </ul>
          </div>
        </Reveal>
        <Reveal delay={100}>
          <figure>
            <Pic
              name="feature-combine"
              alt={c.imgAlt}
              sizes="(max-width: 768px) 92vw, (max-width: 1240px) 55vw, 660px"
              imgClassName="w-full"
            />
            <figcaption className="mt-3 text-center text-[14px] text-ink-soft">{c.caption}</figcaption>
          </figure>
        </Reveal>
      </div>
    </section>
  );
}
