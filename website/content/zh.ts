import type { Dict } from "./types";

export const zh: Dict = {
  locale: "zh",
  htmlLang: "zh-CN",
  meta: {
    title: "DeskMakeover 桌面美颜：Windows 桌面工作室",
    description:
      "DeskMakeover（桌面美颜）给每个 Windows 桌面图标换上九套手工调校的外观，在壁纸上画出整理分区，动手前先完整快照，一键还原到和原来一模一样。免费开源，只在你电脑上运行。",
    ogAlt: "DeskMakeover 桌面美颜：杂乱的 Windows 桌面一键变好看，随时完整还原",
  },
  nav: {
    proof: "实拍对比",
    looks: "九套外观",
    zones: "壁纸分区",
    story: "创作历程",
    engine: "像素引擎",
    download: "下载",
    github: "GitHub",
    langLabel: "English",
    langHref: "/",
  },
  ui: {
    zoomHint: "点击放大",
    zoomClose: "关闭",
  },
  hero: {
    eyebrow: "DeskMakeover · 免费开源",
    title: "桌面美颜",
    tagline: "Windows 桌面工作室",
    sub: "九套手工调校的图标外观，画在壁纸上的整理分区。动手之前先完整快照，一键把一切放回原样。",
    ctaRelease: "下载 Windows 版",
    ctaPending: "即将上线",
    ctaGithub: "GitHub",
    specs: ["WIN 10 1809+ / WIN 11 · 64 位", "MIT 许可证", "全程本地运行", "快照级还原"],
    sceneCaption: "真实桌面，真实还原，实时渲染。",
    sceneAlt: "一台 3D 显示器里，真实的 Windows 桌面正在被 DeskMakeover 重新设计",
  },
  proof: {
    index: "01",
    kicker: "实拍对比",
    title: "真实像素的前后对比",
    body: "没有效果图。两帧是同一张桌面的应用内实拍：126 个图标一键换装，一键换回。拖动中线看看。",
    dragHint: "拖动",
    altBefore: "默认的 Windows 桌面，挤满风格混乱的图标",
    altAfter: "同一张桌面经过 DeskMakeover 处理，每个图标都换上 Squircle 外观",
  },
  looks: {
    index: "02",
    kicker: "九套外观",
    title: "九套外观，逐一手调",
    body: "每一套都在真实的、堆满图标的桌面上调校，不是演示文件夹。选一套点一下，每个图标都换上。形状、上色、底板、质感和快捷标记还能各自单独调整。",
    altPrefix: "整张 Windows 桌面换上这套外观：",
    // 名称与描述与软件内一致（src/lib/i18n/zh-hans.ts 的 Preset_* 键）
    styles: [
      { key: "squircle", name: "方圆", tagline: "一枚方圆，各有其形" },
      { key: "blueprint", name: "蓝图", tagline: "一整套工程蓝图" },
      { key: "pixel-era", name: "像素纪元", tagline: "回到八比特的下午" },
      { key: "gleam", name: "浮光", tagline: "原样的图标，掠过一层光" },
      { key: "glaze", name: "釉光", tagline: "上过釉的冷瓷面" },
      { key: "die-cut", name: "随形贴", tagline: "沿轮廓裁开的贴纸包" },
      { key: "porthole", name: "圆窗", tagline: "程序是圆窗，文件各安其位" },
      { key: "scrapbook", name: "拼贴手帐", tagline: "一页随手拼贴的手帐" },
      { key: "creekstone", name: "溪石", tagline: "溪水磨圆的石头" },
    ],
  },
  zones: {
    index: "03",
    kicker: "壁纸分区",
    title: "画在壁纸上的整理分区",
    body: "半透明面板直接生长在壁纸里，桌面从此有了房间：应用放这边，工作放那边，手头项目摆正中间。",
    points: [
      {
        title: "模板起步，随手可画",
        body: "从工作台、四象限这类现成布局开始，也可以自己画。五种材质，四种标题样式。",
      },
      {
        title: "烙进壁纸，不是悬浮层",
        body: "分区直接渲染进壁纸图片，后台不需要常驻任何程序来维持它们。",
      },
      {
        title: "一键全部移除",
        body: "原来的壁纸原样保存，想回去随时回去。",
      },
    ],
    imgAlt: "DeskMakeover 分区编辑器：真实壁纸上叠着三个半透明分区，标题分别是应用、工作和进行中",
  },
  studio: {
    index: "04",
    kicker: "工作室",
    title: "这是工作室，不是设置页",
    body: "左侧的实时镜像就是你的真实桌面，右侧每个控件都即时响应。任何时候按住空格，就能和原样对比。",
    points: [
      {
        title: "按类型分别造型",
        body: "程序、文件夹、文件可以各穿各的变体。",
      },
      {
        title: "撤销、重做、重读",
        body: "完整历史记录，还能一键重读真实桌面：图标、排列、壁纸。",
      },
      {
        title: "保存并分享风格",
        body: "把组合存进风格库，导出成文件，也能导入朋友的风格包。",
      },
    ],
    imgAlt: "DeskMakeover 图标工作室：实时桌面镜像里 126 个图标已换装，右侧是形状、主体、底板、质感和快捷标记控件",
  },
  engineBand: {
    kicker: "引擎",
    title: "不是滤镜，是读懂图标的引擎",
    body: "每一套外观背后是一条自研确定性像素管线。它先读懂每颗图标再动手，浏览器与桌面端输出字节级一致的像素。",
    cta: "看它如何工作",
  },
  download: {
    index: "05",
    kicker: "下载",
    title: "获取 DeskMakeover",
    body: "下载、双击、完成。按用户安装，全程在你电脑上运行。没有账号，没有遥测，没有上传。",
    ctaRelease: "下载 Windows 版",
    ctaPending: "即将上线",
    watchGithub: "在 GitHub 关注",
    pendingNote: "第一个安装包正在路上。关注仓库，上线那一刻 GitHub 会通知你。",
    smartscreenLead: "Windows 可能弹出蓝色的「Windows 已保护你的电脑」，这不是病毒警告。",
    smartscreenDetail:
      "只是 Windows 对下载人数还不多的新软件比较谨慎。点「更多信息」，再点「仍要运行」。DeskMakeover 完全开源，任何人都能读到它到底做了什么。",
    requirements: "WIN 10 1809+ / WIN 11 · 64 位",
  },
  downloadModal: {
    title: "下载 DeskMakeover",
    device: {
      "win-x64": "你的设备是 64 位 Windows，可直接安装",
      "win-unknown": "你的设备是 Windows，安装包为 64 位版",
      "win-arm": "你的设备可能是 ARM 版 Windows。目前只有 x64 安装包，仍可尝试安装，性能可能受影响",
      "win-32": "这台 Windows 是 32 位的。DeskMakeover 需要 64 位的 Windows 10（1809 及以上）或 Windows 11，仍可下载留作备用",
      "win-old": "这个 Windows 版本偏旧。DeskMakeover 需要 Windows 10（1809 及以上）或 Windows 11，仍可下载留作备用",
      "desktop-other": "DeskMakeover 只支持 Windows。可以先下载安装包，回头拷到 Windows 电脑安装",
      mobile: "这是 Windows 桌面软件，手机装不了。复制下载链接，发到电脑上打开",
    },
    primaryCta: "下载安装包",
    mobileCopyCta: "复制下载链接",
    mobileCopied: "已复制，去电脑上粘贴打开",
    mobileStillDownload: "仍要在手机上下载安装包",
    viaGithub: "GitHub 官方直链",
    mirrorsLead: "大陆下载慢或打不开，用加速线路",
    mirrorNote: "加速线路由第三方公益代理提供，与本项目无关。下载完核对文件大小应为 {size} MB。",
    historyLabel: "历史版本",
    releaseNotes: "发布说明",
    smartscreenStarted:
      "下载应该已经开始。安装时如果 Windows 弹出蓝色的「Windows 已保护你的电脑」，点「更多信息」，再点「仍要运行」。这不是病毒警告，只是新软件下载人数还不多。",
    close: "关闭",
  },
  faq: {
    kicker: "常见问题",
    title: "大家常问的问题",
    items: [
      {
        q: "会不会把我电脑搞坏，或者回不去了？",
        a: "不会。DeskMakeover（桌面美颜）动手之前先给 Windows 桌面拍快照，图标、快捷方式箭头、壁纸都在内，每处改动都能一键还原到和原来一模一样。放心折腾。",
      },
      {
        q: "会拖慢电脑吗？",
        a: "感觉不到。DeskMakeover 的重活只在你点应用的那一下发生，其余时间它安静待着，Windows 重置桌面时再帮你把外观补回来。",
      },
      {
        q: "真的免费吗？有什么套路？",
        a: "DeskMakeover 完全免费，MIT 许可证，源码就在 GitHub。它只在你机器上运行：没有账号，没有遥测，什么都不上传。",
      },
      {
        q: "macOS 或 Linux 能用吗？",
        a: "不能。DeskMakeover 面向 Windows 10（1809 及以上）和 Windows 11，64 位。",
      },
    ],
  },
  footer: {
    tagline: "改成你的样子，也随时改回去。",
    license: "MIT 许可证，永久免费。",
    links: [
      { label: "GitHub", href: "https://github.com/nicepkg/deskmakeover" },
      { label: "发布页", href: "https://github.com/nicepkg/deskmakeover/releases" },
      { label: "开源许可", href: "https://github.com/nicepkg/deskmakeover/blob/main/LICENSE" },
    ],
  },
};
