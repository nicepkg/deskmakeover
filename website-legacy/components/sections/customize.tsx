import type { Dict } from "@/content/types";
import { Pic } from "@/components/pic";
import { Reveal } from "@/components/reveal";

export function Customize({ dict }: { dict: Dict }) {
  const c = dict.customize;
  return (
    <section id="features" className="scroll-mt-20">
      <div className="mx-auto max-w-[1240px] px-5 pb-[clamp(5rem,10vw,8rem)] md:px-8">
        <div className="grid items-center gap-10 lg:grid-cols-[58fr_42fr]">
          <Reveal className="order-2 lg:order-1">
            <figure className="overflow-hidden rounded-card border border-hairline shadow-card">
              <Pic
                name="studio-icons"
                alt={c.imgAlt}
                sizes="(max-width: 1024px) 92vw, 700px"
                imgClassName="w-full"
              />
            </figure>
          </Reveal>
          <Reveal delay={100} className="order-1 lg:order-2">
            <div>
              <h2 className="font-display text-[clamp(2rem,3.6vw,3.2rem)] font-semibold leading-[1.05] tracking-[-0.02em]">
                {c.title}
              </h2>
              <p className="mt-3 text-[clamp(1.02rem,1.3vw,1.2rem)] text-text-mid">{c.body}</p>
              <div className="mt-7">
                {c.rows.map((row) => (
                  <div key={row.title} className="border-t border-hairline py-4 last:border-b">
                    <h3 className="text-[16.5px] font-semibold text-text-hi">{row.title}</h3>
                    <p className="mt-1 text-[14.5px] leading-relaxed text-text-dim">{row.body}</p>
                  </div>
                ))}
              </div>
            </div>
          </Reveal>
        </div>
      </div>
    </section>
  );
}
