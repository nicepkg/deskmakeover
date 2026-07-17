import { Reveal } from "@/components/reveal";
import { Article } from "@/components/story/article";
import {
  AxisUnit,
  Card,
  ConcernBars,
  DailyArc,
  EmotionSpectrum,
  Heatmap,
  LengthBuckets,
  Rhythm,
  SectionHead,
  SignalChips,
  StatRow,
  ValenceHist,
} from "@/components/story/charts";
import { StoryFooter, StoryNav } from "@/components/story/chrome";
import { WordCloud } from "@/components/story/cloud";
import { DriftChart } from "@/components/story/drift";
import { StoryFx } from "@/components/story/fx";
import { Insights, Quotes } from "@/components/story/insights";
import { TideChart } from "@/components/story/tide";
import { ARC_CARD, CLOUD_LEGEND, DRIFT_AXIS, HEADS, HERO, LENGTH_CARD } from "@/content/story";
import { TONE } from "@/components/story/palette";

const LEGEND_TONE: Record<string, string> = { coral: TONE.coral, teal: TONE.teal, gold: TONE.gold };

function Section({
  id,
  children,
  first = false,
}: {
  id: string;
  children: React.ReactNode;
  first?: boolean;
}) {
  return (
    <section id={id} className={first ? "" : "border-t border-line"}>
      <div className="mx-auto max-w-[1100px] px-5 py-14 md:px-8 md:py-20">{children}</div>
    </section>
  );
}

