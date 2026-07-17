/**
 * 03 RESCUE — the three-beat danger / detect / save sequence, told in SVG
 * with CSS transitions keyed off the [data-fx] scope (.fx-wait = danger
 * state, .fx-in = play). The subject's own colour NEVER changes — the rescue
 * only adds an outline and a shadow, exactly like the engine's iron law.
 * Server-rendered final state = rescued.
 */
export function RescueScene({
  beats,
  gauges,
  caption,
}: {
  beats: { key: string; title: string; detail: string }[];
  gauges: { deltaE: string; deltaL: string; melt: string };
  caption: string;
}) {
  // rim probes along the subject circle (cx 100, cy 88, r 46)
  const probes = Array.from({ length: 14 }, (_, i) => {
    const a = (i / 14) * Math.PI * 2 - Math.PI / 2;
    return { x: 100 + Math.cos(a) * 46, y: 88 + Math.sin(a) * 46, i };
  });
  const C = (2 * Math.PI * 49).toFixed(1); // outline circumference

  const gaugeRows: { label: string; fill: number; threshold: number; over: boolean; delay: number }[] = [
    { label: gauges.deltaE, fill: 30, threshold: 52, over: false, delay: 520 },
    { label: gauges.deltaL, fill: 22, threshold: 46, over: false, delay: 640 },
    { label: gauges.melt, fill: 78, threshold: 50, over: true, delay: 760 },
  ];

  return (
    <div>
      <div className="relative mx-auto w-full max-w-[340px] border border-line bg-card">
        <svg viewBox="0 0 200 176" className="block w-full" role="img" aria-label={caption}>
          {/* the new plate — deliberately close to the subject's colour */}
          <rect x="28" y="20" width="144" height="136" rx="26" fill="#ff8a7a" />
          {/* soft shadow lands at the rescue beat — the animated group fades to 1,
              the inner ellipse keeps the soft 0.18 (class opacity would override it) */}
          <g className="eng-beat" style={{ ["--fxd" as string]: "1320ms" }}>
            <ellipse cx="100" cy="140" rx="44" ry="9" fill="#000000" opacity="0.18" />
          </g>
          {/* the subject: its fill NEVER changes across the beats */}
          <circle cx="100" cy="88" r="46" fill="#ff6f5e" />
          {/* rescue outline draws on along the silhouette */}
          <circle
            className="eng-draw"
            style={{ ["--dash" as string]: C, ["--fxd" as string]: "1100ms" }}
            cx="100"
            cy="88"
            r="49"
            fill="none"
            stroke="#ffffff"
            strokeWidth="3.5"
            strokeDasharray={C}
            transform="rotate(-90 100 88)"
          />
          {/* detect probes, appearing point by point */}
          {probes.map((p) => (
            <circle
              key={p.i}
              className="eng-beat"
              style={{ ["--fxd" as string]: `${140 + p.i * 34}ms` }}
              cx={p.x}
              cy={p.y}
              r="2.6"
              fill="#c98a12"
            />
          ))}
        </svg>
      </div>

      {/* the two thresholds + the melt share that actually triggers */}
      <div className="mx-auto mt-4 max-w-[340px] space-y-2.5">
        {gaugeRows.map((g) => (
          <div key={g.label} className="flex items-center gap-3">
            <span className="w-[86px] flex-none font-mono text-[10.5px] tracking-[0.04em] text-ink-3">
              {g.label}
            </span>
            <div className="relative h-[8px] flex-1 bg-panel">
              <div
                className="eng-gauge absolute inset-y-0 left-0"
                style={{
                  width: `${g.fill}%`,
                  background: g.over ? "var(--color-coral)" : "var(--color-slate)",
                  ["--fxd" as string]: `${g.delay}ms`,
                }}
              />
              <div
                className="absolute inset-y-[-2px] w-[2px] bg-ink-3"
                style={{ left: `${g.threshold}%` }}
                aria-hidden
              />
            </div>
          </div>
        ))}
      </div>

      {/* beat chips */}
      <div className="mx-auto mt-5 grid max-w-[340px] grid-cols-3 gap-px border border-line bg-line">
        {beats.map((b, i) => (
          <div
            key={b.key}
            className="eng-beat bg-card px-2.5 py-2.5"
            style={{ ["--fxd" as string]: `${i === 0 ? 0 : i === 1 ? 480 : 1100}ms` }}
          >
            <p className="font-mono text-[9.5px] tracking-[0.14em] text-coral-ink">{b.key}</p>
            <p className="mt-1 text-[12.5px] font-semibold text-ink">{b.title}</p>
            <p className="mt-0.5 text-[10.5px] leading-[1.5] text-ink-3">{b.detail}</p>
          </div>
        ))}
      </div>

      <p className="mx-auto mt-3 max-w-[340px] text-center text-[12px] leading-[1.6] text-ink-3">{caption}</p>
    </div>
  );
}
