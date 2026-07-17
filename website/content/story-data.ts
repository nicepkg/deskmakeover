/**
 * Session-analysis dataset for /story/ — machine-extracted VERBATIM from
 * docs/session-analysis/speech-dashboard.html (ai-command-center repo) by
 * scratchpad/extract-story-data.mjs. Do not hand-edit numbers; re-extract.
 *
 * Source: a nine-day Claude Code session (2026-07-08 → 2026-07-17), 341 human
 * messages after stripping tool results / system noise.
 */

export const META = {
  "span": "2026-07-08 → 2026-07-17",
  "days": 10,
  "turns": 341,
  "chars": 64427,
  "median": 67,
  "imgs": 63,
  "imgturns": 51,
  "night": 103,
  "nightpct": 30,
  "compact": 37,
  "model": 24,
  "interrupt": 19
} as const;

/** [term, total occurrences across all 341 messages] — descending. */
export const WORDCLOUD: readonly (readonly [string, number])[] = [["图标icon",328],["文档/spec/ADR",124],["subagent/专家团",94],["Windows盲写",88],["丑/难看",70],["角标/箭头",70],["壁纸",57],["优化",50],["颜色/配色",45],["codex审查",45],["history版本",45],["用户体验",44],["预设style",42],["继续",41],["分区zone",40],["动效/动画",39],["性能",39],["DRY/SOLID",38],["测试tests",36],["托盘/常驻",35],["settings设置",33],["Rust",29],["毛玻璃/玻璃",27],["commit/push",26],["渐变",22],["无语/服了",21],["圆角",20],["控制面板",19],["恶心用户",19],["极致",18],["重构/重写",17],["间距/padding",16],["welcome/欢迎页",16],["对齐",15],["礼花/庆祝",15],["狗屎/垃圾",15],["大胆反驳",13],["布局",11],["WASM",11],["响应式",10],["dev-cycle",9],["单一真理源",6],["强迫症",4]];

/** term -> category for the cloud + bars (design / engineering / attitude). */
export const CAT_DESIGN = new Set<string>(["图标icon","角标/箭头","壁纸","颜色/配色","预设style","分区zone","动效/动画","毛玻璃/玻璃","渐变","圆角","控制面板","间距/padding","settings设置","welcome/欢迎页","对齐","礼花/庆祝","布局","响应式","丑/难看"]);
export const CAT_ENG = new Set<string>(["文档/spec/ADR","Windows盲写","codex审查","history版本","性能","DRY/SOLID","测试tests","托盘/常驻","Rust","WASM","commit/push","重构/重写","单一真理源","dev-cycle","subagent/专家团"]);

/** [dominant emotion class, message count]. */
export const EMOTION: readonly (readonly [string, number])[] = [["中性指令",177],["批评/宣泄",81],["放权/催进度",36],["肯定/赞赏",27],["探讨/授权反驳",20]];

/** Messages per hour of day, index 0-23. */
export const HOURS: readonly number[] = [18,30,17,9,18,11,8,0,0,3,3,3,8,16,9,12,18,22,26,34,28,22,17,9];

/** [MM-DD, messages that day]. */
export const DAYS: readonly (readonly [string, number])[] = [["07-08",55],["07-09",70],["07-10",38],["07-11",49],["07-12",28],["07-13",19],["07-14",37],["07-15",8],["07-16",19],["07-17",18]];

/** [weekday, messages] — kept from the source dataset (not charted). */
export const DOWS: readonly (readonly [string, number])[] = [["周一",19],["周二",37],["周三",63],["周四",89],["周五",56],["周六",49],["周日",28]];

