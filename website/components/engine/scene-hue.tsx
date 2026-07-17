/**
 * 04 INVARIANT — the hue wheel. Three colliding plate hues rotate apart to a
 * ≥12° gap (rotation capped at ±18°) with a critically-damped settle: pure
 * CSS keyed off the [data-fx] scope. Server-rendered final state = separated.
 */
export function HueWheelScene({ rule, ruleNote, caption }: { rule: string; ruleNote: string; caption: string }) {
  const ticks = Array.from({ length: 36 }, (_, i) => i * 10);
  // three colliding blue-family plates: 226° / 230° / 234° -> 216° / 230° / 244°
  const dots = [
    { hue: 216, from: 226, delay: 0 },
    { hue: 230, from: 230, delay: 0 },
    { hue: 244, from: 234, delay: 90 },
    { hue: 40, from: 40, delay: 0 },
    { hue: 130, from: 130, delay: 0 },
  ];
  return (
    <div>
      <div className="relative mx-auto aspect-square w-full max-w-[340px] border border-line bg-card">
        <div className="absolute inset-0" aria-hidden>
          {/* the wheel: 36 hue ticks */}
          {ticks.map((deg) => (
            <span
              key={deg}
              className="absolute left-1/2 top-1/2 h-[110px] w-[2px] md:h-[128px]"
              style={{ transform: `translate(-50%, -100%) rotate(${deg}deg)`, transformOrigin: "bottom center" }}
            >
              <span
                className="absolute left-0 top-0 h-[9px] w-full"
                style={{ background: `oklch(0.72 0.13 ${deg})`, opacity: 0.75 }}
              />
            </span>
          ))}
          {/* plate dots — the colliding trio rotates apart */}
          {dots.map((d, i) => (
            <span
              key={i}
              className="eng-rot absolute left-1/2 top-1/2 h-[92px] w-[14px] md:h-[108px]"
              style={{
                transformOrigin: "bottom center",
                translate: "-50% -100%",
                ["--rot0" as string]: `${d.from}deg`,
                ["--rot" as string]: `${d.hue}deg`,
                ["--fxd" as string]: `${d.delay}ms`,
              }}
            >
              <span
                className="absolute left-1/2 top-0 h-[14px] w-[14px] -translate-x-1/2 rounded-full border-2 border-canvas"
                style={{ background: `oklch(0.68 0.15 ${d.hue})` }}
              />
            </span>
          ))}
          {/* centre annotation */}
          <div className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 text-center">
            <p className="font-mono text-[11px] text-ink-3">MIN GAP</p>
            <p className="font-mono text-[22px] font-bold text-ink tabular-nums">
              <span data-fx-count>12</span>°
            </p>
            <p className="mt-1 font-mono text-[10.5px] text-ink-3">CAP ±18°</p>
          </div>
        </div>
      </div>

      <div className="mx-auto mt-5 max-w-[340px] border border-line bg-panel px-4 py-3.5 text-center">
        <p className="font-mono text-[10px] tracking-[0.18em] text-coral-ink">IRON LAW</p>
        <p className="mt-1.5 text-[15px] font-bold text-ink">{rule}</p>
        <p className="mt-1 text-[11.5px] leading-[1.55] text-ink-3">{ruleNote}</p>
      </div>
      <p className="mx-auto mt-3 max-w-[340px] text-center text-[12px] leading-[1.6] text-ink-3">{caption}</p>
    </div>
  );
}
