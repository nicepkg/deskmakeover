import type { EngineDict } from "./engine-types";

const GH = "https://github.com/nicepkg/deskmakeover";
const CORE = `${GH}/tree/main/crates/dm-icon-core`;
const BLOB = `${GH}/blob/main/crates/dm-icon-core`;

export const ENGINE_ZH: EngineDict = {
  meta: {
    title: "像素引擎：DeskMakeover 如何读懂并重绘你的图标",
    description:
      "从画像、分离、救援到质感，一条约 12,000 行的纯 Rust 自研确定性像素管线，在桌面端与浏览器输出字节级一致的像素。逐步看它运行，并在浏览器里亲手驱动同一个 WASM 构建。",
  },
  hero: {
    eyebrow: "ENGINE · DM-ICON-CORE",
    title: "一颗图标，如何被读懂",
    sub: "每一套外观背后是同一条自研确定性像素管线：先读懂每颗图标，再在硬规则下重绘。零机器学习，零上传，同样的输入永远得到同样的像素。这一页会让它逐步跑给你看，最后交到你手上。",
    stats: [
      { value: 11946, unit: " 行", label: "纯 Rust 像素核心" },
      { value: 1487, unit: " 颗", label: "字节级校验语料图标" },
      { value: 57, unit: " 项", label: "核心算法测试" },
      { value: 88, unit: " KB", label: "浏览器里的同款引擎" },
    ],
  },
  portrait: {
    index: "01",
    kicker: "PORTRAIT",
    title: "先给每颗图标画一次像",
    body: "动手之前，管线先完成一次完整的阅读。五步分类判断它是什么，沿画布边缘与形状环采样找出它自带的背景，在 OKLab 空间提取主色与色相离散度，再用轮廓匹配确认它是否就是一个标准形状。每颗图标得到一份画像，之后的每个阶段都读这份画像，而不是重新猜。",
    steps: [
      { key: "CLASSIFY", label: "分类" },
      { key: "BACKGROUND", label: "背景" },
      { key: "COLOR", label: "主色" },
      { key: "SILHOUETTE", label: "轮廓" },
      { key: "PROFILE", label: "画像" },
    ],
    probeCaption: "画布边缘环与形状环上的采样探针。角对称判别器专门排除折角文档页的误判。",
    iouLabel: "轮廓匹配判定阈值",
  },
  separate: {
    index: "02",
    kicker: "SEPARATE",
    title: "哪里是主体，哪里是底",
    body: "有透明边的图标直接交出 alpha 轮廓。不透明的图标从边界种下种子，按局部容差一层一层向内洪泛，把背景吃掉，留下主体。碟形图标再走一步：用 Otsu 阈值按颜色距离，把底盘和图案切开。",
    stages: [
      { title: "alpha 轮廓", detail: "透明边图标的主体就是它的不透明像素" },
      { title: "边界洪泛", detail: "BFS 从四边种子向内扩散，容差相对上一步像素" },
      { title: "Otsu 切割", detail: "碟形轮廓按颜色距离分成底盘与图案" },
    ],
    floodCaption: "洪泛按离散层推进。这是算法真实的推进方式，不是过渡动画。",
    replay: "重播",
  },
  rescue: {
    index: "03",
    kicker: "RESCUE",
    title: "当主体快要融进底盘",
    body: "换上新底盘后，主体颜色可能与底盘过于接近。32 到 48 像素的图标上，仅靠色相对比并不可靠：人眼在高空间频率下对色度的分辨远低于亮度。所以救援沿主体最外圈采样，按你实际看见的合成后颜色判断，色距与亮度差两道门限，超过占比阈值就自动补上描边与阴影。看占比而不是平均值，双峰边缘也藏不住。",
    beats: [
      { key: "DANGER", title: "险情", detail: "主体颜色贴近新底盘，边缘将要消失" },
      { key: "DETECT", title: "检测", detail: "外圈探针逐点判定，按合成后的可见颜色" },
      { key: "RESCUE", title: "救援", detail: "描边沿轮廓画上，软阴影落下，主体回来了" },
    ],
    gauges: { deltaE: "OKLab 色距", deltaL: "亮度差", melt: "熔化占比" },
    caption: "两道门限都不够时才触发。判定在源分辨率进行，任何预览尺寸得到同一个结论。",
  },
  invariant: {
    index: "04",
    kicker: "INVARIANT",
    title: "品牌色永远是品牌色",
    body: "整条管线有一条不可越过的规则：主体像素永不重上色。图标之间的区分交给底盘、描边和阴影。当几颗图标的衍生底盘色相撞在一起时，引擎在色环上把它们确定性地旋转分开，同一图案永远得到同一底盘色。",
    rule: "主体像素永不重上色",
    ruleNote: "写在 compose/field.rs 里的硬规则，任何外观都绕不过它。",
    wheelCaption: "最小间隔约 12 度，最大旋转不超过 18 度。相同输入，相同旋转，没有随机数。",
  },
  color: {
    index: "05",
    kicker: "COLOR",
    title: "颜色是算出来的",
    body: "所有颜色数学运行在 linear-light sRGB 与 OKLab 上。单色外观经 Material 式明暗双色调映射，一张 256 级色调表逐级铺开。连续圆角是对 Figma 开源平滑数学的忠实移植，每个角一段圆弧加两段切向三次曲线。缩放是预乘 alpha 的真实面积平均，在线性光下进行。是组合，不是重新发明。",
    points: [
      { title: "OKLab 全程", detail: "感知均匀空间里做距离、做和谐带、做对比" },
      { title: "256 级双色调", detail: "单色外观的明暗从一条色调表逐级映射" },
      { title: "squircle 数学", detail: "Figma MIT 平滑算法的逐字移植，注明出处" },
      { title: "线性光重采样", detail: "降采样是真实面积平均，升采样 4×4 超采样" },
    ],
  },
  finish: {
    index: "06",
    kicker: "FINISH",
    title: "同一颗图标，三种手感",
    body: "质感不是贴图，是算法。每一种 finish 都是一段对像素的确定性计算，逐颗图标现算。",
    finishes: [
      {
        key: "glass",
        kicker: "GLASS",
        name: "玻璃",
        recipe: ["半透明板体", "菲涅耳高光与边缘折射", "板外一圈落地光晕"],
      },
      {
        key: "pixel",
        kicker: "PIXEL",
        name: "像素",
        recipe: ["网格内线性光平均", "映射到糖果调色板", "沿轮廓补一圈描边"],
      },
      {
        key: "sticker",
        kicker: "STICKER",
        name: "贴纸",
        recipe: ["图形缩小一圈", "轮廓外倒角距离生成白边", "同一距离场落下软影"],
      },
    ],
  },
  guarantee: {
    index: "07",
    kicker: "GUARANTEE",
    title: "可复现的诚实",
    body: "强大要以不出事为前提。退化输入有完整的测试族，缓冲区尺寸全部做溢出检查，渲染尺寸有硬上限，错误走返回码，从不 panic。桌面应用动手之前先给桌面拍快照，一键回到原样。",
    items: [
      { title: "从不 panic", detail: "空输入、零尺寸、溢出尺寸全部返回错误码" },
      { title: "溢出检查", detail: "每一处缓冲区分配都做 checked 乘法" },
      { title: "硬上限", detail: "渲染尺寸超限直接拒绝，不会越界写入" },
      { title: "快照先行", detail: "改桌面之前先备份，还原永远只差一次点击" },
    ],
    receiptsLead: "每个数字都链接到对应的源码。这一页不需要你相信，只需要你点开。",
    receipts: [
      { value: "11,946 行", label: "纯 Rust 像素核心", href: CORE },
      { value: "forbid(unsafe_code)", label: "核心禁用 unsafe", href: `${BLOB}/src/lib.rs` },
      { value: "1,487 颗", label: "字节级校验语料", href: `${BLOB}/tests/parity_determinism.rs` },
      { value: "57 项", label: "核心算法测试", href: `${CORE}/tests` },
      { value: "88 KB", label: "wasm 构建（gzip）", href: `${GH}/tree/main/crates/dm-icon-wasm` },
      { value: "MIT", label: "全部开源", href: `${GH}/blob/main/LICENSE` },
    ],
  },
  playground: {
    index: "08",
    kicker: "LIVE",
    title: "现在，换你来驱动它",
    body: "下面的画布由 dm-icon-wasm 驱动：桌面应用出厂的同一套 Rust 管线，编译成 88 KB 的模块在你的浏览器里运行。每一帧都是当场计算，不是预录视频。挑一颗示例图标，或者放进你自己的图，拖动参数看像素实时回应。",
    badge: "与桌面端字节级一致",
    sampleLabel: "示例图标",
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
    fallbackNote: "当前浏览器无法运行 WASM，以下为预渲染效果图。",
  },
  cta: {
    title: "它已经在等你的桌面了",
    body: "这条管线不是演示品，它就是 DeskMakeover 每天在真实桌面上跑的代码。",
    download: "下载 DeskMakeover",
    github: "去 GitHub 读源码",
  },
};

/** Every /engine/ string that renders in the zh display face (h1/h2/h3). */
export const ENGINE_ZH_DISPLAY_STRINGS: string[] = [
  ENGINE_ZH.hero.title,
  ENGINE_ZH.portrait.title,
  ENGINE_ZH.separate.title,
  ENGINE_ZH.rescue.title,
  ENGINE_ZH.invariant.title,
  ENGINE_ZH.color.title,
  ENGINE_ZH.finish.title,
  ENGINE_ZH.guarantee.title,
  ENGINE_ZH.playground.title,
  ENGINE_ZH.cta.title,
];