/** "MM-DD|hour" -> message count (sparse). */
export const HEAT: Readonly<Record<string, number>> = {
  "07-08|16": 2,
  "07-08|18": 7,
  "07-08|19": 14,
  "07-08|20": 17,
  "07-08|21": 8,
  "07-08|22": 7,
  "07-09|0": 8,
  "07-09|1": 7,
  "07-09|2": 2,
  "07-09|3": 2,
  "07-09|4": 6,
  "07-09|5": 2,
  "07-09|6": 1,
  "07-09|12": 5,
  "07-09|13": 8,
  "07-09|14": 4,
  "07-09|15": 1,
  "07-09|16": 4,
  "07-09|17": 3,
  "07-09|18": 4,
  "07-09|19": 9,
  "07-09|20": 3,
  "07-09|23": 1,
  "07-10|0": 3,
  "07-10|1": 4,
  "07-10|2": 1,
  "07-10|3": 3,
  "07-10|4": 1,
  "07-10|10": 1,
  "07-10|11": 2,
  "07-10|14": 2,
  "07-10|15": 2,
  "07-10|17": 5,
  "07-10|18": 5,
  "07-10|21": 7,
  "07-10|22": 2,
  "07-11|1": 7,
  "07-11|2": 5,
  "07-11|3": 2,
  "07-11|4": 7,
  "07-11|5": 6,
  "07-11|6": 1,
  "07-11|10": 2,
  "07-11|11": 1,
  "07-11|14": 1,
  "07-11|17": 3,
  "07-11|19": 3,
  "07-11|20": 4,
  "07-11|21": 1,
  "07-11|22": 2,
  "07-11|23": 4,
  "07-12|0": 2,
  "07-12|1": 6,
  "07-12|2": 1,
  "07-12|6": 4,
  "07-12|12": 3,
  "07-12|13": 4,
  "07-12|14": 1,
  "07-12|16": 3,
  "07-12|17": 1,
  "07-12|21": 1,
  "07-12|22": 1,
  "07-12|23": 1,
  "07-13|2": 1,
  "07-13|3": 2,
  "07-13|4": 1,
  "07-13|5": 1,
  "07-13|9": 1,
  "07-13|13": 1,
  "07-13|15": 1,
  "07-13|16": 3,
  "07-13|17": 2,
  "07-13|18": 1,
  "07-13|19": 1,
  "07-13|21": 1,
  "07-13|22": 3,
  "07-14|0": 5,
  "07-14|2": 3,
  "07-14|9": 1,
  "07-14|13": 1,
  "07-14|16": 2,
  "07-14|17": 6,
  "07-14|18": 8,
  "07-14|19": 3,
  "07-14|20": 2,
  "07-14|21": 2,
  "07-14|22": 2,
  "07-14|23": 2,
  "07-15|1": 5,
  "07-15|4": 2,
  "07-15|5": 1,
  "07-16|15": 5,
  "07-16|16": 2,
  "07-16|17": 2,
  "07-16|18": 1,
  "07-16|19": 4,
  "07-16|20": 2,
  "07-16|21": 2,
  "07-16|23": 1,
  "07-17|1": 1,
  "07-17|2": 4,
  "07-17|4": 1,
  "07-17|5": 1,
  "07-17|6": 2,
  "07-17|9": 1,
  "07-17|13": 2,
  "07-17|14": 1,
  "07-17|15": 3,
  "07-17|16": 2
};

export const HEAT_DAYS: readonly string[] = ["07-08","07-09","07-10","07-11","07-12","07-13","07-14","07-15","07-16","07-17"];

/** Message-length buckets: ≤20 / 20-80 / 80-200 / 200-500 / >500 chars. */
export const LEN_BUCKETS: readonly number[] = [97,94,88,49,13];

/** Behavioral signal -> message count. */
export const SIGNALS: Readonly<Record<string, number>> = {
  "丑/难看": 39,
  "批评宣泄": 31,
  "肯定赞赏": 43,
  "大胆反驳": 11,
  "极致": 16,
  "强迫症/难受": 14,
  "codex": 27,
  "专家团": 36,
  "继续催进度": 39,
  "恶心用户": 8
};

