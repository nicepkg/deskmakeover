import type { CSSProperties } from "react";
import {
  CAT_DESIGN,
  CAT_ENG,
  DAYS,
  EMOTION,
  HEAT,
  HEAT_DAYS,
  HOURS,
  LEN_BUCKETS,
  SIGNALS,
  VDIST,
  WORDCLOUD,
} from "@/content/story-data";
import {
  CONCERNS_AXIS,
  DAY_NOTES,
  HEAT_AXIS,
  HERO,
  LEN_LABELS,
  RHYTHM_AXIS,
  SIGNAL_META,
  TIDE_AXIS,
  VDIST_LABELS,
  type StoryHead,
} from "@/content/story";
import { EMOTION_COLORS, TONE, mixTone } from "./palette";

/*
 * Server-rendered chart markup for /story/. Every element carries its FINAL
 * value inline (charts are complete without JS); the data-fx-* attributes let
 * components/story/fx.tsx arm and replay them as entrance animations.
 */

const delayStyle = (ms: number, extra?: CSSProperties): CSSProperties =>
  ({ "--fxd": `${ms}ms`, ...extra }) as CSSProperties;

const catColor = (w: string) => (CAT_DESIGN.has(w) ? TONE.coral : CAT_ENG.has(w) ? TONE.teal : TONE.gold);

export function SectionHead({ head }: { head: StoryHead }) {
  return (
    <div className="flex flex-wrap items-end justify-between gap-x-10 gap-y-4">
      <div>
        <p className="font-mono text-[12px] tracking-[0.18em] text-ink-3">
          <span className="text-coral-ink">{head.index}</span>
          {"  ·  "}
          {head.kicker}
        </p>
        <h2 className="mt-3 max-w-[26ch] text-[26px] font-bold leading-[1.18] tracking-[-0.01em] md:text-[32px]">
          {head.title}
        </h2>
      </div>
      <p className="max-w-[44ch] text-[13px] leading-[1.65] text-ink-3 md:text-right">{head.hint}</p>
    </div>
  );
}

export function Card({ children, className = "" }: { children: React.ReactNode; className?: string }) {
  return <div className={`border border-line bg-card ${className}`}>{children}</div>;
}

export function AxisUnit({ children, className = "" }: { children: React.ReactNode; className?: string }) {
  return (
    <p className={`mt-4 font-mono text-[11px] leading-[1.65] text-ink-3 ${className}`}>{children}</p>
  );
}

/* ---------- hero stat strip ---------- */
export function StatRow() {
  return (
    <div data-fx className="mt-10 grid grid-cols-2 gap-px border border-line bg-line md:grid-cols-5">
      {HERO.stats.map((s, i) => (
        <div
          key={s.label}
          className={`bg-canvas px-4 pb-4 pt-[18px] ${i === HERO.stats.length - 1 ? "col-span-2 md:col-span-1" : ""}`}
        >
          <div className="font-mono text-[clamp(24px,4.4vw,38px)] font-semibold leading-none tracking-[-0.01em] text-ink tabular-nums">
            <span data-fx-count>{s.value.toFixed(s.decimals)}</span>
            {s.suffix ? (
              <span className="ml-0.5 text-[0.5em] font-semibold text-ink-3">{s.suffix}</span>
            ) : null}
          </div>
          <p className="mt-2.5 text-[12px] leading-[1.5] text-ink-2">{s.label}</p>
        </div>
      ))}
    </div>
  );
}

