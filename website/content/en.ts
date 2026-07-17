import type { Dict } from "./types";

export const en: Dict = {
  locale: "en",
  htmlLang: "en",
  meta: {
    title: "DeskMakeover: the desktop studio for Windows",
    description:
      "DeskMakeover restyles every Windows desktop icon with nine hand-tuned looks, draws organizer zones on your wallpaper, and snapshots everything first so one click restores the exact original. Free, open source, local-only.",
    ogAlt: "DeskMakeover: a cluttered Windows desktop turned beautiful, fully reversible",
  },
  nav: {
    proof: "Proof",
    looks: "Looks",
    zones: "Zones",
    story: "Story",
    download: "Download",
    github: "GitHub",
    langLabel: "中文",
    langHref: "/zh/",
  },
  ui: {
    zoomHint: "Click to enlarge",
    zoomClose: "Close",
  },
  hero: {
    eyebrow: "Free · Open source · 桌面美颜",
    title: "DeskMakeover",
    tagline: "The desktop studio for Windows.",
    sub: "Nine hand-tuned looks for every icon. Organizer zones drawn on your wallpaper. A snapshot before anything changes, so one click puts it all back.",
    ctaRelease: "Download for Windows",
    ctaPending: "Coming soon",
    ctaGithub: "GitHub",
    specs: ["WIN 10 1809+ / WIN 11 · 64-BIT", "MIT LICENSE", "LOCAL-ONLY", "SNAPSHOT RESTORE"],
    sceneCaption: "Real desktop. Real restore. Rendered live.",
    sceneAlt: "A 3D monitor showing a real Windows desktop being restyled by DeskMakeover",
  },
  proof: {
    index: "01",
    kicker: "Proof",
    title: "Real pixels, before and after",
    body: "No mockups. Both frames are the same desktop, shot from the app: 126 icons restyled in one click, and one click back. Drag the line.",
    dragHint: "Drag",
    altBefore: "A default Windows desktop crowded with mismatched icons",
    altAfter: "The same desktop after DeskMakeover: every icon in the Squircle look",
  },
  looks: {
    index: "02",
    kicker: "Looks",
    title: "Nine looks, hand-tuned",
    body: "Each look was tuned on a real, crowded desktop, not a demo folder. Pick one and every icon puts it on. Shape, coloring, plate, finish, and shortcut mark stay adjustable on their own.",
    altPrefix: "A full Windows desktop wearing the look:",
    // names + taglines mirror the app's own preset strings (src/lib/i18n/en.ts)
    styles: [
      { key: "squircle", name: "Squircle", tagline: "One squircle, every type its own shape" },
      { key: "blueprint", name: "Blueprint", tagline: "A full set of engineering drawings" },
      { key: "pixel-era", name: "Pixel Era", tagline: "An afternoon back in 8-bit" },
      { key: "gleam", name: "Gleam", tagline: "Icons as they are, brushed with light" },
      { key: "glaze", name: "Glaze", tagline: "Cool porcelain under a glaze" },
      { key: "die-cut", name: "Die-Cut", tagline: "Stickers cut along every outline" },
      { key: "porthole", name: "Porthole", tagline: "Round apps; files keep their places" },
      { key: "scrapbook", name: "Scrapbook", tagline: "A page pasted together by hand" },
      { key: "creekstone", name: "Creekstone", tagline: "Stones rounded by the stream" },
    ],
  },
  zones: {
    index: "03",
    kicker: "Zones",
    title: "Zones drawn on your wallpaper",
    body: "Translucent panels that live inside the wallpaper itself, so your desktop gets rooms: apps here, work there, current projects front and center.",
    points: [
      {
        title: "Templates or freehand",
        body: "Start from a ready layout like Workbench or Quadrants, or draw your own. Five materials, four title styles.",
      },
      {
        title: "Baked in, not floating",
        body: "Zones render into the wallpaper image. Nothing runs in the background to keep them there.",
      },
      {
        title: "One click removes them",
        body: "Your original wallpaper is kept as shot. Bring it back whenever.",
      },
    ],
    imgAlt: "The DeskMakeover zones editor: three translucent zones titled Apps, Work and Doing over a real wallpaper",
  },
  studio: {
    index: "04",
    kicker: "Studio",
    title: "A studio, not a settings page",
    body: "The live desktop mirror on the left is your actual desktop, and every control on the right answers instantly. Hold space to compare with the original at any time.",
    points: [
      {
        title: "Per-type styling",
        body: "Programs, folders, and files can each wear their own variant.",
      },
      {
        title: "Undo, redo, re-read",
        body: "Full history, plus a one-click re-read of the real desktop: icons, arrangement, wallpaper.",
      },
      {
        title: "Save and share styles",
        body: "Keep your combinations in the style library, export them as files, import packs from friends.",
      },
    ],
    imgAlt: "The DeskMakeover icons studio: a live desktop mirror with 126 styled icons and shape, subject, plate, filter and shortcut controls",
  },
  download: {
    index: "05",
    kicker: "Download",
    title: "Get DeskMakeover",
    body: "Download, double-click, done. Installs per-user, runs entirely on your PC. No account, no telemetry, no uploads.",
    ctaRelease: "Download for Windows",
    ctaPending: "Coming soon",
    watchGithub: "Watch on GitHub",
    pendingNote: "The first installer is on the way. Watch the repo and GitHub notifies you the moment it lands.",
    smartscreenLead: "Windows may show a blue “Windows protected your PC” screen. That is not a virus warning.",
    smartscreenDetail:
      "It is Windows being cautious with newer software few people have downloaded yet. Click More info, then Run anyway. DeskMakeover is open source, so anyone can read exactly what it does.",
    requirements: "WIN 10 1809+ / WIN 11 · 64-BIT",
  },
  faq: {
    kicker: "FAQ",
    title: "Questions people ask",
    items: [
      {
        q: "Can DeskMakeover break my PC, or leave me stuck?",
        a: "No. DeskMakeover snapshots the Windows desktop (icons, shortcut arrows, wallpaper) before changing anything, and every change is one click to undo, back to exactly how it was. Experiment freely.",
      },
      {
        q: "Will it slow my computer down?",
        a: "Not noticeably. DeskMakeover does the heavy work once when you hit apply. The rest of the time it sits quietly and puts your look back if Windows resets the desktop.",
      },
      {
        q: "Is it really free? What is the catch?",
        a: "DeskMakeover is free and MIT-licensed, with the source on GitHub. It runs only on your machine: no account, no telemetry, nothing uploaded.",
      },
      {
        q: "Does it work on macOS or Linux?",
        a: "No. DeskMakeover is built for Windows 10 (1809 or newer) and Windows 11, 64-bit.",
      },
    ],
  },
  footer: {
    tagline: "Make it yours. Take it back.",
    license: "MIT licensed, free forever.",
    links: [
      { label: "GitHub", href: "https://github.com/nicepkg/deskmakeover" },
      { label: "Releases", href: "https://github.com/nicepkg/deskmakeover/releases" },
      { label: "License", href: "https://github.com/nicepkg/deskmakeover/blob/main/LICENSE" },
    ],
  },
};
