/**
 * Copy for /story/ — the making-of dashboard for DeskMakeover v1, generated
 * from a full analysis of Claude Code session c69bf900 (2026-07-08 → 07-17).
 * All analysis copy is preserved VERBATIM from the source report
 * (speech-dashboard.html); only page chrome strings are original here.
 * Chinese-language page by nature: the raw quotes and the whole session
 * happened in Chinese. Rich inline prose (bold runs, highlights) lives as JSX
 * in components/story/; this module holds every plain string, so the zh
 * display-font subset script can see the headings.
 */

export const STORY_META = {
  path: "/story/",
  title: "死磕像素的九日：DeskMakeover 首个版本创作实录",
  description:
    "九天 341 条真实指令的全量分析：一个人怎么指挥两个 AI，把 DeskMakeover 的第一个版本从零逼到发版。词云、情绪潮汐、作息热力图，以及一篇不加修饰的复盘长文。",
  datePublished: "2026-07-17",
};

/** Landing-page entry labels (nav + footer). */
export const STORY_ENTRY = { zh: "创作历程", en: "Story" };

export const HERO = {
  eyebrow: "发言语料分析 · Claude Code Session c69bf900",
  titlePre: "死磕像素的",
  titleAccent: "九日",
  // sub renders in JSX with 「341 句话」 bolded
  stats: [
    { value: 341, decimals: 0, suffix: "", label: "真人发言（已剔除工具与系统噪声）" },
    { value: 64.4, decimals: 1, suffix: "k字", label: "总字数 · 中位仅 67 字" },
    { value: 30, decimals: 0, suffix: "%", label: "发言在凌晨 0 到 6 点" },
    { value: 63, decimals: 0, suffix: "张", label: "甩给 AI 的验收截图" },
    { value: 37, decimals: 0, suffix: "次", label: "上下文压缩 · 会话跨 9 天不断线" },
  ],
};

export interface StoryHead {
  index: string;
  kicker: string;
  title: string;
  hint: string;
}

export const HEADS = {
  cloud: {
    index: "01",
    kicker: "词云 · WORD CLOUD",
    title: "你这九天在念叨什么",
    hint: "按主题词在全部发言里出现的总次数加权。三色编码：设计观感、工程技术、态度口头禅。",
  },
  concerns: {
    index: "02",
    kicker: "高频关注点 · TOP CONCERNS",
    title: "注意力都花在哪",
    hint: "「图标」以 328 次断层第一。你的九天，本质是一场围绕图标形状、角标与配色的像素战争。",
  },
  sentiment: {
    index: "03",
    kicker: "情绪光谱 · SENTIMENT",
    title: "骂与夸，几乎一比一",
    hint: "按主导情绪给每条发言归类。批评从不是纯发泄，后面永远跟着精确诊断。",
  },
  tide: {
    index: "04",
    kicker: "情绪潮汐 · EMOTIONAL TIDE",
    title: "九天里，你的情绪是怎么涨落的",
    hint: "给 341 条发言逐条打分（褒 +3 到 贬 −3），再取滑动平均画出潮线。金色在水上是满意，珊瑚在水下是不满。",
  },
  drift: {
    index: "05",
    kicker: "关注点迁移 · FOCUS DRIFT",
    title: "九天走完，重心从「好不好看」滑到「怎么发出去」",
    hint: "每天所有发言里三类关键词的占比。清晰的三段式：先磨观感，再攻内核，最后做对外。",
  },
  heat: {
    index: "06",
    kicker: "写代码时间 · ACTIVITY HEATMAP",
    title: "你的九天作息，摊开在这",
    hint: "每格是某天某小时的发言密度。深夜与傍晚两条亮带，7 到 8 点几乎空白（睡觉）。",
  },
  rhythm: {
    index: "07",
    kicker: "昼夜节律 · CIRCADIAN",
    title: "一天里，凌晨 2 点和晚 7 点是双峰",
    hint: "蓝色柱是 0 到 6 点的深夜时段。你是彻底的夜行动物。",
  },
  signals: {
    index: "09",
    kicker: "行为信号 · BEHAVIORAL SIGNALS",
    title: "刻进肌肉记忆的口头禅",
    hint: "含该信号的发言条数。这些数字，就是你的工作画像。",
  },
  insights: {
    index: "10",
    kicker: "子策的解读 · WHAT THE DATA SAYS",
    title: "数字底下，我读到的五件事",
    hint: "这一节是我（子策）的判断，不只是统计。凡有数据支撑的都标了出处。",
  },
  quotes: {
    index: "11",
    kicker: "原声语录 · VERBATIM",
    title: "你亲口说的九句",
    hint: "未加工，按情绪着色。从掀桌到「终于弄对了」。",
  },
  article: {
    index: "12",
    kicker: "分享长文 · THE BUILD STORY",
    title: "把这九天，讲给别人听",
    hint: "如果要向别人分享「用 AI 从零到一造一个软件」是什么体验，我会这样讲。文末是我不加修饰的评价。",
  },
} satisfies Record<string, StoryHead>;

