import type { Dict, PointEntry, SectionHead } from "@/content/types";
import { BeforeAfter } from "@/components/before-after";
import { StyleWall } from "@/components/style-wall";
import { Reveal } from "@/components/reveal";
import { Zoomable } from "@/components/zoomable";
import { DOWNLOAD_URL, GITHUB_URL, RELEASE_READY } from "@/lib/site";

function SectionHeader({ head, className }: { head: SectionHead; className?: string }) {
  return (
    <div className={className}>
      <p className="font-mono text-[12px] tracking-[0.22em] text-ink-3">
        <span className="text-coral-ink">{head.index}</span>
        {"  ·  "}
        {head.kicker.toUpperCase()}
      </p>
      <h2 className="mt-4 max-w-[24ch] text-[30px] font-bold leading-[1.12] tracking-[-0.015em] md:text-[40px]">
        {head.title}
      </h2>
      <p className="mt-4 max-w-[46rem] text-[16px] leading-[1.6] text-ink-2">{head.body}</p>
    </div>
  );
}

function Points({ points }: { points: PointEntry[] }) {
  return (
    <div className="mt-8 space-y-6">
      {points.map((p) => (
        <div key={p.title}>
          <h3 className="text-[15px] font-semibold text-ink">{p.title}</h3>
          <p className="mt-1 text-[15px] leading-[1.6] text-ink-2">{p.body}</p>
        </div>
      ))}
    </div>
  );
}

export function ProofSection({ dict }: { dict: Dict }) {
  const s = dict.proof;
  return (
    <section id="proof" className="mx-auto max-w-[1200px] px-5 py-20 md:px-8 md:py-28">
      <Reveal>
        <SectionHeader head={s} />
      </Reveal>
      <Reveal delay={80} className="mt-10">
        <BeforeAfter dragHint={s.dragHint} altBefore={s.altBefore} altAfter={s.altAfter} />
      </Reveal>
    </section>
  );
}

export function LooksSection({ dict }: { dict: Dict }) {
  const s = dict.looks;
  return (
    <section id="looks" className="border-t border-line">
      <div className="mx-auto max-w-[1200px] px-5 py-20 md:px-8 md:py-28">
        <Reveal>
          <SectionHeader head={s} />
        </Reveal>
        <Reveal delay={80} className="mt-10">
          <StyleWall
            styles={s.styles}
            altPrefix={s.altPrefix}
            zoomHint={dict.ui.zoomHint}
            closeLabel={dict.ui.zoomClose}
          />
        </Reveal>
      </div>
    </section>
  );
}

export function ZonesSection({ dict }: { dict: Dict }) {
  const s = dict.zones;
  return (
    <section id="zones" className="border-t border-line">
      <div className="mx-auto grid max-w-[1200px] items-center gap-10 px-5 py-20 md:px-8 md:py-28 lg:grid-cols-12">
        <Reveal className="lg:col-span-7">
          <Zoomable
            id="studio-zones"
            alt={s.imgAlt}
            sizes="(min-width: 1024px) 58vw, 92vw"
            zoomHint={dict.ui.zoomHint}
            closeLabel={dict.ui.zoomClose}
          />
        </Reveal>
        <Reveal delay={80} className="lg:col-span-5">
          <SectionHeader head={s} />
          <Points points={s.points} />
        </Reveal>
      </div>
    </section>
  );
}

export function StudioSection({ dict }: { dict: Dict }) {
  const s = dict.studio;
  return (
    <section id="studio" className="border-t border-line">
      <div className="mx-auto grid max-w-[1200px] items-center gap-10 px-5 py-20 md:px-8 md:py-28 lg:grid-cols-12">
        <Reveal className="lg:order-2 lg:col-span-7">
          <Zoomable
            id="studio-icons"
            alt={s.imgAlt}
            sizes="(min-width: 1024px) 58vw, 92vw"
            zoomHint={dict.ui.zoomHint}
            closeLabel={dict.ui.zoomClose}
          />
        </Reveal>
        <Reveal delay={80} className="lg:order-1 lg:col-span-5">
          <SectionHeader head={s} />
          <Points points={s.points} />
        </Reveal>
      </div>
    </section>
  );
}

export function DownloadSection({ dict }: { dict: Dict }) {
  const s = dict.download;
  return (
    <section id="download" className="border-t border-line bg-coral">
      <div className="mx-auto max-w-[1200px] px-5 py-20 md:px-8 md:py-28">
        <Reveal>
          <p className="font-mono text-[12px] tracking-[0.22em] text-white/90">
            {s.index}
            {"  ·  "}
            {s.kicker.toUpperCase()}
          </p>
          <h2 className="mt-4 text-[34px] font-bold leading-[1.1] tracking-[-0.015em] !text-white md:text-[48px]">
            {s.title}
          </h2>
          <p className="mt-4 max-w-[38rem] text-[16px] leading-[1.6] text-white/90">{s.body}</p>
          <div className="mt-9 flex flex-wrap items-center gap-3">
            {RELEASE_READY ? (
              <a
                href={DOWNLOAD_URL}
                // fixed white-on-coral chip: themed ink would flip light in dark mode
                className="w-full bg-white px-7 py-3.5 text-center text-[16px] font-semibold text-[#16181d] transition-colors hover:bg-white/85 sm:w-auto"
              >
                {s.ctaRelease}
              </a>
            ) : (
              <span className="w-full bg-white/20 px-7 py-3.5 text-center text-[16px] font-semibold text-white sm:w-auto">
                {s.ctaPending}
              </span>
            )}
            <a
              href={GITHUB_URL}
              target="_blank"
              rel="noreferrer"
              className="w-full border border-white/50 px-7 py-3.5 text-center text-[16px] font-semibold text-white transition-colors hover:border-white sm:w-auto"
            >
              {s.watchGithub}
            </a>
          </div>
          {RELEASE_READY ? (
            <div className="mt-8 max-w-[44rem] border-t border-white/25 pt-6">
              <p className="text-[14px] font-semibold text-white">{s.smartscreenLead}</p>
              <p className="mt-2 text-[14px] leading-[1.6] text-white/95">{s.smartscreenDetail}</p>
            </div>
          ) : (
            <p className="mt-8 max-w-[44rem] text-[14px] leading-[1.6] text-white/95">
              {s.pendingNote}
            </p>
          )}
          <p className="mt-6 font-mono text-[11px] tracking-[0.14em] text-white/90">
            {s.requirements}
          </p>
        </Reveal>
      </div>
    </section>
  );
}

export function FaqSection({ dict }: { dict: Dict }) {
  const s = dict.faq;
  return (
    <section id="faq" className="border-t border-line">
      <div className="mx-auto max-w-[1200px] px-5 py-20 md:px-8 md:py-24">
        <Reveal>
          <p className="font-mono text-[12px] tracking-[0.22em] text-ink-3">
            <span className="text-coral-ink">06</span>
            {"  ·  "}
            {s.kicker.toUpperCase()}
          </p>
          <h2 className="mt-4 text-[26px] font-bold tracking-[-0.01em] md:text-[32px]">{s.title}</h2>
        </Reveal>
        <div className="mt-10 grid gap-x-12 gap-y-9 md:grid-cols-2">
          {s.items.map((item, i) => (
            <Reveal key={item.q} delay={(i % 2) * 60}>
              <h3 className="text-[16px] font-semibold leading-[1.4]">{item.q}</h3>
              <p className="mt-2 text-[15px] leading-[1.65] text-ink-2">{item.a}</p>
            </Reveal>
          ))}
        </div>
      </div>
    </section>
  );
}
