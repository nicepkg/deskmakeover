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
import { FinishCards } from "./finish-cards";
import { Head } from "./head";
import { Playground } from "./playground/playground";
import { Receipts } from "./receipts";
import { CutScene } from "./scene-cut";
import { PromiseScene } from "./scene-promise";
import { ReadScene } from "./scene-read";
import { RescueScene } from "./scene-rescue";
import { StackScene } from "./stack-scene";

/** Two-column scene section: the story on one side, the live proof on the other. */
function SceneSection({
  id,
  head,
  scene,
  flip,
}: {
  id: string;
  head: EngineHead;
  scene: React.ReactNode;
  flip?: boolean;
}) {
  return (
    <section id={id} className="border-t border-line">
      <div className="mx-auto grid max-w-[1200px] items-center gap-12 px-5 py-20 md:px-8 md:py-28 lg:grid-cols-12">
        <Reveal className={`lg:col-span-6 ${flip ? "lg:order-2 lg:col-start-7" : ""}`}>
          <Head head={head} />
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
        {/* hero: the claim + the real icon exploded into its real layers */}
        <section className="relative overflow-hidden border-b border-line">
          <div className="grid-paper absolute inset-0" aria-hidden />
          <div className="relative mx-auto grid max-w-[1200px] items-center gap-10 px-5 pb-14 pt-16 md:px-8 md:pb-16 md:pt-20 lg:grid-cols-12">
            <div data-fx className="lg:col-span-6">
              <p className="font-mono text-[12px] tracking-[0.26em] text-coral-ink">{e.hero.eyebrow}</p>
              <h1 className="mt-5 max-w-[14ch] text-[38px] font-bold leading-[1.1] tracking-[-0.02em] md:text-[56px]">
                {e.hero.title}
              </h1>
              <p className="mt-6 max-w-[34rem] text-[16px] leading-[1.65] text-ink-2">{e.hero.sub}</p>
              <div className="mt-10 grid grid-cols-2 gap-px border border-line bg-line">
                {e.hero.stats.map((s) => (
                  <div key={s.label} className="bg-card px-4 py-3.5">
                    <p className="font-mono text-[20px] font-bold text-ink tabular-nums md:text-[23px]">
                      <span data-fx-count>{s.value.toLocaleString("en-US")}</span>
                      <span className="text-[13px] font-semibold text-ink-2">{s.unit}</span>
                    </p>
                    <p className="mt-0.5 text-[11.5px] leading-[1.5] text-ink-3">{s.label}</p>
                  </div>
                ))}
              </div>
            </div>
            <div className="lg:col-span-6">
              <StackScene engine={e} />
            </div>
          </div>
        </section>

        {/* 01 read */}
        <SceneSection id="read" head={e.read} scene={<ReadScene steps={e.read.steps} caption={e.read.caption} />} />

        {/* 02 cut */}
        <SceneSection id="cut" flip head={e.cut} scene={<CutScene engine={e} />} />

        {/* 03 rescue */}
        <SceneSection id="rescue" head={e.rescue} scene={<RescueScene engine={e} />} />

        {/* 04 promise */}
        <SceneSection id="promise" flip head={e.promise} scene={<PromiseScene engine={e} />} />

        {/* 05 finish */}
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

        {/* 06 playground finale */}
        <Playground engine={e} />

        {/* 07 receipts */}
        <section id="receipts" className="border-t border-line">
          <div className="mx-auto max-w-[1200px] px-5 py-20 md:px-8 md:py-24">
            <Reveal>
              <Head head={e.receipts} />
            </Reveal>
            <div data-fx className="mt-10">
              <Receipts lead="" receipts={e.receipts.receipts} />
            </div>
          </div>
        </section>

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