/** Per-message emotional valence (-3..+3), ordinal = message order. */
export const SERIES: readonly number[] = [0,-2,-1,-2,-2,-2,-2,-1,0,0,0,-1,0,0,0,-3,2,-1,-1,-1,0,-2,0,0,-1,-1,2,-2,-2,-1,0,-2,-1,-2,-1,0,-2,-2,-2,0,0,1,2,0,0,1,0,0,1,2,0,-1,-2,0,0,0,2,0,-2,3,3,0,0,1,-1,0,0,0,2,0,1,1,1,3,0,2,0,-1,1,1,0,1,0,0,-3,0,1,-1,0,-3,-2,0,-2,1,1,0,1,1,0,0,0,1,-2,0,0,0,0,0,0,0,0,1,0,-1,-1,1,-1,0,0,2,1,1,0,1,0,0,0,-3,-3,1,0,0,-1,0,0,0,0,1,0,0,-1,1,-1,-1,0,0,0,-1,0,-2,-2,-1,0,0,0,0,0,-3,-3,0,0,0,0,-1,0,1,0,0,0,0,0,0,1,0,-1,0,0,0,0,0,0,-1,0,0,0,0,0,0,-1,2,0,1,0,1,0,0,-1,0,0,0,1,0,2,1,1,0,0,-2,0,0,0,-1,0,0,-1,0,0,0,0,0,2,0,0,-1,-1,0,2,2,0,0,0,0,0,0,0,1,0,1,3,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,-2,0,0,-1,0,0,0,0,0,0,0,0,0,0,-2,0,-1,-1,1,0,-1,-2,-1,0,-2,-2,-2,0,0,-2,-2,0,0,0,-1,-3,-3,0,-3,2,2,0,2,-2,0,0,0,0,0,0,1,1,0,-1,-1,-1,-2,0,0,-2,0,-2,-2,0,-2,0,3,0,-1,-2,0,0,0,-2,-3,0,0,-2,0,0,-3,0,0,-1,0,0];

export const DAY_LIST: readonly string[] = ["07-08","07-09","07-10","07-11","07-12","07-13","07-14","07-15","07-16","07-17"];

/** Ordinal of each day's first message (aligns SERIES to DAY_LIST). */
export const DAY_FIRST: readonly number[] = [0,55,125,163,212,240,259,296,304,323];

/** Tide-chart annotations: message ordinal, title, quote, direction. */
export const ANNOS: readonly { i: number; t: string; q: string; dir: "up" | "down" }[] = [
  {
    "i": 2,
    "t": "第一次掀桌",
    "q": "「这他妈的也叫设计？就是狗屎」",
    "dir": "down"
  },
  {
    "i": 73,
    "t": "首次真心夸",
    "q": "「必须夸奖你，干得不错」",
    "dir": "up"
  },
  {
    "i": 127,
    "t": "礼花之怒",
    "q": "「礼花礼花礼花，我打死你」",
    "dir": "down"
  },
  {
    "i": 203,
    "t": "WASM 交付",
    "q": "「我看了很满意，挺不错的」",
    "dir": "up"
  },
  {
    "i": 291,
    "t": "最深低谷",
    "q": "「你这个 Claude 就是个垃圾」",
    "dir": "down"
  },
  {
    "i": 295,
    "t": "谷底反弹",
    "q": "「这次你终于弄对了，非常满意」",
    "dir": "up"
  },
  {
    "i": 329,
    "t": "3D 官网返工",
    "q": "「你写的什么狗屎模型」",
    "dir": "down"
  },
  {
    "i": 340,
    "t": "收官定调",
    "q": "「我要一百分的完美试卷」",
    "dir": "up"
  }
];