/** Section 08 is a two-card grid; each card carries its own head. */
export const ARC_CARD = {
  index: "08",
  kicker: "每日弧线 · DAILY ARC",
  title: "九天叙事",
};
export const LENGTH_CARD = {
  kicker: "发言长度 · MESSAGE LENGTH",
  title: "短促催促 vs 长篇规格",
  hint: "一半发言不到 67 字（催进度、甩截图、放权），另一头是数百字的产品规格与决策。",
};

export const CLOUD_LEGEND = [
  { label: "设计 · 观感", tone: "coral" },
  { label: "工程 · 技术", tone: "teal" },
  { label: "态度 · 口头禅", tone: "gold" },
] as const;

export const CONCERNS_AXIS = {
  colTerm: "主题词",
  colCount: "出现次数",
};

export const TIDE_AXIS = {
  histCaption: "▲ 情绪分布：中性占多数（189 条），但一旦开口，贬多于褒（94 : 58）。",
};

export const DRIFT_AXIS = {
  y: "纵轴 · 关键词占比（0 → 100%）",
  x: "横轴 · 日期（07-08 → 07-17）",
  legend: [
    { label: "设计 · 观感（图标 / 壁纸 / 配色 / 丑）", tone: "coral" },
    { label: "工程 · 内核（Rust / 性能 / codex / 文档）", tone: "teal" },
    { label: "对外 · 交付（官网 / README / 发版 / SEO）", tone: "gold" },
  ],
} as const;

export const HEAT_AXIS = {
  y: "纵轴 · 日期（月-日）",
  x: "横轴 · 一天 24 小时（0 → 23 时）",
  caption: "▲ 横轴：小时（时）",
  keyLow: "发言条数  少",
  keyHigh: "多（单格峰值 17 条）",
};

export const RHYTHM_AXIS = {
  ylab: "发言条数",
};

/** Day -> that day's storyline note (Daily Arc). */
export const DAY_NOTES: Record<string, string> = {
  "07-08": "开局即掀桌，UI 全面推倒重来",
  "07-09": "密度巅峰，从毛玻璃到图标算法",
  "07-10": "文档治理 + 礼花与「恶心用户」产品线",
  "07-11": "交互峰值，图标内核迁移 Rust / WASM",
  "07-12": "规划后台常驻「大脑」M6 / M7",
  "07-13": "无人值守，Codex 地毯式审查",
  "07-14": "情绪低谷，流体玻璃复刻拉锯",
  "07-15": "预设与背景收尾，commit",
  "07-16": "Windows 真机跑通 + README 官网",
  "07-17": "3D 官网重写 + 发版全自动化",
};

export const LEN_LABELS = [
  "≤ 20 字（催促 / 放权）",
  "20 到 80 字",
  "80 到 200 字",
  "200 到 500 字（细节反馈）",
  "> 500 字（规格 / 粘贴）",
];

/** [signals key, card label] — order defines the chip grid. */
export const SIGNAL_META: readonly (readonly [string, string])[] = [
  ["丑/难看", "说「丑」或「难看」"],
  ["批评宣泄", "强烈批评的发言"],
  ["肯定赞赏", "真诚肯定的发言"],
  ["大胆反驳", "喊 AI「大胆反驳我」"],
  ["极致", "追求「极致」体验或性能"],
  ["强迫症/难受", "自述强迫症 / 看着难受"],
  ["codex", "要 Codex 对抗审查"],
  ["专家团", "开 subagent 专家团"],
  ["继续催进度", "一句「继续」催进度"],
  ["恶心用户", "设计「恶心那批黑我的人」"],
];

export interface StoryQuote {
  text: string;
  date: string;
  label: string;
  tone: "neg" | "pos" | "chal" | "phil";
}

