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
    download: "下载",
    github: "GitHub",
    langLabel: "English",
    langHref: "/",
  },
  hero: {
    eyebrow: "DeskMakeover 桌面美颜",
    title: "Windows\n桌面工作室",
    sub: "九套手工调校的图标外观，画在壁纸上的整理分区。动手之前先完整快照，一键把一切放回原样。",
    ctaRelease: "下载 Windows 版",
    ctaPending: "即将上线",
    ctaGithub: "GitHub",
    specs: ["WIN 10 1809+ / WIN 11 · 64 位", "MIT 许可证", "全程本地运行", "快照级还原"],
    sceneCaption: "真实桌面，真实还原，实时渲染。",
    sceneAlt: "一台 3D 显示器里，真实的 Windows 桌面正在被 DeskMakeover 重新设计",
    sceneBefore: "改造前",
    sceneAfter: "改造后",
  },
  proof: {
    index: "01",
    kicker: "实拍对比",
    title: "真实像素的前后对比",
    body: "没有效果图。两帧是同一张桌面的应用内实拍：126 个图标一键换装，一键换回。拖动中线看看。",
    beforeLabel: "改造前",
    afterLabel: "改造后",
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
    styles: [
      { key: "squircle", name: "Squircle", tagline: "连续圆角" },
      { key: "blueprint", name: "Blueprint", tagline: "单色墨线" },
      { key: "pixel-era", name: "Pixel Era", tagline: "八位机午后" },
      { key: "gleam", name: "Gleam", tagline: "拉丝微光" },
      { key: "glaze", name: "Glaze", tagline: "冷调釉瓷" },
      { key: "die-cut", name: "Die-Cut", tagline: "贴纸描边" },
      { key: "porthole", name: "Porthole", tagline: "干净圆窗" },
      { key: "scrapbook", name: "Scrapbook", tagline: "手帐拼贴" },
      { key: "creekstone", name: "Creekstone", tagline: "溪石圆润" },
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
  faq: {
    title: "大家常问的问题",
    items: [
      {
        q: "会不会把我电脑搞坏，或者回不去了？",
        a: "不会。动手之前它先给桌面拍快照，每处改动都能一键撤销，还原到和原来一模一样。放心折腾。",
      },
      {
        q: "会拖慢电脑吗？",
        a: "感觉不到。重活只在你点应用的那一下发生，其余时间它安静待着，Windows 重置桌面时再帮你把外观补回来。",
      },
      {
        q: "真的免费吗？有什么套路？",
        a: "免费，MIT 许可证，源码就在 GitHub。它只在你机器上运行：没有账号，没有遥测，什么都不上传。",
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