/** Per-day valence stats — kept from the source dataset (not charted). */
export const DAY_STATS: readonly { day: string; mean: number; min: number; max: number; n: number }[] = [
  {
    "day": "07-08",
    "mean": -0.58,
    "min": -3,
    "max": 2,
    "n": 55
  },
  {
    "day": "07-09",
    "mean": 0.21,
    "min": -3,
    "max": 3,
    "n": 70
  },
  {
    "day": "07-10",
    "mean": -0.5,
    "min": -3,
    "max": 1,
    "n": 38
  },
  {
    "day": "07-11",
    "mean": 0.06,
    "min": -2,
    "max": 2,
    "n": 49
  },
  {
    "day": "07-12",
    "mean": 0.29,
    "min": -1,
    "max": 3,
    "n": 28
  },
  {
    "day": "07-13",
    "mean": -0.11,
    "min": -2,
    "max": 1,
    "n": 19
  },
  {
    "day": "07-14",
    "mean": -0.62,
    "min": -3,
    "max": 2,
    "n": 37
  },
  {
    "day": "07-15",
    "mean": 0,
    "min": -2,
    "max": 2,
    "n": 8
  },
  {
    "day": "07-16",
    "mean": -0.42,
    "min": -2,
    "max": 3,
    "n": 19
  },
  {
    "day": "07-17",
    "mean": -0.78,
    "min": -3,
    "max": 0,
    "n": 18
  }
];

/** Per-day mean message length + delegation share — source dataset (cited in insights). */
export const TRUST: readonly { day: string; meanlen: number; delegpct: number }[] = [
  {
    "day": "07-08",
    "meanlen": 138,
    "delegpct": 0
  },
  {
    "day": "07-09",
    "meanlen": 216,
    "delegpct": 7
  },
  {
    "day": "07-10",
    "meanlen": 486,
    "delegpct": 8
  },
  {
    "day": "07-11",
    "meanlen": 73,
    "delegpct": 18
  },
  {
    "day": "07-12",
    "meanlen": 201,
    "delegpct": 29
  },
  {
    "day": "07-13",
    "meanlen": 296,
    "delegpct": 26
  },
  {
    "day": "07-14",
    "meanlen": 115,
    "delegpct": 0
  },
  {
    "day": "07-15",
    "meanlen": 34,
    "delegpct": 38
  },
  {
    "day": "07-16",
    "meanlen": 97,
    "delegpct": 5
  },
  {
    "day": "07-17",
    "meanlen": 114,
    "delegpct": 11
  }
];

/** Per-day keyword share (%) of design / engineering / outbound topics. */
export const DRIFT: readonly { day: string; design: number; eng: number; out: number }[] = [
  {
    "day": "07-08",
    "design": 90,
    "eng": 7,
    "out": 2
  },
  {
    "day": "07-09",
    "design": 89,
    "eng": 9,
    "out": 1
  },
  {
    "day": "07-10",
    "design": 51,
    "eng": 41,
    "out": 8
  },
  {
    "day": "07-11",
    "design": 38,
    "eng": 62,
    "out": 0
  },
  {
    "day": "07-12",
    "design": 28,
    "eng": 71,
    "out": 1
  },
  {
    "day": "07-13",
    "design": 14,
    "eng": 84,
    "out": 2
  },
  {
    "day": "07-14",
    "design": 54,
    "eng": 46,
    "out": 0
  },
  {
    "day": "07-15",
    "design": 38,
    "eng": 62,
    "out": 0
  },
  {
    "day": "07-16",
    "design": 62,
    "eng": 21,
    "out": 17
  },
  {
    "day": "07-17",
    "design": 35,
    "eng": 2,
    "out": 63
  }
];

/** Aggregate stats cited by the insight cards. */
export const DEEP = {
  "neg_total": 94,
  "neg_with_fix": 76,
  "neg_fix_pct": 81,
  "night_v": 0.05,
  "day_v": -0.32,
  "night_len": 240.94,
  "day_len": 166.43,
  "night_n": 103,
  "day1_len": 138,
  "last_len": 114
} as const;

/** Valence histogram: score (-3..+3 as string) -> message count. */
export const VDIST: Readonly<Record<string, number>> = {
  "0": 189,
  "1": 37,
  "2": 16,
  "3": 5,
  "-3": 12,
  "-2": 38,
  "-1": 44
};

export const TOTAL = 341;
