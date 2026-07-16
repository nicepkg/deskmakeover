import type { Dict } from "@/content/types";
import { Reveal } from "@/components/reveal";

export function Promise({ dict }: { dict: Dict }) {
  const p = dict.promise;
  return (
    <section className="mx-auto max-w-[1200px] px-5 py-[clamp(5rem,10vw,7.5rem)] md:px-8">
      <Reveal>
        <h2 className="font-display text-[clamp(1.9rem,3.6vw,2.6rem)] font-semibold leading-[1.12] tracking-[-0.01em]">
          {p.title}
        </h2>
      </Reveal>
      <div className="mt-10 grid gap-x-14 gap-y-10 md:grid-cols-2">
        {p.items.map((item, i) => (
          <Reveal key={item.title} delay={i * 60}>
            <div className="border-t border-hairline pt-6">
              <h3 className="text-[19px] font-semibold">{item.title}</h3>
              <p className="mt-2 max-w-[48ch] text-ink-soft">{item.body}</p>
            </div>
          </Reveal>
        ))}
      </div>
    </section>
  );
}
