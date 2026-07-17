import { ARTICLE } from "@/content/story";
import { TONE, mixTone } from "./palette";

/*
 * Section 12 — the long-form build story + the independent verdict. Every
 * sentence is VERBATIM from the source report.
 */

const B = ({ children }: { children: React.ReactNode }) => (
  <b className="font-semibold text-ink">{children}</b>
);

/** Coral highlight run (the source report's <em>). */
const Em = ({ children }: { children: React.ReactNode }) => (
  <em
    className="not-italic px-1"
    style={{ background: mixTone(TONE.coral, 12), color: TONE.coralInk }}
  >
    {children}
  </em>
);

function H3({ no, children }: { no: string; children: React.ReactNode }) {
  return (
    <h3 className="mb-1.5 mt-10 text-[clamp(18px,2.6vw,23px)] font-bold leading-[1.3] tracking-[-0.01em]">
      <span className="mr-2.5 font-mono text-[0.82em] font-bold text-coral-ink">{no}</span>
      {children}
    </h3>
  );
}

const pCls = "mb-[18px] text-[16.5px] leading-[1.82] text-ink";
const liCls = "mb-2.5 text-[16px] leading-[1.7] text-ink";

export function Article() {
  return (
    <article className="pt-2">
      <div className="max-w-[70ch]">
        <p className="font-mono text-[11.5px] tracking-[0.2em] text-coral-ink">{ARTICLE.kicker}</p>
        <h2 className="mb-2 mt-4 text-[clamp(26px,5vw,44px)] font-bold leading-[1.14] tracking-[-0.02em]">
          {ARTICLE.title}
        </h2>
        <p className="mb-2 text-[clamp(16.5px,2.4vw,20px)] leading-[1.6] text-ink-2">
          工具是当下最强的：Claude Code（Fable 5 + Opus
          4.8）负责写，Codex（GPT-5.6 sol
          ultra）负责审。可就算调了最顶级的模型，真正决定成品的，从来不是模型有多聪明，而是你怎么指挥它、怎么给它兜底、以及你自己有没有品味替它把关。
        </p>

        <H3 no="01">{ARTICLE.headings[0]}</H3>
        <p className={pCls}>
          DeskMakeover 是个 Windows
          桌面美化工具，把杂乱的图标批量套上统一形状、角标和配色，再给壁纸划分区。最初的版本是我在
          Windows 上让另一个 AI
          写的，界面丑到我一看就来气：文字溢出、高度乱用、卡片奇形怪状。我做过一期视频聊桌面审美，评论区一堆人骂我。那口气，我是带着造这个软件的。
        </p>

        <H3 no="02">{ARTICLE.headings[1]}</H3>
        <p className={pCls}>
          我没让一个 AI 从头包到尾。<B>Claude 负责生成和迭代，Codex 负责对抗式审查</B>：Claude
          每写完一批，就派 Codex 去毛孔式、地毯式地挑毛病。关键是我很清楚——
          <Em>Codex 挑出来的不一定对</Em>，有些是设计本来如此。所以规矩是：Codex
          报的问题，先核实是不是真问题，是才修。两个顶级模型互为镜子，比任何单一模型自评都可靠。
        </p>

        <H3 no="03">{ARTICLE.headings[2]}</H3>
        <ul className="mb-[18px] list-disc pl-[1.1em]">
          <li className={liCls}>
            <B>甩截图 + 精确诊断，不空谈。</B>九天我发了 63
            张验收截图。不说「这里不好看」，说「icon 和数字不在同一 baseline，数字偏下」——把 AI
            当成能看图的下属，越具体它越不跑偏。
          </li>
          <li className={liCls}>
            <B>逼它「大胆反驳我」。</B>我 11 次明确授权 AI 反驳我的方案。顺从的 AI
            是废的；我要的是我的想法先被压力测试一轮，扛得住才用。
          </li>
          <li className={liCls}>
            <B>开专家团。</B>承重的设计决策，我让它拆成「首席 PM / UI / UX」几个 subagent
            各自发言、互相反驳，再给我一张裁决表。
          </li>
          <li className={liCls}>
            <B>文档治理防回归。</B>全程按{" "}
            <code className="bg-panel px-1 font-mono text-[0.92em]">/dev-cycle</code> 把每个决策沉淀进
            spec / ADR / STATE，专门防止换一个会话就把之前定的东西又推翻。
          </li>
          <li className={liCls}>
            <B>睡前放权，醒来验收。</B>
            我常留一句「做完再停，无人值守，有拿不准的攒着等我」，然后去睡。醒来第一件事就是挑刺。
          </li>
        </ul>

        <blockquote className="my-8 text-[clamp(19px,3vw,26px)] font-bold leading-[1.4] tracking-[-0.01em] text-ink">
          <span className="mb-4 block h-[3px] w-[38px] bg-coral-deep" />
          {ARTICLE.pullquote}
        </blockquote>

        <H3 no="04">{ARTICLE.headings[3]}</H3>
        <ul className="mb-[18px] list-disc pl-[1.1em]">
          <li className={liCls}>
            <B>坑：它爱说「搞定了」，其实没跑过。</B>→ 我逼它每轮<B>诚实盘点</B>
            「还有哪些没做、哪些只在 Mac 验过、哪些纯盲写」，再加 Codex
            审查 + 全绿门禁（几百个测试）双兜底。
          </li>
          <li className={liCls}>
            <B>坑：它讨好、顺着我说。</B>→ 制度化的「大胆反驳」，把顺从从流程里删掉。
          </li>
          <li className={liCls}>
            <B>坑：一个模型钻牛角尖出不来。</B>液态玻璃效果，Claude
            改了几版都像路边仿制品。→ 我直接把它 Q 掉，甩一个 GitHub 参考实现，命令 Codex{" "}
            <Em>完全复刻</Em>，所有折射都要有。换模型 + 给参照，比硬磕有效。
          </li>
          <li className={liCls}>
            <B>坑：渐进补丁越修越烂。</B>→
            到某个点我不修了，直接下「推倒重来」。我三次最满意的成品，全是重写出来的，不是补出来的。
          </li>
          <li className={liCls}>
            <B>坑：拿假素材、mock 糊弄我。</B>它自己手绘图标、用假渐变冒充壁纸。→ 我坚持
            <B>真实素材 + 真机 / 真浏览器验收</B>，不接受「在这里过家家」。
          </li>
          <li className={liCls}>
            <B>坑：让我决策，却甩几万字文档。</B>→ 我要求它给我<B>推荐选项</B>
            让我选，而不是把一堆报告糊我脸上。
          </li>
          <li className={liCls}>
            <B>坑：跨机器的验证盲区。</B>Mac 上能写的都写完、Windows
            专属部分盲写，留一份逐条验证文档，到 Windows 上让另一个 AI 做最后对接。
          </li>
        </ul>

        <H3 no="05">{ARTICLE.headings[4]}</H3>
        <p className={pCls}>
          因为我认定：<B>极致的用户体验 × 极致的性能</B>
          ，是唯一的标尺。图标处理慢，我推动整个内核从 .NET 算法迁到 <B>Rust / WASM</B>
          ，在保证单一真理源的前提下榨性能。对齐差两个像素，我会让它重来——用户是视觉动物，你糊弄的每一处，他都感觉得到。我宁愿要一个小而认同我审美的用户群，也不要一个大而摇摆的。这不是完美主义作秀，是我知道好东西长什么样，而且不愿意将就。
        </p>

        <p className="mb-[18px] text-[14.5px] leading-[1.8] text-ink-3">{ARTICLE.toolsNote}</p>

        <div className="mt-12 bg-panel p-6 md:p-9">
          <p className="mb-1.5 inline-block border border-gold/50 px-2.5 py-[3px] font-mono text-[10.5px] tracking-[0.12em] text-gold">
            {ARTICLE.verdictBadge}
          </p>
          <h3 className="mt-2 text-[clamp(19px,2.8vw,25px)] font-bold leading-[1.3] tracking-[-0.01em]">
            {ARTICLE.verdictTitle}
          </h3>
          <p className="mt-3 text-[16px] leading-[1.8] text-ink-2">
            你让我别迎合，那我直说。先说结论：
            <B>这套打法是先进的，强度是过载的，成品大概率好看好用，但这条路对绝大多数人不可持续。</B>
          </p>

          <div className="my-6 grid gap-px bg-line sm:grid-cols-3">
            {ARTICLE.scoreItems.map((s) => (
              <div key={s.value} className="bg-card px-4 py-3.5">
                <div
                  className="font-mono text-[23px] font-bold leading-none"
                  style={{ color: s.tone === "teal" ? TONE.teal : s.tone === "gold" ? TONE.gold : TONE.coralInk }}
                >
                  {s.value}
                </div>
                <p className="mt-2 text-[12px] leading-[1.5] text-ink-2">{s.label}</p>
              </div>
            ))}
          </div>

          <p className="mb-[18px] text-[16px] leading-[1.8] text-ink-2">
            <B>真正做对的地方（不吹）：</B>你没把 AI
            当许愿池，而是当成一支能无限调度、但必须严格监工的施工队。三件事领先于大多数人的 AI
            工作流：<B>①</B> 用第二个模型做对抗审查，而不是信一个模型的自评；<B>②</B>{" "}
            把「反谄媚」制度化，主动逼 AI 反驳；<B>③</B>{" "}
            坚持真素材、真机验收，不被「能编译」和漂亮的 mock
            骗。这三点，是这个项目质量的真正来源。
          </p>

          <p className="mb-[18px] text-[16px] leading-[1.8] text-ink-2">
            <B>但代价和盲区也是真的：</B>
          </p>
          <ul className="mb-[18px] list-disc pl-[1.1em]">
            <li className="mb-2.5 text-[15.5px] leading-[1.75] text-ink-2">
              <B>情绪化沟通拖了效率。</B>
              「狗屎 / 无语 / 服了」这些词不增加信息量，真正推动修复的是后半句的精确诊断。同一个对齐问题磨了六七轮，其中有几轮是情绪先于诊断。把情绪和诊断分开，迭代会更快。
            </li>
            <li className="mb-2.5 text-[15.5px] leading-[1.75] text-ink-2">
              <B>「推倒重来」是把双刃剑。</B>三次满意都靠完全重写，反过来说明前期需求和 spec
              没有一次讲清——重写掩盖了「把话说明白」的成本。更早用一轮严格的需求澄清把边界钉死，能省掉好几次推倒。
            </li>
            <li className="mb-2.5 text-[15.5px] leading-[1.75] text-ink-2">
              <B>单人瓶颈不可复制。</B>
              全程你是唯一的验收人和唯一的真理源，热力图显示你近乎全天候在线。这套流程的产出上限由你的品味决定，但它绑死在你一个人身上，既无法持续，也无法交给团队跑。
            </li>
            <li className="mb-2.5 text-[15.5px] leading-[1.75] text-ink-2">
              <B>最大的未验证面在 Windows。</B>Mac
              上再多绿色门禁，也不等于真机可用；核心的系统集成靠「盲写 + 一个你自己都说更傻的 AI
              去后验」。这是整个项目工程风险最高、却最薄的一环。
            </li>
            <li className="mb-2.5 text-[15.5px] leading-[1.75] text-ink-2">
              <B>「恶心用户」是个人表达，不是产品策略。</B>
              作为态度它很酷；但作为一个要公开发布的开源产品，主动羞辱潜在用户，是把赌气和营销混在了一起，可能反噬口碑。这一条我只做客观提示，不评判你的动机。
            </li>
          </ul>

          <p className="text-[16px] leading-[1.8] text-ink-2">
            <B>一句话总评：</B>这不是「AI
            帮人写代码」的故事，是「一个有极致品味的人，把 AI
            当施工队严格监工」的故事。可复制的是方法（对抗式多模型、反谄媚、真机验收、文档治理防回归）；
            <B>不可复制的，是你愿意九天连轴、逐像素较真的那股劲</B>
            。前者值得所有人抄；后者，是这软件好看的原因，也是这条路难走的原因。
          </p>
        </div>

        <p className="mt-6 border-t border-line pt-[18px] font-mono text-[12px] text-ink-3">
          {ARTICLE.byline}
        </p>
      </div>
    </article>
  );
}
