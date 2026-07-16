import type { Dict } from "./types";

export const en: Dict = {
  locale: "en",
  htmlLang: "en",
  meta: {
    title: "DeskMakeover: make your Windows desktop beautiful in one click",
    description:
      "DeskMakeover restyles your Windows desktop icons, adds wallpaper zones, and backs everything up first so one click restores the exact original. Free, open source, local-only.",
    ogAlt: "DeskMakeover: a cluttered Windows desktop turned beautiful, fully reversible",
  },
  nav: {
    looks: "Looks",
    features: "Features",
    faq: "FAQ",
    download: "Download",
    github: "GitHub",
    langLabel: "中文",
    langHref: "/zh/",
  },
  hero: {
    headline1: "One click makes your desktop beautiful.",
    headline2: "One click puts it back.",
    sub: "Hand-tuned icon looks, wallpaper zones, and a full backup before anything changes.",
    trust: "Free. Open source. Runs only on your PC.",
    statusRefreshed: "Refreshed",
    ctaRelease: "Download for Windows",
    ctaPending: "Coming soon",
    ctaSecondary: "See the nine looks",
    putBack: "Put it back",
    beautify: "Beautify",
    stageBefore: "Your desktop today",
    stageAfter: "Your desktop after",
    imgAlt: "A cluttered default Windows desktop transformed into clean squircle icons, then restored",
  },
  promise: {
    title: "You can always go back",
    lead: "DeskMakeover photographs your desktop before touching it. Try the toggle: that is the real promise, running on real pixels.",
    toggleStyled: "Styled",
    toggleOriginal: "Original",
    items: [
      {
        title: "It backs everything up first",
        body: "Like taking a photo of your desktop: icons, arrows, wallpaper, kept exactly as they were.",
      },
      {
        title: "One-click restore",
        body: "Back to just how it was. Not close. Exact.",
      },
      {
        title: "Runs only on your computer",
        body: "No account, no upload, nothing online. Your desktop never leaves your PC.",
      },
      {
        title: "Nothing technical to touch",
        body: "Pick, preview, click. That is the whole job.",
      },
    ],
  },
  looks: {
    title: "Nine looks built in",
    sub: "Each one hand-tuned on a real desktop. Pick one, click once, every icon puts it on.",
    specimenAlt: "One folder icon rendered in all nine DeskMakeover styles",
    specimenCaption: "The same folder, nine ways.",
    presets: [
      { img: "preset-squircle", name: "Squircle", tagline: "continuous corners" },
      { img: "preset-blueprint", name: "Blueprint", tagline: "monochrome ink" },
      { img: "preset-pixel-era", name: "Pixel Era", tagline: "8-bit afternoon" },
      { img: "preset-gleam", name: "Gleam", tagline: "brushed with light" },
      { img: "preset-glaze", name: "Glaze", tagline: "cool porcelain" },
      { img: "preset-die-cut", name: "Die-Cut", tagline: "sticker outlines" },
      { img: "preset-porthole", name: "Porthole", tagline: "clean circles" },
      { img: "preset-scrapbook", name: "Scrapbook", tagline: "pasted by hand" },
      { img: "preset-creekstone", name: "Creekstone", tagline: "river-worn stone" },
    ],
  },
  customize: {
    title: "Make every part yours",
    body: "The nine are a starting point. Everything below is one panel away.",
    rows: [
      { title: "Eleven shapes, five axes", body: "Shape, coloring, plate, finish, and shortcut mark, each adjustable on its own." },
      { title: "Per-type looks", body: "Apps, folders, and plain files can each wear their own style." },
      { title: "Compare and undo freely", body: "Hold to see the original, right-click to restyle one icon, undo and redo anytime." },
      { title: "Save it. Share it.", body: "Keep combinations as your own styles, export them as files, import packs from friends." },
      { title: "Zones on your wallpaper", body: "Draw translucent organizer zones, five materials, four title styles, ready-made layouts. One click removes them." },
      { title: "The shortcut arrow, your call", body: "Swap it for a cleaner mark or remove it. Backed up first, one click brings it back." },
    ],
    imgAlt: "The DeskMakeover icons tab: the live desktop mirror on the left, shape, coloring, plate and finish controls on the right",
  },
  download: {
    title: "Get DeskMakeover",
    body: "Download, double-click, done. It installs just for your account, on Windows 10 (1809 or newer) and Windows 11, 64-bit.",
    ctaRelease: "Download for Windows",
    ctaPending: "Coming soon",
    watchGithub: "Watch on GitHub",
    pendingNote: "The first installer is on the way. Watch the repo and GitHub tells you the moment it lands.",
    smartscreenLead: "Windows may show a blue “Windows protected your PC” screen. That is not a virus warning.",
    smartscreenDetail: "It is Windows being cautious with newer software that few people have downloaded yet. Click More info, then Run anyway. DeskMakeover is open source, so anyone can read exactly what it does.",
    requirements: "Windows 10 (1809+) and Windows 11, 64-bit",
    nonWindowsNote: "DeskMakeover runs on Windows. Browsing from your phone or a Mac? Send yourself the link for later.",
    copyLink: "Copy link",
    copied: "Copied",
    mailLink: "Email me the link",
    mailSubject: "DeskMakeover for Windows",
    mailBody: "Open this on your Windows PC: https://dm.nicepkg.cn",
  },
  beta: {
    title: "Straight talk",
    body: "This is beta and there will be rough edges. It styles your icons and wallpaper zones; it is not a full theme for all of Windows. Do not expect one-click perfection, but whatever you try, one click takes it back.",
  },
  faq: {
    title: "Questions people ask",
    items: [
      {
        q: "Can DeskMakeover break my PC, or leave me stuck?",
        a: "No. DeskMakeover backs up your Windows desktop before changing anything, and every change is one click to undo, back to exactly how it was. Experiment freely.",
      },
      {
        q: "Will it slow my computer down?",
        a: "Not noticeably. The heavy work happens once when you hit apply; the rest of the time DeskMakeover sits quietly, and puts your look back if Windows resets the desktop.",
      },
      {
        q: "Will my new look survive a restart?",
        a: "Yes. Restyled icons are real image files, so they persist across restarts. If a big Windows update resets some settings, re-apply with one click, or restore.",
      },
      {
        q: "Is DeskMakeover free? Does anything get uploaded?",
        a: "DeskMakeover is completely free and open source (MIT). It runs only on your computer: no account, no telemetry, nothing sent anywhere.",
      },
      {
        q: "Why does Windows show a blue “protected your PC” screen?",
        a: "That is Microsoft SmartScreen being cautious with new software, not a virus warning. Click More info, then Run anyway. The source code is public, so anyone can verify what the app does.",
      },
      {
        q: "Which Windows versions are supported?",
        a: "DeskMakeover supports Windows 10 (version 1809 or newer) and Windows 11, 64-bit. It does not run on Windows 7, Windows 8, or macOS.",
      },
      {
        q: "What happens when I uninstall?",
        a: "Your desktop is restored from the backup, so icons, arrows, and wallpaper return to how they were before DeskMakeover touched them.",
      },
      {
        q: "How is this different from a Windows theme?",
        a: "A theme reskins system colors. DeskMakeover restyles the actual desktop: your icons, your wallpaper zones, the shortcut arrows. It does not theme the rest of Windows, and it never changes anything it cannot restore.",
      },
      {
        q: "What is the current status?",
        a: "DeskMakeover is in beta and there will be rough edges. It styles icons and wallpaper zones; it is not a full Windows theme. Whatever you try, one click takes it back.",
      },
      {
        q: "I am not great with computers. Can I use it?",
        a: "Yes. Pick a look, check the preview, click apply. No commands, no technical settings, and restore is always one click away.",
      },
    ],
  },
  footer: {
    tagline: "Made your desktop nicer?",
    star: "Give it a star",
    starLink: "https://github.com/nicepkg/deskmakeover",
    license: "MIT © 2026 Jinming Yang. Free and open source.",
    links: [
      { label: "GitHub", href: "https://github.com/nicepkg/deskmakeover" },
      { label: "Releases", href: "https://github.com/nicepkg/deskmakeover/releases" },
      { label: "Issues", href: "https://github.com/nicepkg/deskmakeover/issues" },
    ],
  },
};
