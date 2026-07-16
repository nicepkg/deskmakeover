import type { Dict } from "@/content/types";
import { HeroTheater } from "@/components/hero-theater";
import { DOWNLOAD_URL, RELEASE_READY } from "@/lib/site";

function StaggeredLine({ text, from, coral }: { text: string; from: number; coral?: boolean }) {
  const words = text.split(/(\s+)/).filter((w) => w.length > 0);
  let i = from;
  return (
    <span className={coral ? "text-coral" : undefined}>
      {words.map((w, k) =>
        /^\s+$/.test(w) ? (
          " "
        ) : (
          <span key={k} className="hero-word" style={{ "--word-delay": `${250 + i++ * 60}ms` } as React.CSSProperties}>
            {w}
          </span>
        ),
      )}
    </span>
  );
}

export function Hero({ dict }: { dict: Dict }) {
  const h = dict.hero;
  const isZh = dict.locale === "zh";
  return (
    <section className="relative overflow-hidden pb-10 pt-10 md:pt-14">
      {/* faint grid + vignette behind everything */}
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 [background-image:linear-gradient(rgb(47_54_61/0.045)_1px,transparent_1px),linear-gradient(90deg,rgb(47_54_61/0.045)_1px,transparent_1px)] [background-size:64px_64px] [mask-image:radial-gradient(ellipse_70%_60%_at_50%_35%,black,transparent)]"
      />
      <div className="relative mx-auto max-w-[1240px] px-5 md:px-8">
        <div className="mx-auto max-w-[1000px] text-center">
          <h1 className="font-display text-[clamp(2.4rem,5vw,4.4rem)] font-bold leading-[0.98] tracking-[-0.03em]">
            <StaggeredLine text={h.headline1} from={0} />
            <br />
            <StaggeredLine text={h.headline2} from={isZh ? 2 : 6} coral />
          </h1>
          <p className="mx-auto mt-4 max-w-[52ch] text-[clamp(1.05rem,1.4vw,1.3rem)] text-text-mid">{h.sub}</p>
          <div className="mt-6 flex flex-wrap items-center justify-center gap-3">
            {RELEASE_READY ? (
              <a
                href={DOWNLOAD_URL}
                className="rounded-btn bg-coral-deep px-7 py-3.5 text-[17px] font-semibold text-white transition-colors duration-150 hover:bg-coral focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-coral active:scale-[0.99]"
              >
                {h.ctaRelease}
              </a>
            ) : (
              <span className="rounded-btn bg-coral-deep px-7 py-3.5 text-[17px] font-semibold text-white">
                {h.ctaPending}
              </span>
            )}
            <a
              href="#looks"
              className="group rounded-btn border border-hairline px-6 py-3.5 text-[16px] font-medium text-text-mid transition-colors hover:border-text-dim hover:text-text-hi"
            >
              {h.ctaSecondary}
              <span aria-hidden="true" className="ml-1.5 inline-block transition-transform duration-200 group-hover:translate-x-0.5">
                →
              </span>
            </a>
          </div>
          <p className="mt-4 text-[13.5px] text-text-dim">{h.trust}</p>
        </div>
        <div className="mt-8 md:mt-10">
          <HeroTheater dict={dict} />
        </div>
      </div>
    </section>
  );
}
