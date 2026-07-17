/**
 * /engine/ — the pixel-engine deep-dive page, v2 (owner-directed redesign
 * 2026-07-17: real Windows icons everywhere, three.js layer explosion,
 * plain-language copy; spec: docs/specs/10-website.md §"/engine/").
 *
 * Wording iron laws (owner-approved): "purpose-built, fully deterministic
 * pixel pipeline" — never "original algorithm", never "AI". Copy is written
 * for ordinary people; the technical receipts live in §receipts and every
 * number deep-links to source.
 */

export interface EngineHead {
  /** mono index label, e.g. "01" */
  index: string;
  /** mono uppercase kicker, e.g. "READ" */
  kicker: string;
  title: string;
  body: string;
}

export interface EngineStat {
  /** count-up target (rendered server-side at its final value) */
  value: number;
  /** suffix printed after the number, e.g. " 行", " KB" */
  unit: string;
  label: string;
}

export interface EngineReceipt {
  value: string;
  label: string;
  /** deep link to the exact source file / test on GitHub */
  href: string;
}

export interface EngineDict {
  meta: {
    title: string;
    description: string;
  };
  hero: {
    eyebrow: string;
    title: string;
    sub: string;
    stats: EngineStat[];
    /** the 3D layer-explosion labels (bottom → top) + its caption */
    stack: {
      raw: string;
      plate: string;
      final: string;
      caption: string;
    };
  };
  /** 01 — the per-icon checkup (portrait) */
  read: EngineHead & {
    steps: { key: string; label: string }[];
    caption: string;
  };
  /** 02 — subject/background separation on a real white-plate icon */
  cut: EngineHead & {
    caption: string;
    replay: string;
    maskNote: string;
    /** floating 3D layer labels */
    layers: { bg: string; art: string; final: string };
  };
  /** 03 — auto-separation rescue, real A/B */
  rescue: EngineHead & {
    beats: { key: string; title: string; detail: string }[];
    offLabel: string;
    onLabel: string;
    caption: string;
    /** floating 3D layer labels */
    layers: { tile: string; rescue: string };
  };
  /** 04 — subject pixels never recoloured + hue spread trio */
  promise: EngineHead & {
    rule: string;
    before: string;
    after: string;
    caption: string;
  };
  /** 05 — finishes computed live on a real icon */
  finish: EngineHead & {
    finishes: {
      key: "glass" | "pixel" | "sticker";
      kicker: string;
      name: string;
      line: string;
    }[];
  };
  /** 06 — the playground */
  live: EngineHead & {
    badge: string;
    castLabel: string;
    uploadCta: string;
    uploadNote: string;
    controls: {
      shape: string;
      look: string;
      hue: string;
      finish: string;
      original: string;
    };
    /** control options; `tag` is the ABI enum name (config-abi order) */
    options: {
      shapes: { tag: string; label: string }[];
      looks: { tag: string; label: string }[];
      finishes: { tag: string; label: string }[];
    };
    autoHue: string;
    loading: string;
    fallbackNote: string;
  };
  /** 07 — the engineer's receipts */
  receipts: EngineHead & {
    receipts: EngineReceipt[];
  };
  cta: {
    title: string;
    body: string;
    download: string;
    github: string;
  };
  /** user-facing names for the real-icon cast (components/engine/lab.ts ids) */
  castNames: Record<string, string>;
}
