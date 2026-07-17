/**
 * Static schematic diagrams for the COLOR and FINISH sections — flat
 * engineering drawings (SVG/CSS), theme-aware via tokens, revealed with
 * [data-fx-cell] staggers. No photography, no gradients on chrome.
 */

/** 05 COLOR — the tonal-duotone ramp, 16 OKLCH lightness steps of one hue. */
export function TonalRamp() {
  const steps = Array.from({ length: 16 }, (_, i) => 0.22 + (i / 15) * 0.65);
  return (
    <div className="flex h-[52px] w-full border border-line" role="img" aria-label="OKLCH tonal ramp">
      {steps.map((l, i) => (
        <span
          key={i}
          data-fx-cell
          className="h-full flex-1"
          style={{ background: `oklch(${l.toFixed(3)} 0.085 25)`, ["--fxd" as string]: `${i * 40}ms` }}
        />
      ))}
    </div>
  );
}

/** 05 COLOR — one squircle corner: arc segment + two tangent cubic ramps. */
export function SquircleCorner() {
  return (
    <svg viewBox="0 0 200 120" className="block w-full" role="img" aria-label="Squircle corner smoothing schematic">
      {/* straight edges */}
      <path d="M 6 114 L 6 74" stroke="var(--color-ink-3)" strokeWidth="2" fill="none" />
      <path d="M 128 8 L 194 8" stroke="var(--color-ink-3)" strokeWidth="2" fill="none" />
      {/* tangent cubic ramp in (teal) */}
      <path d="M 6 74 C 6 46 10 34 24 22" stroke="var(--color-teal)" strokeWidth="2.5" fill="none" />
      {/* circular arc keeps (1 - xi) of the turn (coral) */}
      <path d="M 24 22 A 40 40 0 0 1 66 10" stroke="var(--color-coral)" strokeWidth="2.5" fill="none" />
      {/* tangent cubic ramp out (teal) */}
      <path d="M 66 10 C 90 6 106 8 128 8" stroke="var(--color-teal)" strokeWidth="2.5" fill="none" />
      {/* joints */}
      {[
        [6, 74],
        [24, 22],
        [66, 10],
        [128, 8],
      ].map(([x, y], i) => (
        <circle key={i} cx={x} cy={y} r="3.4" fill="var(--color-ink)" stroke="var(--color-canvas)" strokeWidth="1.5" />
      ))}
      <text x="30" y="52" fontSize="10" fontFamily="var(--font-mono)" fill="var(--color-coral-ink)">
        arc
      </text>
      <text x="96" y="26" fontSize="10" fontFamily="var(--font-mono)" fill="var(--color-teal)">
        cubic
      </text>
    </svg>
  );
}

const FINISH_ART: Record<"glass" | "pixel" | "sticker", React.ReactNode> = {
  glass: (
    <svg viewBox="0 0 160 120" className="block w-full" aria-hidden>
      {/* grounding halo just outside the slab */}
      <ellipse cx="80" cy="102" rx="52" ry="8" fill="var(--color-ink-3)" opacity="0.22" />
      {/* translucent slab body */}
      <rect x="30" y="18" width="100" height="80" rx="20" fill="var(--color-slate)" opacity="0.3" />
      <rect x="30" y="18" width="100" height="80" rx="20" fill="none" stroke="var(--color-slate)" strokeWidth="2" />
      {/* fresnel/specular hairline */}
      <path d="M 44 26 Q 80 18 116 26" stroke="#ffffff" strokeWidth="2.5" fill="none" opacity="0.85" />
      {/* rim refraction ticks */}
      <path d="M 33 62 l -6 3 M 127 62 l 6 3" stroke="var(--color-teal)" strokeWidth="2" />
      {/* frosted subject */}
      <circle cx="80" cy="58" r="20" fill="var(--color-canvas)" opacity="0.85" />
    </svg>
  ),
  pixel: (
    <svg viewBox="0 0 160 120" className="block w-full" aria-hidden>
      {(() => {
        const cells: React.ReactNode[] = [];
        const palette = ["var(--color-coral)", "var(--color-gold)", "var(--color-teal)", "var(--color-panel)"];
        const glyph = [
          [0, 0, 3, 3, 0, 0],
          [0, 3, 1, 1, 3, 0],
          [3, 1, 0, 0, 1, 3],
          [3, 1, 0, 0, 1, 3],
          [0, 3, 1, 1, 3, 0],
          [0, 0, 3, 3, 0, 0],
        ];
        for (let r = 0; r < 6; r++) {
          for (let c = 0; c < 6; c++) {
            cells.push(
              <rect
                key={`${r}-${c}`}
                x={26 + c * 18}
                y={6 + r * 18}
                width="17"
                height="17"
                fill={palette[glyph[r][c]]}
                stroke="var(--color-line)"
                strokeWidth="1"
              />,
            );
          }
        }
        return cells;
      })()}
    </svg>
  ),
  sticker: (
    <svg viewBox="0 0 160 120" className="block w-full" aria-hidden>
      {/* soft outer shadow from the chamfer distance */}
      <ellipse cx="82" cy="98" rx="46" ry="9" fill="var(--color-ink-3)" opacity="0.28" />
      {/* white die-cut border */}
      <path
        d="M 80 12 C 108 12 128 30 128 56 C 128 84 106 100 80 100 C 54 100 32 84 32 56 C 32 30 52 12 80 12 Z"
        fill="#ffffff"
        stroke="var(--color-line)"
        strokeWidth="1.5"
      />
      {/* the shrunk artwork inside */}
      <path
        d="M 80 24 C 100 24 116 38 116 56 C 116 76 100 88 80 88 C 60 88 44 76 44 56 C 44 38 60 24 80 24 Z"
        fill="var(--color-gold)"
      />
      <circle cx="80" cy="56" r="14" fill="#ffffff" />
    </svg>
  ),
};

/** 06 FINISH — three recipe cards with schematic art. */
export function FinishCards({
  finishes,
}: {
  finishes: { key: "glass" | "pixel" | "sticker"; kicker: string; name: string; recipe: string[] }[];
}) {
  return (
    <div className="grid gap-px border border-line bg-line sm:grid-cols-3">
      {finishes.map((f, i) => (
        <div key={f.key} data-fx-cell className="bg-card p-5" style={{ ["--fxd" as string]: `${i * 140}ms` }}>
          <div className="border border-line bg-panel p-3">{FINISH_ART[f.key]}</div>
          <p className="mt-4 font-mono text-[10px] tracking-[0.18em] text-coral-ink">{f.kicker}</p>
          <h3 className="mt-1 text-[17px] font-bold text-ink">{f.name}</h3>
          <ol className="mt-3 space-y-1.5">
            {f.recipe.map((r, j) => (
              <li key={j} className="flex gap-2 text-[12.5px] leading-[1.55] text-ink-2">
                <span className="font-mono text-[11px] text-ink-3">{j + 1}.</span>
                {r}
              </li>
            ))}
          </ol>
        </div>
      ))}
    </div>
  );
}
