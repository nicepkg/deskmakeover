"use client";

import { useEffect, useRef } from "react";
import { ANNOS, DAY_FIRST, DAY_LIST, SERIES } from "@/content/story-data";
import { onThemeChange, readTones } from "./palette";

function rolling(arr: readonly number[], win: number): number[] {
  const half = win >> 1;
  const out: number[] = [];
  for (let i = 0; i < arr.length; i++) {
    let s = 0;
    let n = 0;
    for (let j = Math.max(0, i - half); j <= Math.min(arr.length - 1, i + half); j++) {
      s += arr[j];
      n++;
    }
    out.push(s / n);
  }
  return out;
}

const SMOOTH = rolling(SERIES, 13);

/**
 * Emotional-tide chart: per-message valence (-3..+3) smoothed with a
 * 13-message rolling mean; gold above the waterline, coral below, key
 * turning points annotated.
 *
 * Plays like a live stock ticker: when the card scrolls into view, a clip
 * window sweeps left to right at the pace of the line while a cursor dot
 * rides the line head; annotations surface as the line passes them. The
 * cursor keeps a radar ping afterwards. Final frame is drawn immediately for
 * reduced motion, and everything re-renders on theme flips.
 */
export function TideChart() {
  const ref = useRef<SVGSVGElement>(null);

  useEffect(() => {
    const svg = ref.current;
    if (!svg) return;
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    let played = reduce;
    let inView = reduce;
    let raf = 0;

    function draw() {
      if (!svg) return;
      cancelAnimationFrame(raf);
      const c = readTones();
      const W = svg.clientWidth || 900;
      const H = svg.clientHeight || 360;
      const padL = 34;
      const padR = 16;
      const padT = 30;
      const padB = 28;
      const N = SMOOTH.length;
      const x = (i: number) => padL + (i / (N - 1)) * (W - padL - padR);
      const y = (v: number) => padT + (1 - (v + 3) / 6) * (H - padT - padB);
      const y0 = y(0);

      let grid = "";
      const labelEvery = W < 560 ? 2 : 1;
      let lastLabelX = -Infinity;
      DAY_FIRST.forEach((idx, k) => {
        const gx = x(idx);
        grid += `<line x1="${gx.toFixed(1)}" y1="${padT}" x2="${gx.toFixed(1)}" y2="${H - padB}" stroke="${c.line}" stroke-width="1" stroke-dasharray="2 4"/>`;
        if (k % labelEvery !== 0) return;
        if (gx - lastLabelX < 34) return; // short days bunch up at the tail — skip colliding labels
        lastLabelX = gx;
        grid += `<text x="${gx.toFixed(1)}" y="${H - 9}" fill="${c.ink3}" font-size="10" font-family="var(--font-mono)" text-anchor="${k === 0 ? "start" : "middle"}">${DAY_LIST[k].slice(-2)}日</text>`;
      });
      for (const v of [3, 1.5, 0, -1.5, -3]) {
        const gy = y(v);
        grid += `<line x1="${padL}" y1="${gy.toFixed(1)}" x2="${W - padR}" y2="${gy.toFixed(1)}" stroke="${c.line}" stroke-width="${v === 0 ? 1.4 : 1}" ${v === 0 ? "" : 'stroke-dasharray="1 5"'}/>`;
      }
      const lab = (t: string, yv: number) =>
        `<text x="4" y="${(y(yv) + 3).toFixed(1)}" fill="${c.ink3}" font-size="9.5" font-family="var(--font-mono)">${t}</text>`;
      grid += lab("褒+3", 3) + lab("0", 0) + lab("贬−3", -3);

      const pts = SMOOTH.map((v, i) => [x(i), y(v)] as const);
      const lineD = pts.map((p, i) => `${i ? "L" : "M"}${p[0].toFixed(1)} ${p[1].toFixed(1)}`).join(" ");
      const area =
        `M${x(0).toFixed(1)} ${y0.toFixed(1)} ` +
        pts.map((p) => `L${p[0].toFixed(1)} ${p[1].toFixed(1)}`).join(" ") +
        ` L${x(N - 1).toFixed(1)} ${y0.toFixed(1)} Z`;

      let fig = `<path d="${area}" fill="${c.gold}" opacity="0.17" clip-path="url(#tideClipPos)"/>`;
      fig += `<path d="${area}" fill="${c.coral}" opacity="0.17" clip-path="url(#tideClipNeg)"/>`;
      fig += `<path id="tideLine" d="${lineD}" fill="none" stroke="${c.ink2}" stroke-width="1.6" stroke-linejoin="round" opacity="0.85"/>`;
      // narrow screens: the right-third annotations collide — keep a spread subset
      const narrowKeep = new Set([0, 1, 3, 4, 7]);
      const annos = W < 560 ? ANNOS.filter((_, k) => narrowKeep.has(k)) : ANNOS;
      for (const a of annos) {
        const ax = x(a.i);
        const ay = y(SMOOTH[a.i]);
        const up = a.dir === "up";
        const col = up ? c.gold : c.coralInk;
        const laby = up ? Math.max(padT + 8, ay - 46) : Math.min(H - padB - 8, ay + 44);
        fig += `<line x1="${ax.toFixed(1)}" y1="${ay.toFixed(1)}" x2="${ax.toFixed(1)}" y2="${laby.toFixed(1)}" stroke="${col}" stroke-width="1" opacity="0.55"/>`;
        fig += `<circle cx="${ax.toFixed(1)}" cy="${ay.toFixed(1)}" r="3.6" fill="${col}"/>`;
        const anchor = ax < W * 0.14 ? "start" : ax > W * 0.86 ? "end" : "middle";
        const ty = up ? laby - 4 : laby + 11;
        fig += `<text x="${ax.toFixed(1)}" y="${ty.toFixed(1)}" fill="${col}" font-size="10.5" font-weight="700" font-family="var(--font-mono)" text-anchor="${anchor}">${a.t}</text>`;
      }

      const cursorCol = SMOOTH[N - 1] >= 0 ? c.gold : c.coral;
      const out = `<defs>
        <clipPath id="tideClipPos"><rect x="0" y="${padT - 4}" width="${W}" height="${(y0 - (padT - 4)).toFixed(1)}"/></clipPath>
        <clipPath id="tideClipNeg"><rect x="0" y="${y0.toFixed(1)}" width="${W}" height="${(H - padB - y0 + 4).toFixed(1)}"/></clipPath>
        <clipPath id="tideReveal"><rect id="tideRevealRect" x="0" y="0" width="${W}" height="${H}"/></clipPath>
      </defs>
      ${grid}
      <g clip-path="url(#tideReveal)">${fig}</g>
      <g id="tideCursor" style="opacity:0">
        <circle class="tide-cursor-ring" r="7" fill="${cursorCol}" opacity="0.5"/>
        <circle r="3.4" fill="${cursorCol}"/>
      </g>`;
      svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
      svg.innerHTML = out;

      const reveal = svg.querySelector<SVGRectElement>("#tideRevealRect");
      const cursor = svg.querySelector<SVGGElement>("#tideCursor");
      const path = svg.querySelector<SVGPathElement>("#tideLine");
      if (!reveal || !cursor || !path) return;
      const end = pts[N - 1];
      if (played) {
        cursor.setAttribute("transform", `translate(${end[0].toFixed(1)} ${end[1].toFixed(1)})`);
        cursor.style.opacity = "1";
      } else {
        reveal.setAttribute("width", "0");
        if (inView) play(reveal, cursor, path);
      }
    }

    function play(reveal: SVGRectElement, cursor: SVGGElement, path: SVGPathElement) {
      const L = path.getTotalLength();
      const start = performance.now();
      const dur = 2800;
      cursor.style.opacity = "1";
      const tick = (now: number) => {
        const t = Math.min(1, (now - start) / dur);
        const e = 1 - Math.pow(1 - t, 3);
        const pt = path.getPointAtLength(L * e);
        reveal.setAttribute("width", String(pt.x + 1.5));
        cursor.setAttribute("transform", `translate(${pt.x.toFixed(1)} ${pt.y.toFixed(1)})`);
        if (t < 1) raf = requestAnimationFrame(tick);
        else played = true;
      };
      raf = requestAnimationFrame(tick);
    }

    draw();

    let io: IntersectionObserver | undefined;
    if (!reduce) {
      io = new IntersectionObserver(
        (records) => {
          for (const r of records) {
            if (r.isIntersecting) {
              inView = true;
              draw();
              io?.disconnect();
            }
          }
        },
        { threshold: 0.3 },
      );
      io.observe(svg);
    }

    let rt: ReturnType<typeof setTimeout>;
    const onResize = () => {
      clearTimeout(rt);
      rt = setTimeout(draw, 180);
    };
    window.addEventListener("resize", onResize);
    const offTheme = onThemeChange(() => draw());
    return () => {
      window.removeEventListener("resize", onResize);
      cancelAnimationFrame(raf);
      clearTimeout(rt);
      io?.disconnect();
      offTheme();
    };
  }, []);

  return (
    <svg
      ref={ref}
      className="block h-[clamp(300px,42vw,388px)] w-full overflow-visible"
      preserveAspectRatio="none"
      role="img"
      aria-label="情绪潮汐曲线：341 条发言的情绪滑动平均，金色为满意，珊瑚为不满"
    />
  );
}