export const QUOTES: readonly StoryQuote[] = [
  {
    text: "这他妈的也叫设计？文字都溢出来了，高度乱用。你写出来的这些东西就是狗屎，你知道吗？我叫你完全重构，不是让你保留原来的样子。",
    date: "07-08",
    label: "掀桌",
    tone: "neg",
  },
  {
    text: "扁平化大哥，一堆卡片看起来奇奇怪怪的，有些根本没必要存在，连成一张不就行了？求求你，你能不能有点审美？",
    date: "07-08",
    label: "审美质问",
    tone: "neg",
  },
  {
    text: "这里必须夸奖一下你，干得不错。整套设计非常好看，设计师很有品味。",
    date: "07-09",
    label: "真诚肯定",
    tone: "pos",
  },
  {
    text: "你可以大胆反驳我，或者你有更好的想法也可以提出来。总之，为了极致的用户体验。",
    date: "07-09",
    label: "授权反驳",
    tone: "chal",
  },
  {
    text: "做完再停，无人值守模式，有不确定的攒到做完后给我过一下，我睡觉了。",
    date: "07-13",
    label: "高信任放权",
    tone: "phil",
  },
  {
    text: "我要的是最终视觉上的对齐，不是代码层面的对齐。无语死了。",
    date: "07-14",
    label: "像素洁癖",
    tone: "neg",
  },
  {
    text: "我真是要吐血了！我给了你一个完整的 repo 参考，你为什么还原得跟狗屎一样？我要真正的、有质量的、精美的东西。",
    date: "07-14",
    label: "情绪低谷",
    tone: "neg",
  },
  {
    text: "这次你终于弄对了。我非常满意。",
    date: "07-14",
    label: "闭环",
    tone: "pos",
  },
  {
    text: "如果我每次发版都要手动更新网站，那就是失败的。我要一百分的完美试卷，不要六十分。",
    date: "07-17",
    label: "交付哲学",
    tone: "phil",
  },
];

export const QUOTE_TONE_NAME = { neg: "批评", pos: "肯定", chal: "探讨", phil: "哲学" } as const;

/** Valence histogram labels, score -3..+3. */
export const VDIST_LABELS: Record<string, string> = {
  "-3": "暴怒",
  "-2": "不满",
  "-1": "挑刺",
  "0": "中性",
  "1": "认可",
  "2": "满意",
  "3": "很满意",
};

/** Insight card scaffolding; bodies are rich JSX in components/story/insights.tsx. */
/** figs render as JSX (count-up spans) in components/story/insights.tsx */
export const INSIGHT_HEADS = [
  { tag: "观察 01 · 愤怒的结构", title: "你几乎从不「只骂不指路」" },
  { tag: "观察 02 · 信任是让渡出来的", title: "从逐像素盯，到睡前一句「继续」" },
  { tag: "观察 03 · 深夜的你，其实更平静", title: "白天你在挑刺，深夜你在建构" },
  { tag: "观察 04 · 满意来自推倒", title: "你最满意的时刻，都跟在「完全重写」之后" },
  { tag: "观察 05 · 反驳的边界", title: "你要的是压力测试，不是投票" },
] as const;

export const INSIGHT_SURPRISE = "反直觉 · 我原来的猜测被数据推翻了";

export const ARTICLE = {
  kicker: "从零到一 · DESKMAKEOVER",
  title: "我用两个 AI，九天磕出一个我自己愿意用的软件",
  headings: [
    "起因：我受够了丑东西",
    "分工：一个写，一个当「对手」",
    "我怎么「设计 AI」：五条指挥法",
    "我踩的 AI 坑，和我的应对",
    "我为什么这么「难伺候」",
  ],
  pullquote:
    "最顶级的模型也会自信满满地把没测过的东西说成「搞定了」。你不搭兜底，它就把你带沟里。",
  toolsNote:
    "工具：Claude Code（claude-fable-5 / claude-opus-4-8）· Codex（gpt-5.6 sol ultra）· 周期：2026 年 7 月 8 日至 17 日，9 天，341 条指令。",
  verdictBadge: "子策的独立评价 · 不迎合",
  verdictTitle: "中立地说，这是一个什么样的过程",
  scoreItems: [
    { value: "先进", tone: "teal", label: "方法论：对抗式多模型 + 反谄媚 + 真机验收" },
    { value: "过载", tone: "gold", label: "强度：9 天连轴、单人验收、深夜为主" },
    { value: "有风险", tone: "coral", label: "盲区：Windows 全靠盲写 + 更弱 AI 后验" },
  ] as const,
  byline:
    "述评 · 子策（Ace）｜ 基于会话 c69bf900 全量 341 条发言与工程记录，独立成文，未经润色迎合。",
};

/** Every string that renders in the zh display face on /story/ (h1/h2/h3 + pullquote). */
export const STORY_DISPLAY_STRINGS: string[] = [
  HERO.titlePre + HERO.titleAccent,
  ...Object.values(HEADS).map((h) => h.title),
  ARC_CARD.title,
  LENGTH_CARD.title,
  ...INSIGHT_HEADS.map((i) => i.title),
  ARTICLE.title,
  ...ARTICLE.headings,
  ARTICLE.pullquote,
  ARTICLE.verdictTitle,
  STORY_ENTRY.zh,
];
