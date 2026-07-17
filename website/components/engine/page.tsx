import type { EngineDict, EngineHead } from "@/content/engine-types";
import type { Dict } from "@/content/types";
import { DownloadCta } from "@/components/download/button";
import { DownloadModal } from "@/components/download/modal";
import { Reveal } from "@/components/reveal";
import { SiteFooter } from "@/components/site-footer";
import { SiteNav } from "@/components/site-nav";
import { StoryFx } from "@/components/story/fx";
import { engineJsonLdScript } from "@/lib/jsonld";
import { DOWNLOAD_URL, GITHUB_URL, RELEASE_READY } from "@/lib/site";
import { FinishCards, SquircleCorner, TonalRamp } from "./diagrams";
import { Head } from "./head";
import { Playground } from "./playground/playground";
import { Receipts } from "./receipts";
import { FloodScene } from "./scene-flood";
import { HueWheelScene } from "./scene-hue";
import { PortraitScene } from "./scene-portrait";
import { RescueScene } from "./scene-rescue";

/** Two-column scene section: text on one side, the living diagram on the other. */
function SceneSection({
  id,
  head,
  scene,
  extra,
  flip,
}: {
  id: string;
  head: EngineHead;
  scene: React.ReactNode;
  extra?: React.ReactNode;
  flip?: boolean;
}) {
  return (
    <section id={id} className="border-t border-line">
      <div className="mx-auto grid max-w-[1200px] items-center gap-12 px-5 py-20 md:px-8 md:py-28 lg:grid-cols-12">
        <Reveal className={`lg:col-span-6 ${flip ? "lg:order-2 lg:col-start-7" : ""}`}>
          <Head head={head} />
          {extra}
        </Reveal>
        <div data-fx className={`lg:col-span-5 ${flip ? "lg:order-1 lg:col-start-1" : "lg:col-start-8"}`}>
          {scene}
        </div>
      </div>
    </section>
  );
}

