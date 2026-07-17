/**
 * /engine/ — the pixel-engine deep-dive page (spec: docs/specs/10-website.md
 * §"/engine/"). One EngineDict per locale (engine-en.ts / engine-zh.ts).
 *
 * Wording iron laws (owner-approved, 2026-07-17 panel): the claim is
 * "purpose-built, fully deterministic pixel pipeline" — never "original
 * algorithm", never "AI". Standard methods are credited by name; byte-parity
 * claims stay scoped to WASM ↔ native from the same Rust source. Every number
 * shown on the page must deep-link to source (receipts, not vanity metrics).
 */

export interface EngineHead {
  /** mono index label, e.g. "01" */
  index: string;
  /** mono uppercase kicker, e.g. "PORTRAIT" */
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
  };
  portrait: EngineHead & {
    steps: { key: string; label: string }[];
    probeCaption: string;
    iouLabel: string;
  };
  separate: EngineHead & {
    stages: { title: string; detail: string }[];
    floodCaption: string;
    replay: string;
  };
  rescue: EngineHead & {
    beats: { key: string; title: string; detail: string }[];
    gauges: { deltaE: string; deltaL: string; melt: string };
    caption: string;
  };
  invariant: EngineHead & {
    rule: string;
    ruleNote: string;
    wheelCaption: string;
  };
  color: EngineHead & {
    points: { title: string; detail: string }[];
  };
  finish: EngineHead & {
    finishes: {
      key: "glass" | "pixel" | "sticker";
      kicker: string;
      name: string;
      recipe: string[];
    }[];
  };
  guarantee: EngineHead & {
    items: { title: string; detail: string }[];
    receiptsLead: string;
    receipts: EngineReceipt[];
  };
  playground: EngineHead & {
    /** the "this demo is the product" badge */
    badge: string;
    sampleLabel: string;
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
    /** the derived-plate chip on the hue slider */
    autoHue: string;
    loading: string;
    fallbackNote: string;
  };
  cta: {
    title: string;
    body: string;
    download: string;
    github: string;
  };
}
