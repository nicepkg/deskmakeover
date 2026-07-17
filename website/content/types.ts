import type { LocaleCode } from "@/lib/locales";

/** Alias of the registry's LocaleCode — lib/locales.ts is the source of truth. */
export type Locale = LocaleCode;

export interface StyleEntry {
  /** style key matching assets-src/desktop/<key>.webp */
  key: string;
  name: string;
  tagline: string;
}

export interface PointEntry {
  title: string;
  body: string;
}

export interface FaqItem {
  q: string;
  a: string;
}

export interface SectionHead {
  /** mono index label, e.g. "01" */
  index: string;
  /** mono uppercase kicker, e.g. "PROOF" */
  kicker: string;
  title: string;
  body: string;
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
    proof: string;
    looks: string;
    zones: string;
    /** the /story/ making-of page entry */
    story: string;
    download: string;
    github: string;
    langLabel: string;
    langHref: string;
  };
  ui: {
    zoomHint: string;
    zoomClose: string;
  };
  hero: {
    eyebrow: string;
    /** the product name — always the largest text on screen */
    title: string;
    /** category line under the name */
    tagline: string;
    sub: string;
    ctaRelease: string;
    ctaPending: string;
    ctaGithub: string;
    /** mono spec strip under the fold line */
    specs: string[];
    sceneCaption: string;
    sceneAlt: string;
  };
  proof: SectionHead & {
    dragHint: string;
    altBefore: string;
    altAfter: string;
  };
  looks: SectionHead & {
    altPrefix: string;
    styles: StyleEntry[];
  };
  zones: SectionHead & {
    points: PointEntry[];
    imgAlt: string;
  };
  studio: SectionHead & {
    points: PointEntry[];
    imgAlt: string;
  };
  download: SectionHead & {
    ctaRelease: string;
    ctaPending: string;
    watchGithub: string;
    pendingNote: string;
    smartscreenLead: string;
    smartscreenDetail: string;
    requirements: string;
  };
  /** the click-to-download dialog (only rendered once a release exists) */
  downloadModal: {
    title: string;
    /** advisory device-support line per detected OS; unknown renders nothing */
    device: {
      "win-x64": string;
      "win-unknown": string;
      "win-arm": string;
      "win-32": string;
      "win-old": string;
      "desktop-other": string;
      mobile: string;
    };
    primaryCta: string;
    /** mobile-first action: copy the direct link to send to a PC */
    mobileCopyCta: string;
    mobileCopied: string;
    mobileStillDownload: string;
    viaGithub: string;
    mirrorsLead: string;
    /** "{size}" is replaced with the installer size in MB */
    mirrorNote: string;
    historyLabel: string;
    releaseNotes: string;
    /** shown after a download actually starts (SmartScreen walkthrough) */
    smartscreenStarted: string;
    close: string;
  };
  faq: {
    kicker: string;
    title: string;
    items: FaqItem[];
  };
  footer: {
    tagline: string;
    license: string;
    links: { label: string; href: string }[];
  };
}
