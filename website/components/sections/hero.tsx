import type { Dict } from "@/content/types";
import { HeroStage } from "@/components/hero-stage";
import { DOWNLOAD_URL, RELEASE_READY } from "@/lib/site";

export function Hero({ dict }: { dict: Dict }) {
  const h = dict.hero;
  return (
    <section className="mx-auto max-w-[1200px] px-5 pt-14 md:px-8 md:pt-20">
      <div>
        <h1 className="font-display text-[clamp(2.2rem,4.6vw,3.5rem)] font-bold leading-[1.08] tracking-[-0.02em]">
          {h.headline1}
          <br />
          <span className="text-coral-text">{h.headline2}</span>
        </h1>
        <p className="mt-5 max-w-[54ch] text-[18px] leading-relaxed text-ink-soft">{h.sub}</p>
        <div className="mt-8 flex flex-wrap items-center gap-4">
          <a
            href={RELEASE_READY ? DOWNLOAD_URL : "#download"}
            className="rounded-btn bg-gradient-to-br from-coral to-coral-deep px-6 py-3 text-[16px] font-semibold text-cream shadow-lift transition-transform duration-150 hover:brightness-[1.05] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-coral active:scale-[0.98]"
          >
            {RELEASE_READY ? h.ctaRelease : h.ctaPending}
          </a>
          <a
            href="#looks"
            className="rounded-btn px-4 py-3 text-[16px] font-medium text-ink-soft transition-colors hover:text-ink"
          >
            {h.ctaSecondary}
          </a>
        </div>
      </div>
      <div className="mt-12 md:mt-16">
        <HeroStage dict={h} />
      </div>
    </section>
  );
}
