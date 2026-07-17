import {
  INSIGHT_HEADS,
  INSIGHT_SURPRISE,
  QUOTES,
  QUOTE_TONE_NAME,
} from "@/content/story";
import { QUOTE_TONE_COLORS, mixTone } from "./palette";

/*
 * Section 10 (子策的解读) and section 11 (原声语录). All prose is VERBATIM
 * from the source report — bold runs and emphasis included.
 */

const B = ({ children }: { children: React.ReactNode }) => (
  <b className="font-semibold text-ink">{children}</b>
);

const Unit = ({ children }: { children: React.ReactNode }) => (
  <span className="ml-1 text-[0.42em] font-semibold tracking-normal text-ink-3">{children}</span>
);

function InsightCard({
  n,
  fig,
  wide,
  surprise,
  children,
}: {
  n: number;
  fig: React.ReactNode;
  wide?: boolean;
  surprise?: boolean;
  children: React.ReactNode;
}) {
  const h = INSIGHT_HEADS[n];
  return (
    <div className={`border border-line bg-card px-6 py-[22px] ${wide ? "md:col-span-2" : ""}`}>
      {surprise ? (
        <p className="mb-3 inline-block border border-gold/50 px-2.5 py-0.5 font-mono text-[10px] tracking-[0.1em] text-gold">
          {INSIGHT_SURPRISE}
        </p>
      ) : null}
      <p className="font-mono text-[10.5px] tracking-[0.14em] text-coral-ink">{h.tag}</p>
      <div
        data-fx
        className="mt-2.5 font-mono text-[clamp(30px,5vw,44px)] font-bold leading-none tracking-[-0.02em] text-ink tabular-nums"
      >
        {fig}
      </div>
      <h3 className="mt-3 text-[18.5px] font-bold leading-[1.3] tracking-[-0.01em]">{h.title}</h3>
      <p className="mt-2 text-[14px] leading-[1.72] text-ink-2">{children}</p>
    </div>
  );
}

export function Insights() {
  return (
    <div className="grid gap-4 md:grid-cols-2">
      <InsightCard
        n={0}
        fig={
          <>
            <span data-fx-count>81</span>
            <Unit>%</Unit>
          </>
        }
      >
        94 条负向发言里，<B>76 条（81%）</B>
        在同一句里就给出了具体修法：改哪、换成什么、对齐到哪。「狗屎」不是情绪垃圾，是一种压缩：它同时说了「这里错了」和「我知道对的样子」。真正的纯发泄极少。
      </InsightCard>

      <InsightCard
        n={1}
        fig={
          <>
            <span data-fx-count>0</span>
            {" → "}
            <span data-fx-count>38</span>
            <Unit>% 放权</Unit>
          </>
        }
      >
        第一天你零放权、平均每条 138 字、逐个像素纠错；到中段，一句话就走的短指令占到当天{" "}
        <B>38%</B>。但这不是线性的：07-14 液态玻璃翻车那天，放权率<B>跌回 0</B>
        ，你重新亲自接管。<B>你只在「门禁全绿 + Codex 兜底」时才敢松手，出事立刻收回。</B>
      </InsightCard>

      <InsightCard
        n={2}
        wide
        surprise
        fig={
          <>
            {"夜 +"}
            <span data-fx-count>0.05</span>
            {"　·　昼 −"}
            <span data-fx-count>0.32</span>
          </>
        }
      >
        我本以为熬夜会让你更暴躁。数据相反：<B>凌晨 0 到 6 点</B>（103 条发言）情绪均值是
        <B>微正的 +0.05</B>，且平均每条 <B>241 字</B>；<B>白天</B>反而更负（−0.32），每条只有 166
        字。读法是：深夜你在心流里写长规格、做规划、派活，语气是平稳的；白天你一睁眼看到 AI
        夜里交的活，短促而锋利的批评就来了。<B>你的暴躁不属于深夜，属于「验收的那一刻」。</B>
      </InsightCard>

      <InsightCard
        n={3}
        fig={
          <>
            <span data-fx-count>3</span>
            {" / "}
            <span data-fx-count>3</span>
          </>
        }
      >
        WASM 内核（07-11）、液态玻璃（07-14「终于弄对了」）、3D 官网（07-17「完美试卷」），三次高峰无一例外，都发生在你下了
        <B>「推倒重来 / 完全重写」</B>的死命令之后。
        <B>渐进打补丁几乎从没真正让你满意，大刀阔斧才行。</B>这既是你的标准，也是成本。
      </InsightCard>

      <InsightCard
        n={4}
        fig={
          <>
            <span data-fx-count>11</span>
            <Unit>次授权</Unit>
          </>
        }
      >
        你 11 次主动喊 AI「大胆反驳我」，但当反驳触到你的<B>核心判断</B>（沿用上次 apply 的
        style、拦截黑过你的用户）时，你会据理把它压回去。
        <B>你要的不是被说服，是让自己的观点先扛一轮攻击</B>
        ；扛住了才用。这比「言听计从」更难伺候，也更值钱。
      </InsightCard>
    </div>
  );
}

export function Quotes() {
  return (
    <div className="columns-1 gap-4 md:columns-2">
      {QUOTES.map((q) => (
        <figure
          key={q.text}
          className="mb-4 break-inside-avoid border border-line bg-card px-5 py-[18px]"
        >
          <div className="text-[15.5px] leading-[1.62] tracking-[0.005em] text-ink">
            「{q.text}」
          </div>
          <figcaption className="mt-3 flex items-center gap-2.5 font-mono text-[11.5px] text-ink-3">
            <span
              className="px-2 py-[3px] font-mono text-[10.5px] tracking-[0.08em]"
              style={{
                color: QUOTE_TONE_COLORS[q.tone],
                background: mixTone(QUOTE_TONE_COLORS[q.tone], 12),
              }}
            >
              {QUOTE_TONE_NAME[q.tone]}
            </span>
            <span>
              {q.date} · {q.label}
            </span>
          </figcaption>
        </figure>
      ))}
    </div>
  );
}