/* ---------- 02 · top concerns ---------- */
export function ConcernBars() {
  const top = WORDCLOUD.slice(0, 16);
  const max = top[0][1];
  return (
    <div data-fx>
      <div className="grid grid-cols-[96px_1fr_38px] gap-2.5 border-b border-line pb-2.5 font-mono text-[10.5px] tracking-[0.09em] text-ink-3 sm:grid-cols-[118px_1fr_46px] sm:gap-3">
        <span className="text-right">{CONCERNS_AXIS.colTerm}</span>
        <span />
        <span className="text-right">{CONCERNS_AXIS.colCount}</span>
      </div>
      <div className="mt-2.5 flex flex-col gap-[11px]">
        {top.map(([w, n]) => (
          <div
            key={w}
            className="grid grid-cols-[96px_1fr_38px] items-center gap-2.5 sm:grid-cols-[118px_1fr_46px] sm:gap-3"
          >
            <div className="overflow-hidden text-ellipsis whitespace-nowrap text-right text-[12px] text-ink sm:text-[13px]">
              {w}
            </div>
            <div className="relative h-[15px] overflow-hidden bg-panel">
              <div
                data-fx-w={((n / max) * 100).toFixed(1)}
                className="h-full"
                style={{ width: `${((n / max) * 100).toFixed(1)}%`, background: catColor(w) }}
              />
            </div>
            <div className="text-right font-mono text-[12.5px] text-ink-3 tabular-nums">
              <span data-fx-count>{n}</span>
            </div>
          </div>
        ))}
      </div>
      <AxisUnit>
        <b className="font-semibold text-ink-2">横轴</b>
        ＝该主题词在全部 341 条发言里出现的<b className="font-semibold text-ink-2">总次数</b>
        （条形长度按最高频 328 归一）。
      </AxisUnit>
    </div>
  );
}

/* ---------- 03 · sentiment ---------- */
export function EmotionSpectrum() {
  const total = EMOTION.reduce((s, [, n]) => s + n, 0);
  return (
    <div data-fx>
      <div className="flex h-11 overflow-hidden border border-line">
        {EMOTION.map(([k, n]) => (
          <div
            key={k}
            data-fx-grow={n}
            title={`${k} ${n}`}
            className="min-w-[2px]"
            style={{ flexGrow: n, background: EMOTION_COLORS[k] }}
          />
        ))}
      </div>
      <div className="mt-5 grid grid-cols-1 gap-x-6 gap-y-2.5 sm:grid-cols-2">
        {EMOTION.map(([k, n]) => (
          <div key={k} className="flex items-baseline gap-2.5">
            <i
              className="h-[11px] w-[11px] flex-none translate-y-px"
              style={{ background: EMOTION_COLORS[k] }}
            />
            <span className="text-[13.5px] text-ink">{k}</span>
            <span className="ml-auto font-mono text-[13px] text-ink-3 tabular-nums">
              <span data-fx-count>{n}</span> · <span data-fx-count>{((n / total) * 100).toFixed(0)}</span>%
            </span>
          </div>
        ))}
      </div>
      <div className="mt-5 border border-line bg-panel px-[18px] py-4 text-[14px] leading-[1.7] text-ink-2">
        含强烈批评（<b className="font-semibold text-ink">狗屎 / 垃圾 / 无语 / 服了</b>）的发言{" "}
        <span data-fx-count className="font-mono font-semibold text-coral-ink tabular-nums">31</span> 条，含真诚肯定（
        <b className="font-semibold text-ink">不错 / 满意 / 终于弄对</b>）的{" "}
        <span data-fx-count className="font-mono font-semibold text-coral-ink tabular-nums">43</span>{" "}
        条。你嘴上凶，但认得出好东西，也不吝啬夸奖。
      </div>
    </div>
  );
}

/* ---------- 04 · valence histogram (under the tide) ---------- */
export function ValenceHist() {
  const order = ["-3", "-2", "-1", "0", "1", "2", "3"];
  const cols: Record<string, string> = {
    "-3": TONE.coral,
    "-2": TONE.coral,
    "-1": mixTone(TONE.coral, 55),
    "0": TONE.ink3,
    "1": mixTone(TONE.gold, 55),
    "2": TONE.gold,
    "3": TONE.gold,
  };
  const vals = order.map((k) => VDIST[k] ?? 0);
  const vmax = Math.max(...vals);
  return (
    <div data-fx>
      <div className="mt-6 grid grid-cols-7 gap-1.5">
        {order.map((k, i) => (
          <div key={k} className="flex flex-col items-center gap-1.5">
            <div className="flex h-[66px] w-full items-end">
              <div
                data-fx-h={((vals[i] / vmax) * 100).toFixed(1)}
                className="w-full"
                style={{ height: `${((vals[i] / vmax) * 100).toFixed(1)}%`, background: cols[k] }}
              />
            </div>
            <div className="font-mono text-[12px] text-ink-2 tabular-nums">
              <span data-fx-count>{vals[i]}</span>
            </div>
            <div className="text-center text-[10.5px] leading-[1.25] text-ink-3">
              {Number(k) > 0 ? `+${k}` : k}
              <br />
              {VDIST_LABELS[k]}
            </div>
          </div>
        ))}
      </div>
      <AxisUnit className="text-center">{TIDE_AXIS.histCaption}</AxisUnit>
    </div>
  );
}

