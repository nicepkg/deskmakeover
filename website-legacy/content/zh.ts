import type { Dict } from "./types";

export const zh: Dict = {
  locale: "zh",
  htmlLang: "zh-CN",
  meta: {
    title: "DeskMakeover 桌面美颜：一键把 Windows 桌面变好看",
    description:
      "DeskMakeover（桌面美颜）一键给 Windows 桌面图标换上手工调校的外观，在壁纸上圈出分区，动手前先完整备份，一键还原到和原来一模一样。免费开源，只在你电脑上跑。",
    ogAlt: "DeskMakeover 桌面美颜：杂乱的 Windows 桌面一键变好看，随时完整还原",
  },
  nav: {
    looks: "外观",
    features: "功能",
    faq: "常见问题",
    download: "下载",
    github: "GitHub",
    langLabel: "English",
    langHref: "/",
  },
  hero: {
    headline1: "一键把桌面变好看。",
    headline2: "一键变回原样。",
    sub: "手工调校的图标外观，壁纸分区，动手前先完整备份。",
    trust: "免费。开源。只在你电脑上跑。",
    statusRefreshed: "已焕新",
    ctaRelease: "下载 Windows 版",
    ctaPending: "即将上线",
    ctaSecondary: "先看九套外观",
    putBack: "变回原样",
    beautify: "一键美颜",
    stageBefore: "现在的桌面",
    stageAfter: "美颜后的桌面",
    imgAlt: "杂乱的 Windows 默认桌面一键变成整齐的方圆图标，然后完整还原",
  },
  promise: {
    title: "随时能变回来",
    lead: "DeskMakeover 动手前先给桌面拍照。试试这个开关，这就是承诺本身，跑在真实像素上。",
    toggleStyled: "美颜后",
    toggleOriginal: "原样",
    items: [
      {
        title: "动手前先完整备份",
        body: "像先给桌面拍张照片：图标、箭头、壁纸，原封不动留好。",
      },
      {
        title: "一键还原",
        body: "和你没动过时一模一样。不是差不多，是一模一样。",
      },
      {
        title: "只在你电脑上跑",
        body: "不注册、不联网、不上传。你的桌面不会离开你的电脑。",
      },
      {
        title: "不用碰任何技术设置",
        body: "挑、预览、点一下，就这三件事。",
      },
    ],
  },
  looks: {
    title: "九套现成外观，一键上身",
    sub: "每一套都在真实桌面上手工调过。挑一套点一下，所有图标就换上。",
    specimenAlt: "同一个文件夹图标在九套风格下的样子",
    specimenCaption: "同一个文件夹，九种穿法。",
    presets: [
      { img: "preset-squircle", name: "方圆", tagline: "圆角连续，顺滑过渡" },
      { img: "preset-blueprint", name: "蓝图", tagline: "一整套工程蓝图" },
      { img: "preset-pixel-era", name: "像素纪元", tagline: "回到八比特的下午" },
      { img: "preset-gleam", name: "浮光", tagline: "原样的图标，掠过一层光" },
      { img: "preset-glaze", name: "釉光", tagline: "上过釉的冷瓷面" },
      { img: "preset-die-cut", name: "随形贴", tagline: "沿轮廓裁开的贴纸" },
      { img: "preset-porthole", name: "圆窗", tagline: "干净利落的圆形" },
      { img: "preset-scrapbook", name: "拼贴手帐", tagline: "随手拼贴的一页" },
      { img: "preset-creekstone", name: "溪石", tagline: "溪水磨圆的石头" },
    ],
  },
  customize: {
    title: "每一处都能自己调",
    body: "九套只是起点。下面每一件事，都只隔一个面板。",
    rows: [
      { title: "十一种形状，五个轴", body: "形状、配色、底板、质感、快捷方式标记，每个轴单独调。" },
      { title: "分类型换装", body: "程序、文件夹、普通文件，可以各用各的样子。" },
      { title: "随时对比与撤销", body: "按住看原样，右键单独改一个图标，撤销重做随便用。" },
      { title: "存成风格，还能分享", body: "调顺眼了存成自己的风格，导出发给朋友，导入别人的风格包直接用。" },
      { title: "壁纸上的收纳分区", body: "画几块半透明分区，五种材质、四种标题样式，布局模板一键铺好，一键撤掉。" },
      { title: "快捷方式小箭头，随你", body: "换成精致小标记或者干脆去掉。改之前照样备份，一键装回。" },
    ],
    imgAlt: "DeskMakeover 图标标签页：左边实时桌面镜像，右边形状、配色、底板、质感控制面板",
  },
  download: {
    title: "下载 DeskMakeover",
    body: "下载、双击、开用，只装给你自己的账户。支持 Windows 10（1809 及以上）和 Windows 11，64 位。",
    ctaRelease: "下载 Windows 版",
    ctaPending: "即将上线",
    watchGithub: "去 GitHub 关注",
    pendingNote: "首个安装包正在准备。Watch 仓库，发布那一刻 GitHub 会通知你。",
    smartscreenLead: "Windows 可能会弹蓝色的「Windows 已保护你的电脑」。这不是病毒警告。",
    smartscreenDetail: "只是 Windows 对下载人数还不多的新软件比较谨慎。点「更多信息」，再点「仍要运行」即可。DeskMakeover 是开源软件，任何人都能查看它到底做了什么。",
    requirements: "Windows 10（1809 及以上）与 Windows 11，64 位",
    nonWindowsNote: "DeskMakeover 运行在 Windows 上。现在用手机或 Mac 在看？把链接发给自己，回头在电脑上打开。",
    copyLink: "复制链接",
    copied: "已复制",
    mailLink: "发到我的邮箱",
    mailSubject: "DeskMakeover 桌面美颜",
    mailBody: "在你的 Windows 电脑上打开：https://dm.nicepkg.cn/zh/",
  },
  beta: {
    title: "说句实在话",
    body: "现在还是 Beta，会有毛边。它只负责桌面图标和壁纸分区，不给整个 Windows 换主题。别指望一键完美，但怎么调都能一键收回，放心试。",
  },
  faq: {
    title: "常见问题",
    items: [
      {
        q: "会不会把电脑弄坏，回不去？",
        a: "不会。DeskMakeover 动手前先完整备份 Windows 桌面，任何改动一键还原到没动过的样子。这是最优先保证的事，随便试。",
      },
      {
        q: "会不会拖慢电脑？",
        a: "不会让你察觉。重活只在点「应用」那一下干一次，平时 DeskMakeover 安安静静待着；Windows 把桌面刷回默认时它会帮你补回来。",
      },
      {
        q: "重启之后还在吗？",
        a: "在。换好的图标是真实图片文件，重启不丢。Windows 大更新重置了设置的话，一键重新应用或还原。",
      },
      {
        q: "要花钱吗？会上传我的东西吗？",
        a: "DeskMakeover 完全免费，开源（MIT 协议）。只在你电脑上跑，不注册账号，什么都不上传。",
      },
      {
        q: "为什么 Windows 弹「已保护你的电脑」蓝屏？",
        a: "那是微软 SmartScreen 对新软件的谨慎提示，不是病毒警告。点「更多信息」，再点「仍要运行」。源代码公开，任何人都能验证这个应用做了什么。",
      },
      {
        q: "支持哪些 Windows 版本？",
        a: "DeskMakeover 支持 Windows 10（1809 及以上）和 Windows 11，64 位。不支持 Windows 7、Windows 8 和 macOS。",
      },
      {
        q: "卸载之后桌面会怎样？",
        a: "会从备份还原：图标、箭头、壁纸都回到 DeskMakeover 动手之前的样子。",
      },
      {
        q: "和换主题软件有什么区别？",
        a: "主题软件换的是系统配色。DeskMakeover 改的是桌面本身：你的图标、壁纸分区、快捷方式箭头。它不动 Windows 其他部分，也绝不改任何还原不了的东西。",
      },
      {
        q: "现在是什么状态？",
        a: "DeskMakeover 还在 Beta，会有毛边。它只负责桌面图标和壁纸分区，不给整个 Windows 换主题。但怎么调都能一键收回，放心试。",
      },
      {
        q: "我不太懂电脑，能用明白吗？",
        a: "能。挑一套、看预览、点一下，全程没有命令和技术设置。挑错了随时一键还原。",
      },
    ],
  },
  footer: {
    tagline: "桌面变好看了？",
    star: "给个 Star",
    starLink: "https://github.com/nicepkg/deskmakeover",
    license: "MIT © 2026 Jinming Yang。免费开源。",
    links: [
      { label: "GitHub", href: "https://github.com/nicepkg/deskmakeover" },
      { label: "Releases", href: "https://github.com/nicepkg/deskmakeover/releases" },
      { label: "Issues", href: "https://github.com/nicepkg/deskmakeover/issues" },
    ],
  },
};
