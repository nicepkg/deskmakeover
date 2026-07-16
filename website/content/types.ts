export type Locale = "en" | "zh";

export interface PresetEntry {
  /** image manifest key, e.g. "preset-squircle" */
  img: string;
  name: string;
  tagline: string;
}

export interface FaqItem {
  q: string;
  a: string;
}

export interface Dict {
  locale: Locale;
  htmlLang: string;
  meta: {
    title: string;
    description: string;
    ogAlt: string;
  };
  nav: {
    looks: string;
    features: string;
    faq: string;
    download: string;
    github: string;
    langLabel: string;
    langHref: string;
  };
  hero: {
    headline1: string;
    headline2: string;
    sub: string;
    ctaRelease: string;
    ctaPending: string;
    ctaSecondary: string;
    putBack: string;
    beautify: string;
    stageBefore: string;
    stageAfter: string;
    imgAlt: string;
  };
  promise: {
    title: string;
    items: { title: string; body: string }[];
  };
  looks: {
    title: string;
    sub: string;
    specimenAlt: string;
    specimenCaption: string;
    presets: PresetEntry[];
  };
  customize: {
    title: string;
    body: string;
    bullets: string[];
    imgAlt: string;
    caption: string;
  };
  zones: {
    title: string;
    body: string;
    imgAlt: string;
    caption: string;
    arrowTitle: string;
    arrowBody: string;
  };
  studio: {
    title: string;
    body: string;
    imgAlt: string;
    caption: string;
    packTitle: string;
    packBody: string;
    packImgAlt: string;
  };
  download: {
    title: string;
    body: string;
    ctaRelease: string;
    ctaPending: string;
    pendingNote: string;
    smartscreenLead: string;
    smartscreenDetail: string;
    requirements: string;
    nonWindowsNote: string;
    copyLink: string;
    copied: string;
    mailLink: string;
    mailSubject: string;
    mailBody: string;
  };
  beta: {
    title: string;
    body: string;
  };
  faq: {
    title: string;
    items: FaqItem[];
  };
  footer: {
    tagline: string;
    star: string;
    starLink: string;
    license: string;
    links: { label: string; href: string }[];
  };
}