/* ---------- 06 · activity heatmap ---------- */
export function Heatmap() {
  let hmax = 0;
  for (const k in HEAT) hmax = Math.max(hmax, HEAT[k]);
  const cellColor = (v: number) => {
    if (!v) return "var(--color-panel)";
    const a = 0.16 + 0.84 * Math.sqrt(v / hmax);
    return mixTone(TONE.coral, a * 100);
  };
  return (
    <div data-fx>
      <div className="mb-3 flex flex-wrap justify-between gap-3 font-mono text-[10.5px] tracking-[0.08em] text-ink-3">
        <span>{HEAT_AXIS.y}</span>
        <span>{HEAT_AXIS.x}</span>
      </div>
      <div className="overflow-x-auto">
        <div className="grid min-w-[640px] gap-[3px]">
          {HEAT_DAYS.map((day, di) => (
            <div key={day} className="grid grid-cols-[56px_repeat(24,1fr)] items-center gap-[3px]">
              <div className="pr-1 text-right font-mono text-[11px] text-ink-3">{day}</div>
              {Array.from({ length: 24 }, (_, h) => {
                const v = HEAT[`${day}|${h}`] ?? 0;
                return (
                  <div
                    key={h}
                    data-fx-cell
                    title={`${day} ${String(h).padStart(2, "0")}:00 · ${v}`}
                    className="aspect-square"
                    style={delayStyle(di * 26 + h * 11, { background: cellColor(v) })}
                  />
                );
              })}
            </div>
          ))}
          <div className="grid grid-cols-[56px_repeat(24,1fr)] gap-[3px] pt-1">
            <span />
            {Array.from({ length: 24 }, (_, h) => (
              <span key={h} className="text-center font-mono text-[9.5px] text-ink-3">
                {h % 3 === 0 ? String(h).padStart(2, "0") : ""}
              </span>
            ))}
          </div>
        </div>
      </div>
      <AxisUnit className="mt-2 text-center">{HEAT_AXIS.caption}</AxisUnit>
      <div className="mt-3 flex items-center justify-end gap-[7px] font-mono text-[11px] text-ink-3">
        <span>{HEAT_AXIS.keyLow}</span>
        {[0, 0.25, 0.5, 0.75, 1].map((t) => (
          <i
            key={t}
            className="h-[13px] w-[13px]"
            style={{ background: t === 0 ? "var(--color-panel)" : mixTone(TONE.coral, (0.16 + 0.84 * Math.sqrt(t)) * 100) }}
          />
        ))}
        <span>{HEAT_AXIS.keyHigh}</span>
      </div>
    </div>
  );
}

/* ---------- 07 · circadian rhythm ---------- */
export function Rhythm() {
  const max = Math.max(...HOURS);
  return (
    <div data-fx>
      <div className="flex items-stretch gap-2.5">
        <div className="flex flex-none items-center justify-center font-mono text-[10px] tracking-[0.14em] text-ink-3 [text-orientation:upright] [writing-mode:vertical-rl]">
          {RHYTHM_AXIS.ylab}
        </div>
        <div className="flex min-w-[20px] flex-none flex-col items-end justify-between pb-5 font-mono text-[10px] text-ink-3 tabular-nums">
          <span>34</span>
          <span>17</span>
          <span>0</span>
        </div>
        <div className="min-w-0 flex-1">
          <div className="grid h-[150px] grid-cols-24 items-end gap-1">
            {HOURS.map((n, h) => (
              <div
                key={h}
                data-fx-h={((n / max) * 100).toFixed(1)}
                title={`${String(h).padStart(2, "0")}:00 · ${n}`}
                className="min-h-[2px]"
                style={{
                  height: `${((n / max) * 100).toFixed(1)}%`,
                  background: h < 6 ? TONE.slate : TONE.coral,
                  opacity: h < 6 ? 0.6 : 0.92,
                }}
              />
            ))}
          </div>
          <div className="mt-2 grid grid-cols-24 gap-1">
            {HOURS.map((_, h) => (
              <span key={h} className="text-center font-mono text-[9px] text-ink-3">
                {h % 3 === 0 ? String(h).padStart(2, "0") : ""}
              </span>
            ))}
          </div>
        </div>
      </div>
      <AxisUnit>
        <b className="font-semibold text-ink-2">横轴</b>
        {"＝一天 24 小时（0 → 23 时）　·　"}
        <b className="font-semibold text-ink-2">纵轴</b>
        {"＝发言条数（峰值 34 条，晚 19 时）　·　"}
        <span style={{ color: TONE.slate }}>蓝色柱</span>
        {"＝深夜 0 → 6 点。"}
      </AxisUnit>
    </div>
  );
}

