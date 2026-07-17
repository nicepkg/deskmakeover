import type { EngineDict } from "./engine-types";

const GH = "https://github.com/nicepkg/deskmakeover";
const CORE = `${GH}/tree/main/crates/dm-icon-core`;
const BLOB = `${GH}/blob/main/crates/dm-icon-core`;

export const ENGINE_ZH: EngineDict = {
  meta: {
    title: "像素引擎：驱动 DeskMakeover 的自研像素计算核心",
    description:
      "DeskMakeover 的底层是一台自研确定性像素计算引擎：读懂真实图标、精准分离、自动救援、现场渲染质感。桌面端与这个网页跑同一套引擎，页面里可以亲手驱动它。",
  },
  hero: {
    eyebrow: "ENGINE · DM-ICON-CORE",
    title: "这台像素引擎，我们自己造的",
    sub: "DeskMakeover 的每一次换装，都由 dm-icon-core 驱动：一台近一万二千行纯 Rust 写成的确定性像素计算引擎。它先读懂每一颗图标再动手，同样的输入永远得到同样的像素。桌面端和这个网页跑的是同一套引擎，下面所有画面都是它当场算出来的。",
    stats: [
      { value: 11946, unit: " 行", label: "引擎代码，纯 Rust" },
      { value: 1487, unit: " 颗", label: "真实图标逐字节校验" },
      { value: 57, unit: " 项", label: "核心算法测试" },
      { value: 88, unit: " KB", label: "整个引擎装进这个网页" },
    ],
    stack: {
      raw: "原来的图标",
      plate: "算出来的底盘",
      final: "换好外观的成品",
      caption: "引擎的装配线，像发动机的分解图：先读原图，再算底盘，最后合成成品。每一层都是引擎的真实输出，拖动可以旋转查看。",
    },
  },
  read: {
    index: "01",
    kicker: "READ",
    title: "引擎的视觉：先读懂，再动手",
    body: "每颗图标进入引擎，先经过一轮完整分析：主色是什么，带没带自己的背景，轮廓是不是标准形状。分析结果写进一份档案，后面每个环节都读这份档案，不靠猜。这是这台引擎所有能力的地基。",
    steps: [
      { key: "KIND", label: "认类型" },
      { key: "BG", label: "找背景" },
      { key: "COLOR", label: "取主色" },
      { key: "SHAPE", label: "对轮廓" },
      { key: "PROFILE", label: "出报告" },
    ],
    caption: "扫描掠过这颗真实的「此电脑」，轮廓层被提起，主色被抽出，档案就此建立。拖动可以旋转查看。",
  },
  cut: {
    index: "02",
    kicker: "CUT",
    title: "精准分离：图案和底，一刀切开",
    body: "很多图标自带底板，比如这颗「控制面板」。引擎认出底板不是图案，把两层干净分开，再为图案配上一块自己算出来的底盘。整套判断由算法完成，不需要人工描一笔。",
    caption: "泛珊瑚色的那一层，是引擎判定的「底」。这份逐像素的判定与桌面端引擎完全一致。",
    replay: "重播",
    maskNote: "01 认出底板 · 02 整层剥离 · 03 换上新底盘",
    layers: { bg: "自带的底板", art: "真正的图案", final: "换好底盘的成品" },
  },
  rescue: {
    index: "03",
    kicker: "RESCUE",
    title: "自动救援：撞色了，引擎自己处理",
    body: "蓝信封遇上蓝底盘，眼看要融进去。引擎沿边缘取样，判断出颜色差和明暗差都不够用，立刻补一圈描边、垫一层影子。这不是预设的特例，是引擎对每一颗图标都在做的实时判断。",
    beats: [
      { key: "01", title: "撞色", detail: "蓝配蓝，信封边缘快看不见了" },
      { key: "02", title: "发现", detail: "沿着边缘取样，颜色差和明暗差都不达标" },
      { key: "03", title: "救回", detail: "救援层合入，信封回来了" },
    ],
    offLabel: "关掉救援",
    onLabel: "打开救援",
    caption: "悬浮的那一层，就是引擎算出来的救援内容：一圈描边加一层影子。两种状态都是引擎真实渲染。",
    layers: { tile: "撞色的瓦片", rescue: "引擎算出的救援层" },
  },
  promise: {
    index: "04",
    kicker: "PROMISE",
    title: "铁律：品牌色，一个像素都不动",
    body: "这台引擎有一条写死在代码里的规矩：图案本身永不改色。区分度全部来自底盘、描边和影子。当几颗图标的底盘撞色，引擎把底盘错开一点，让每颗都认得清，图案原封不动。",
    rule: "图案像素永不改色",
    before: "错开前",
    after: "错开后",
    caption: "三颗蓝色系图标，底盘被推开到互相认得清。图案一个像素都没动。",
  },
  finish: {
    index: "05",
    kicker: "FINISH",
    title: "质感渲染：玻璃、像素、贴纸，全部现算",
    body: "质感不是叠一张滤镜图，是引擎的渲染能力。反光、颗粒、白边、投影，都是对着当前这颗图标逐像素算出来的。换一颗图标，全部重新算。",
    finishes: [
      { key: "glass", kicker: "GLASS", name: "玻璃", line: "反光和边缘折射按图形现算" },
      { key: "pixel", kicker: "PIXEL", name: "像素", line: "逐格取平均，再配上糖果色" },
      { key: "sticker", kicker: "STICKER", name: "贴纸", line: "白边沿着轮廓外一圈生长" },
    ],
  },
  live: {
    index: "06",
    kicker: "LIVE",
    title: "引擎就在这个网页里",
    body: "不用装软件，引擎已经装进了这一页：88 KB，就是桌面端出厂的那套 Rust 代码。挑一颗真实图标，或者传一张你自己的图，每拖一下都是当场算出来的，不是预录视频。",
    badge: "与桌面端字节级一致",
    castLabel: "真实图标",
    uploadCta: "用自己的图试试",
    uploadNote: "图片只在你的浏览器里处理，不会上传到任何地方。",
    controls: {
      shape: "形状",
      look: "外观",
      hue: "底盘色相",
      finish: "质感",
      original: "对比原图",
    },
    options: {
      shapes: [
        { tag: "Apple", label: "圆角方" },
        { tag: "Circle", label: "正圆" },
        { tag: "Tile", label: "方砖" },
        { tag: "Diamond", label: "菱形" },
        { tag: "Flower", label: "花瓣" },
        { tag: "Pebble", label: "卵石" },
      ],
      looks: [
        { tag: "Original", label: "原彩" },
        { tag: "BlackWhite", label: "黑白" },
        { tag: "Mono", label: "单色" },
      ],
      finishes: [
        { tag: "None", label: "无" },
        { tag: "Glass", label: "玻璃" },
        { tag: "Pixel", label: "像素" },
        { tag: "Sticker", label: "贴纸" },
      ],
    },
    autoHue: "自动",
    loading: "正在加载引擎",
    fallbackNote: "当前浏览器无法运行 WASM，以下为引擎预渲染的效果图。",
  },
  receipts: {
    index: "07",
    kicker: "RECEIPTS",
    title: "给工程师的收据",
    body: "上面每一句话都能在源码里对账：1,487 颗真实图标逐字节校验，浏览器里的演示与桌面端字节级一致。每个数字都可以点开看。",
    receipts: [
      { value: "11,946 行", label: "纯 Rust 像素核心", href: CORE },
      { value: "forbid(unsafe_code)", label: "核心禁用 unsafe", href: `${BLOB}/src/lib.rs` },
      { value: "1,487 颗", label: "字节级校验语料", href: `${BLOB}/tests/parity_determinism.rs` },
      { value: "57 项", label: "核心算法测试", href: `${CORE}/tests` },
      { value: "88 KB", label: "wasm 构建（gzip）", href: `${GH}/tree/main/crates/dm-icon-wasm` },
      { value: "MIT", label: "全部开源", href: `${GH}/blob/main/LICENSE` },
    ],
  },
  cta: {
    title: "这台引擎，已经在等你的桌面",
    body: "它不是演示品，桌面上它天天干活。动手之前先给桌面拍快照，不满意一键回到原样。",
    download: "下载 DeskMakeover",
    github: "去 GitHub 读源码",
  },
  castNames: {
    folder: "文件夹",
    pics: "图片文件夹",
    bin: "回收站",
    thispc: "此电脑",
    camera: "相机",
    mail: "邮件",
    maps: "地图",
    panel: "控制面板",
  },
};

/** Every /engine/ string that renders in the zh display face (h1/h2/h3). */
export const ENGINE_ZH_DISPLAY_STRINGS: string[] = [
  ENGINE_ZH.hero.title,
  ENGINE_ZH.read.title,
  ENGINE_ZH.cut.title,
  ENGINE_ZH.rescue.title,
  ENGINE_ZH.promise.title,
  ENGINE_ZH.finish.title,
  ENGINE_ZH.live.title,
  ENGINE_ZH.receipts.title,
  ENGINE_ZH.cta.title,
];
