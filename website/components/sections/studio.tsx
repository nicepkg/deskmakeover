import type { Dict } from "@/content/types";
import { Pic } from "@/components/pic";
import { Reveal } from "@/components/reveal";

export function Studio({ dict }: { dict: Dict }) {
  const s = dict.studio;
  return (
    <section>
      <div className="mx-auto max-w-[1200px] px-5 pb-[clamp(5rem,10vw,7.5rem)] md:px-8">
        <Reveal>
          <h2 className="font-display text-[clamp(1.9rem,3.6vw,2.6rem)] font-semibold leading-[1.12] tracking-[-0.01em]">
            {s.title}
          </h2>
          <p className="mt-3 max-w-[58ch] text-[18px] text-ink-soft">{s.body}</p>
        </Reveal>
        <Reveal delay={80}>
          <figure className="mt-10">
            <Pic
              name="app-studio"
              alt={s.imgAlt}
              sizes="(max-width: 1240px) 92vw, 1100px"
              imgClassName="w-full"
            />
            <figcaption className="mt-3 text-center text-[14px] text-ink-soft">{s.caption}</figcaption>
          </figure>
        </Reveal>
        <div className="mt-16 grid items-center gap-10 md:grid-cols-[2fr_3fr]">
          <Reveal>
            <Pic
              name="feature-stylepack"
              alt={s.packImgAlt}
              sizes="(max-width: 768px) 80vw, 400px"
              imgClassName="mx-auto w-full max-w-[400px]"
            />
          </Reveal>
          <Reveal delay={100}>
            <div>
              <h3 className="font-display text-[clamp(1.5rem,2.6vw,1.9rem)] font-semibold leading-[1.15]">
                {s.packTitle}
              </h3>
              <p className="mt-3 max-w-[50ch] text-[17px] text-ink-soft">{s.packBody}</p>
            </div>
          </Reveal>
        </div>
      </div>
    </section>
  );
}