export function EnginePage({ dict, engine }: { dict: Dict; engine: EngineDict }) {
  const e = engine;
  const home = dict.locale === "zh" ? "/zh/" : "/";
  return (
    <>
      <SiteNav dict={dict} />
      <main>
        {/* hero */}
        <section className="relative overflow-hidden border-b border-line">
          <div className="grid-paper absolute inset-0" aria-hidden />
          <div data-fx className="relative mx-auto max-w-[1200px] px-5 pb-16 pt-20 md:px-8 md:pb-20 md:pt-28">
            <p className="font-mono text-[12px] tracking-[0.26em] text-coral-ink">{e.hero.eyebrow}</p>
            <h1 className="mt-5 max-w-[16ch] text-[40px] font-bold leading-[1.08] tracking-[-0.02em] md:text-[62px]">
              {e.hero.title}
            </h1>
            <p className="mt-6 max-w-[46rem] text-[16.5px] leading-[1.65] text-ink-2">{e.hero.sub}</p>
            <div className="mt-12 grid grid-cols-2 gap-px border border-line bg-line md:grid-cols-4">
              {e.hero.stats.map((s) => (
                <div key={s.label} className="bg-card px-4 py-4">
                  <p className="font-mono text-[22px] font-bold text-ink tabular-nums md:text-[26px]">
                    <span data-fx-count>{s.value.toLocaleString("en-US")}</span>
                    <span className="text-[14px] font-semibold text-ink-2">{s.unit}</span>
                  </p>
                  <p className="mt-1 text-[11.5px] leading-[1.5] text-ink-3">{s.label}</p>
                </div>
              ))}
            </div>
          </div>
        </section>

        {/* 01 portrait */}
        <SceneSection
          id="portrait"
          head={e.portrait}
          scene={
            <PortraitScene
              steps={e.portrait.steps}
              probeCaption={e.portrait.probeCaption}
              iouLabel={e.portrait.iouLabel}
            />
          }
        />

        {/* 02 separate */}
        <SceneSection
          id="separate"
          flip
          head={e.separate}
          extra={
            <div className="mt-8 space-y-4">
              {e.separate.stages.map((s, i) => (
                <div key={s.title} className="flex gap-3.5">
                  <span className="mt-0.5 font-mono text-[12px] text-coral-ink">{String(i + 1).padStart(2, "0")}</span>
                  <div>
                    <h3 className="text-[14.5px] font-semibold text-ink">{s.title}</h3>
                    <p className="mt-0.5 text-[13.5px] leading-[1.55] text-ink-2">{s.detail}</p>
                  </div>
                </div>
              ))}
            </div>
          }
          scene={<FloodScene caption={e.separate.floodCaption} replay={e.separate.replay} />}
        />

        {/* 03 rescue */}
        <SceneSection
          id="rescue"
          head={e.rescue}
          scene={<RescueScene beats={e.rescue.beats} gauges={e.rescue.gauges} caption={e.rescue.caption} />}
        />

        {/* 04 invariant */}
        <SceneSection
          id="invariant"
          flip
          head={e.invariant}
          scene={
            <HueWheelScene rule={e.invariant.rule} ruleNote={e.invariant.ruleNote} caption={e.invariant.wheelCaption} />
          }
        />

        {/* 05 color */}
        <section id="color" className="border-t border-line">
          <div className="mx-auto max-w-[1200px] px-5 py-20 md:px-8 md:py-28">
            <Reveal>
              <Head head={e.color} />
            </Reveal>
            <div data-fx className="mt-10 grid gap-10 lg:grid-cols-12">
              <div className="space-y-5 lg:col-span-5">
                {e.color.points.map((p, i) => (
                  <div key={p.title} data-fx-cell style={{ ["--fxd" as string]: `${i * 110}ms` }}>
                    <h3 className="text-[15px] font-semibold text-ink">{p.title}</h3>
                    <p className="mt-1 text-[13.5px] leading-[1.6] text-ink-2">{p.detail}</p>
                  </div>
                ))}
              </div>
              <div className="lg:col-span-6 lg:col-start-7">
                <TonalRamp />
                <div className="mt-6 border border-line bg-card p-5">
                  <SquircleCorner />
                </div>
              </div>
            </div>
          </div>
        </section>

        {/* 06 finish */}
        <section id="finish" className="border-t border-line">
          <div className="mx-auto max-w-[1200px] px-5 py-20 md:px-8 md:py-28">
            <Reveal>
              <Head head={e.finish} />
            </Reveal>
            <div data-fx className="mt-10">
              <FinishCards finishes={e.finish.finishes} />
            </div>
          </div>
        </section>

        {/* 07 guarantee + receipts */}
        <section id="guarantee" className="border-t border-line">
          <div className="mx-auto max-w-[1200px] px-5 py-20 md:px-8 md:py-28">
            <Reveal>
              <Head head={e.guarantee} />
            </Reveal>
            <div data-fx className="mt-10 grid gap-px border border-line bg-line sm:grid-cols-2 lg:grid-cols-4">
              {e.guarantee.items.map((it, i) => (
                <div key={it.title} data-fx-cell className="bg-card px-4 py-4" style={{ ["--fxd" as string]: `${i * 90}ms` }}>
                  <h3 className="text-[14.5px] font-semibold text-ink">{it.title}</h3>
                  <p className="mt-1 text-[12.5px] leading-[1.55] text-ink-2">{it.detail}</p>
                </div>
              ))}
            </div>
            <div data-fx className="mt-14">
              <Receipts lead={e.guarantee.receiptsLead} receipts={e.guarantee.receipts} />
            </div>
          </div>
        </section>

        {/* playground finale */}
        <Playground engine={e} />

        {/* closing CTA */}
        <section className="border-t border-line bg-panel">
          <div className="mx-auto max-w-[1200px] px-5 py-20 text-center md:px-8 md:py-24">
            <Reveal>
              <h2 className="mx-auto max-w-[22ch] text-[28px] font-bold leading-[1.14] tracking-[-0.015em] md:text-[36px]">
                {e.cta.title}
              </h2>
              <p className="mx-auto mt-4 max-w-[40rem] text-[15.5px] leading-[1.6] text-ink-2">{e.cta.body}</p>
              <div className="mt-8 flex flex-wrap items-center justify-center gap-3.5">
                {RELEASE_READY ? (
                  <DownloadCta
                    href={DOWNLOAD_URL}
                    className="inline-flex h-12 items-center bg-coral px-7 text-[15px] font-semibold text-white transition-colors hover:bg-coral-deep"
                  >
                    {e.cta.download}
                  </DownloadCta>
                ) : (
                  <a
                    href={`${home}#download`}
                    className="inline-flex h-12 items-center bg-coral px-7 text-[15px] font-semibold text-white transition-colors hover:bg-coral-deep"
                  >
                    {e.cta.download}
                  </a>
                )}
                <a
                  href={GITHUB_URL}
                  target="_blank"
                  rel="noreferrer"
                  className="inline-flex h-12 items-center border border-ink px-7 text-[15px] font-semibold text-ink transition-colors hover:bg-ink hover:text-canvas"
                >
                  {e.cta.github}
                </a>
              </div>
            </Reveal>
          </div>
        </section>
      </main>
      <SiteFooter dict={dict} />
      <DownloadModal dict={dict} />
      <StoryFx />
      <script
        type="application/ld+json"
        // eslint-disable-next-line react/no-danger
        dangerouslySetInnerHTML={{ __html: engineJsonLdScript(dict, e.meta) }}
      />
    </>
  );
}
