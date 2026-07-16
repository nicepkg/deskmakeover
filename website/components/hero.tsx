import type { Dict } from "@/content/types";
import { MonitorScene } from "@/components/monitor-scene";
import { texture } from "@/lib/manifest";
import { DOWNLOAD_URL, GITHUB_URL, RELEASE_READY } from "@/lib/site";

/**
 * First viewport, whole story: copy hard-left, the live 3D monitor filling
 * the right, download reachable without a single scroll.
 */
export function Hero({ dict }: { dict: Dict }) {
  const h = dict.hero;
  return (
    <section className="relative overflow-hidden border-b border-line">
      <div aria-hidden className="grid-paper absolute inset-0" />
      <div className="relative mx-auto max-w-[1200px] px-5 md:px-8">
        <div className="grid min-h-[calc(100svh-3.5rem-4rem)] items-center gap-10 py-10 lg:grid-cols-12 lg:gap-6">
          <div className="lg:col-span-5">
            <p className="font-mono text-[12px] uppercase tracking-[0.22em] text-coral-ink">
              {h.eyebrow}
            </p>
            <h1 className="mt-5 text-[52px] font-bold leading-[1.02] tracking-[-0.02em] md:text-[64px]">
              {h.title}
            </h1>
            <p className="mt-3 text-[20px] font-semibold tracking-[-0.01em] text-ink md:text-[23px]">
              {h.tagline}
            </p>
            <p className="mt-5 max-w-[26rem] text-[17px] leading-[1.6] text-ink-2">{h.sub}</p>
            <div className="mt-9 flex flex-wrap items-center gap-3">
              <a
                href={RELEASE_READY ? DOWNLOAD_URL : "#download"}
                className="bg-coral px-6 py-3 text-[15px] font-semibold text-white transition-colors hover:bg-coral-deep"
              >
                {RELEASE_READY ? h.ctaRelease : h.ctaPending}
              </a>
              <a
                href={GITHUB_URL}
                target="_blank"
                rel="noreferrer"
                className="border border-line px-6 py-3 text-[15px] font-semibold text-ink transition-colors hover:border-ink"
              >
                {h.ctaGithub}
              </a>
            </div>
            <p className="mt-6 font-mono text-[12px] tracking-[0.06em] text-ink-3">
              {h.sceneCaption}
            </p>
          </div>
          <div className="relative h-[46vh] min-h-[320px] lg:col-span-7 lg:h-[70vh] lg:min-h-[480px]">
            <MonitorScene
              before={texture("tex-before")}
              after={texture("tex-after")}
              alt={h.sceneAlt}
              labelBefore={h.sceneBefore}
              labelAfter={h.sceneAfter}
            />
          </div>
        </div>
      </div>
      <div className="relative border-t border-line">
        <div className="mx-auto flex max-w-[1200px] flex-wrap items-center justify-between gap-x-8 gap-y-2 px-5 py-4 font-mono text-[11px] tracking-[0.14em] text-ink-3 md:px-8">
          {h.specs.map((s) => (
            <span key={s}>{s}</span>
          ))}
        </div>
      </div>
    </section>
  );
}