/* ---------- 08a · daily arc ---------- */
export function DailyArc() {
  const max = Math.max(...DAYS.map((d) => d[1]));
  return (
    <div data-fx className="flex flex-col">
      {DAYS.map(([d, n], i) => (
        <div
          key={d}
          className={`grid grid-cols-[64px_42px_1fr] items-center gap-3.5 py-2.5 ${i > 0 ? "border-t border-line" : ""}`}
        >
          <div className="font-mono text-[12.5px] text-ink-3">{d}</div>
          <div className="text-right font-mono text-[13px] font-semibold text-coral-ink tabular-nums">
            <span data-fx-count>{n}</span>
          </div>
          <div>
            <div className="h-[9px] bg-panel">
              <div
                data-fx-w={((n / max) * 100).toFixed(1)}
                className="h-full bg-coral-deep"
                style={{ width: `${((n / max) * 100).toFixed(1)}%` }}
              />
            </div>
            <p className="mt-[5px] text-[12.5px] leading-[1.5] text-ink-2">{DAY_NOTES[d] ?? ""}</p>
          </div>
        </div>
      ))}
    </div>
  );
}

/* ---------- 08b · message length ---------- */
export function LengthBuckets() {
  const total = LEN_BUCKETS.reduce((a, b) => a + b, 0);
  const max = Math.max(...LEN_BUCKETS);
  return (
    <div data-fx>
      <div className="flex flex-col gap-3">
        {LEN_BUCKETS.map((n, i) => (
          <div key={LEN_LABELS[i]}>
            <div className="flex items-center justify-between gap-2.5">
              <span className="text-[13.5px] text-ink">{LEN_LABELS[i]}</span>
              <span className="font-mono text-[12.5px] text-ink-3 tabular-nums">
                <span data-fx-count>{n}</span> · <span data-fx-count>{((n / total) * 100).toFixed(0)}</span>%
              </span>
            </div>
            <div className="mt-1.5 h-3 bg-panel">
              <div
                data-fx-w={((n / max) * 100).toFixed(1)}
                className="h-full"
                style={{ width: `${((n / max) * 100).toFixed(1)}%`, background: TONE.gold }}
              />
            </div>
          </div>
        ))}
      </div>
      <div className="mt-5 border border-line bg-panel px-[18px] py-4 text-[14px] leading-[1.7] text-ink-2">
        最短的一类是 <b className="font-semibold text-ink">「继续」「开工」「commit」</b>，共{" "}
        <span data-fx-count className="font-mono font-semibold text-coral-ink tabular-nums">37</span>{" "}
        条。这是你独特的高信任低带宽驱动：睡前一句话，放权 AI 连续作业几小时。
      </div>
    </div>
  );
}

/* ---------- 09 · behavioral signal chips ---------- */
export function SignalChips() {
  return (
    <div data-fx className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-5">
      {SIGNAL_META.map(([key, label]) => (
        <div key={key} className="border border-line bg-panel px-4 py-[15px]">
          <div className="font-mono text-[27px] font-semibold leading-none text-ink tabular-nums">
            <span data-fx-count>{SIGNALS[key]}</span>
            <span className="text-[0.5em] text-ink-3"> 条</span>
          </div>
          <p className="mt-2 text-[12.5px] leading-[1.5] text-ink-2">{label}</p>
        </div>
      ))}
    </div>
  );
}
