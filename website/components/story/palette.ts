/**
 * Data-viz tones for /story/ charts, expressed as CSS variables (defined in
 * app/globals.css @theme + its dark overrides) so every chart flips with the
 * theme. Server markup embeds var()/color-mix() strings; canvas/SVG string
 * builders resolve actual values at paint time via readTones() and re-render
 * on theme changes via onThemeChange().
 */
export const TONE = {
  coral: "var(--color-coral-deep)",
  coralInk: "var(--color-coral-ink)",
  gold: "var(--color-gold)",
  teal: "var(--color-teal)",
  slate: "var(--color-slate)",
  ink: "var(--color-ink)",
  ink2: "var(--color-ink-2)",
  ink3: "var(--color-ink-3)",
  line: "var(--color-line)",
  panel: "var(--color-panel)",
} as const;

/** Translucent tint of a tone (flat color over whatever surface is beneath). */
export function mixTone(tone: string, pct: number): string {
  return `color-mix(in srgb, ${tone} ${Math.round(pct)}%, transparent)`;
}

export interface ResolvedTones {
  coral: string;
  coralInk: string;
  gold: string;
  teal: string;
  slate: string;
  ink: string;
  ink2: string;
  ink3: string;
  line: string;
  panel: string;
}

/** Resolve the current theme's tone values (client only; canvas needs literals). */
export function readTones(): ResolvedTones {
  const cs = getComputedStyle(document.documentElement);
  const v = (name: string) => cs.getPropertyValue(name).trim();
  return {
    coral: v("--color-coral-deep"),
    coralInk: v("--color-coral-ink"),
    gold: v("--color-gold"),
    teal: v("--color-teal"),
    slate: v("--color-slate"),
    ink: v("--color-ink"),
    ink2: v("--color-ink-2"),
    ink3: v("--color-ink-3"),
    line: v("--color-line"),
    panel: v("--color-panel"),
  };
}

/** Fires on any theme flip: explicit toggle (data-theme) or system change. */
export function onThemeChange(cb: () => void): () => void {
  const mo = new MutationObserver(cb);
  mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  mq.addEventListener("change", cb);
  return () => {
    mo.disconnect();
    mq.removeEventListener("change", cb);
  };
}

/** Emotion class -> tone (order matches EMOTION in content/story-data.ts). */
export const EMOTION_COLORS: Record<string, string> = {
  中性指令: TONE.ink3,
  "批评/宣泄": TONE.coral,
  "放权/催进度": TONE.slate,
  "肯定/赞赏": TONE.gold,
  "探讨/授权反驳": TONE.teal,
};

export const QUOTE_TONE_COLORS = {
  neg: TONE.coral,
  pos: TONE.gold,
  chal: TONE.teal,
  phil: TONE.slate,
} as const;