export default function StoryPage() {
  return (
    <>
      <StoryNav />
      <main>
        {/* hero */}
        <div className="relative">
          <div aria-hidden className="grid-paper absolute inset-0" />
          <div className="relative mx-auto max-w-[1100px] px-5 pb-12 pt-16 md:px-8 md:pb-16 md:pt-24">
            <Reveal>
              <p className="flex items-center gap-3 font-mono text-[11.5px] tracking-[0.22em] text-ink-3">
                <span className="inline-block h-2 w-2 bg-coral" />
                {HERO.eyebrow}
              </p>
              <h1 className="mt-5 max-w-[16ch] text-[clamp(34px,7.2vw,68px)] font-bold leading-[1.05] tracking-[-0.02em]">
                {HERO.titlePre}
                <span className="text-coral-deep">{HERO.titleAccent}</span>
              </h1>
              <p className="mt-4 max-w-[62ch] text-[clamp(15px,2.2vw,18.5px)] leading-[1.65] text-ink-2">
                2026 年 7 月 8 日到 17 日，你在一次会话里对 AI 说了{" "}
                <b className="font-semibold text-ink">341 句话</b>
                ，把一个「别人写得丑到想骂人」的 Windows
                桌面美化应用，从头逼到能发版。这是从你每一句话里读出来的九天。
              </p>
              <StatRow />
            </Reveal>
          </div>
        </div>

        <Section id="cloud">
          <Reveal>
            <SectionHead head={HEADS.cloud} />
          </Reveal>
          <Reveal delay={80}>
            <Card className="mt-8 overflow-hidden">
              <WordCloud />
              <div className="flex flex-wrap gap-x-[18px] gap-y-2 border-t border-line px-5 py-3.5 md:px-7">
                {CLOUD_LEGEND.map((l) => (
                  <span
                    key={l.label}
                    className="inline-flex items-center gap-[7px] font-mono text-[12.5px] text-ink-3"
                  >
                    <i className="h-2.5 w-2.5" style={{ background: LEGEND_TONE[l.tone] }} />
                    {l.label}
                  </span>
                ))}
              </div>
            </Card>
          </Reveal>
        </Section>

        <Section id="concerns">
          <Reveal>
            <SectionHead head={HEADS.concerns} />
          </Reveal>
          <Reveal delay={80}>
            <Card className="mt-8 p-5 md:p-7">
              <ConcernBars />
            </Card>
          </Reveal>
        </Section>

        <Section id="sentiment">
          <Reveal>
            <SectionHead head={HEADS.sentiment} />
          </Reveal>
          <Reveal delay={80}>
            <Card className="mt-8 p-5 md:p-7">
              <EmotionSpectrum />
            </Card>
          </Reveal>
        </Section>

        <Section id="tide">
          <Reveal>
            <SectionHead head={HEADS.tide} />
          </Reveal>
          <Reveal delay={80}>
            <Card className="mt-8 p-5 md:p-7">
              <TideChart />
              <AxisUnit>
                <b className="font-semibold text-ink-2">横轴</b>
                {"＝341 条发言按时间先后（竖虚线为每日分界）　·　"}
                <b className="font-semibold text-ink-2">纵轴</b>
                {"＝情绪强度（+3 非常满意 → 0 中性 → −3 暴怒）　·　标注为关键拐点。"}
              </AxisUnit>
              <ValenceHist />
            </Card>
          </Reveal>
        </Section>

        <Section id="drift">
          <Reveal>
            <SectionHead head={HEADS.drift} />
          </Reveal>
          <Reveal delay={80}>
            <Card className="mt-8 p-5 md:p-7">
              <div className="mb-3 flex flex-wrap justify-between gap-3 font-mono text-[10.5px] tracking-[0.08em] text-ink-3">
                <span>{DRIFT_AXIS.y}</span>
                <span>{DRIFT_AXIS.x}</span>
              </div>
              <div data-fx>
                <div data-fx-wipe>
                  <DriftChart />
                </div>
              </div>
              <div className="mt-4 flex flex-wrap gap-x-5 gap-y-2 font-mono text-[12.5px] text-ink-3">
                {DRIFT_AXIS.legend.map((l) => (
                  <span key={l.label} className="inline-flex items-center gap-[7px]">
                    <i className="h-[11px] w-[11px]" style={{ background: LEGEND_TONE[l.tone] }} />
                    {l.label}
                  </span>
                ))}
              </div>
            </Card>
          </Reveal>
        </Section>

        <Section id="heat">
          <Reveal>
            <SectionHead head={HEADS.heat} />
          </Reveal>
          <Reveal delay={80}>
            <Card className="mt-8 p-5 md:p-7">
              <Heatmap />
            </Card>
          </Reveal>
        </Section>

        <Section id="rhythm">
          <Reveal>
            <SectionHead head={HEADS.rhythm} />
          </Reveal>
          <Reveal delay={80}>
            <Card className="mt-8 p-5 md:p-7">
              <Rhythm />
            </Card>
          </Reveal>
        </Section>

        <Section id="arc">
          <div className="grid gap-4 lg:grid-cols-2">
            <Reveal>
              <Card className="h-full p-5 md:p-7">
                <p className="font-mono text-[12px] tracking-[0.18em] text-ink-3">
                  <span className="text-coral-ink">{ARC_CARD.index}</span>
                  {"  ·  "}
                  {ARC_CARD.kicker}
                </p>
                <h3 className="mt-3 text-[20px] font-bold tracking-[-0.01em]">{ARC_CARD.title}</h3>
                <p className="mb-4 mt-2 text-[13.5px] leading-[1.6] text-ink-3">
                  每行左起：<b className="font-medium text-ink-2">日期</b>、当日
                  <b className="font-medium text-ink-2">发言条数</b>
                  、当日主线（条形按峰值 70 条归一）。
                </p>
                <DailyArc />
              </Card>
            </Reveal>
            <Reveal delay={80}>
              <Card className="h-full p-5 md:p-7">
                <p className="font-mono text-[12px] tracking-[0.18em] text-ink-3">{LENGTH_CARD.kicker}</p>
                <h3 className="mt-3 text-[20px] font-bold tracking-[-0.01em]">{LENGTH_CARD.title}</h3>
                <p className="mb-5 mt-2 text-[13.5px] leading-[1.6] text-ink-3">{LENGTH_CARD.hint}</p>
                <LengthBuckets />
              </Card>
            </Reveal>
          </div>
        </Section>

        <Section id="signals">
          <Reveal>
            <SectionHead head={HEADS.signals} />
          </Reveal>
          <Reveal delay={80}>
            <div className="mt-8">
              <SignalChips />
            </div>
          </Reveal>
        </Section>

        <Section id="insights">
          <Reveal>
            <SectionHead head={HEADS.insights} />
          </Reveal>
          <Reveal delay={80}>
            <div className="mt-8">
              <Insights />
            </div>
          </Reveal>
        </Section>

        <Section id="quotes">
          <Reveal>
            <SectionHead head={HEADS.quotes} />
          </Reveal>
          <Reveal delay={80}>
            <div className="mt-8">
              <Quotes />
            </div>
          </Reveal>
        </Section>

        <Section id="article">
          <Reveal>
            <SectionHead head={HEADS.article} />
          </Reveal>
          <Reveal delay={80}>
            <div className="mt-8">
              <Article />
            </div>
          </Reveal>
        </Section>
      </main>
      <StoryFooter />
      <StoryFx />
    </>
  );
}
