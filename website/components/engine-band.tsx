import type { Dict } from "@/content/types";
import { Reveal } from "@/components/reveal";

/**
 * The un-numbered hairline strip between Studio and Download (spec §IA):
 * one claim + CTA into /engine/. Deliberately compact — a fold-line, not a
 * section, so the landing's 01–05 numbering stays untouched.
 */
export function EngineBand({ dict }: { dict: Dict }) {
  const b = dict.engineBand;
  const href = dict.locale === "zh" ? "/zh/engine/" : "/engine/";
  return (
    <section className="border-t border-line bg-panel">
      <div className="mx-auto flex max-w-[1200px] flex-col gap-6 px-5 py-12 md:flex-row md:items-center md:justify-between md:px-8 md:py-14">
        <Reveal>
          <p className="font-mono text-[12px] tracking-[0.22em] text-ink-3">
            <span className="text-coral-ink">◆</span>
            {"  "}
            {b.kicker.toUpperCase()}
          </p>
          <h2 className="mt-3 max-w-[26ch] text-[24px] font-bold leading-[1.15] tracking-[-0.01em] md:text-[30px]">
            {b.title}
          </h2>
          <p className="mt-3 max-w-[44rem] text-[15.5px] leading-[1.6] text-ink-2">{b.body}</p>
        </Reveal>
        <Reveal delay={80} className="md:flex-none">
          <a
            href={href}
            className="group inline-flex h-11 items-center gap-2.5 border border-ink px-6 text-[14px] font-semibold text-ink transition-colors hover:bg-ink hover:text-canvas"
          >
            {b.cta}
            <span aria-hidden className="transition-transform group-hover:translate-x-0.5">→</span>
          </a>
        </Reveal>
      </div>
    </section>
  );
}
