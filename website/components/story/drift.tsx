"use client";

import { useEffect, useRef } from "react";
import { DRIFT } from "@/content/story-data";
import { onThemeChange, readTones } from "./palette";

/**
 * Focus-drift stacked area: per-day share of design / engineering / outbound
 * keywords. Ported from the source dashboard, flat fills on the site palette.
 */
export function DriftChart() {
  const ref = useRef<SVGSVGElement>(null);

  useEffect(() => {
    const svg = ref.current;
    if (!svg) return;

    function draw() {
      if (!svg) return;
      const c = readTones();
      const W = svg.clientWidth || 900;
      const H = svg.clientHeight || 300;
      const padL = 30;
      const padR = 12;
      const padT = 12;
      const padB = 26;
      const N = DRIFT.length;
      const x = (i: number) => padL + (i / (N - 1)) * (W - padL - padR);
      const y = (p: number) => padT + (1 - p / 100) * (H - padT - padB);
      type Day = (typeof DRIFT)[number];
      const band = (lo: (d: Day) => number, hi: (d: Day) => number, fill: string, op: number) => {
        const top = DRIFT.map((d, i) => [x(i), y(hi(d))] as const);
        const bot = DRIFT.map((d, i) => [x(i), y(lo(d))] as const).reverse();
        const d = [
          ...top.map((p, i) => `${i ? "L" : "M"}${p[0].toFixed(1)} ${p[1].toFixed(1)}`),
          ...bot.map((p) => `L${p[0].toFixed(1)} ${p[1].toFixed(1)}`),
          "Z",
        ].join(" ");
        return `<path d="${d}" fill="${fill}" opacity="${op}"/>`;
      };
      let out = "";
      for (const p of [0, 25, 50, 75, 100]) {
        const gy = y(p);
        out += `<line x1="${padL}" y1="${gy.toFixed(1)}" x2="${W - padR}" y2="${gy.toFixed(1)}" stroke="${c.line}" stroke-width="1" stroke-dasharray="1 5"/>`;
        out += `<text x="4" y="${(gy + 3).toFixed(1)}" fill="${c.ink3}" font-size="9" font-family="var(--font-mono)">${p}%</text>`;
      }
      out += band(() => 0, (d) => d.design, c.coral, 0.82);
      out += band((d) => d.design, (d) => d.design + d.eng, c.teal, 0.72);
      out += band((d) => d.design + d.eng, () => 100, c.gold, 0.66);
      DRIFT.forEach((d, i) => {
        out += `<text x="${x(i).toFixed(1)}" y="${H - 8}" fill="${c.ink3}" font-size="9" font-family="var(--font-mono)" text-anchor="${i === 0 ? "start" : i === N - 1 ? "end" : "middle"}">${d.day.slice(-2)}</text>`;
      });
      svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
      svg.innerHTML = out;
    }

    draw();
    let rt: ReturnType<typeof setTimeout>;
    const onResize = () => {
      clearTimeout(rt);
      rt = setTimeout(draw, 180);
    };
    window.addEventListener("resize", onResize);
    const offTheme = onThemeChange(() => draw());
    return () => {
      window.removeEventListener("resize", onResize);
      clearTimeout(rt);
      offTheme();
    };
  }, []);

  return (
    <svg
      ref={ref}
      className="block h-[clamp(240px,34vw,320px)] w-full"
      preserveAspectRatio="none"
      role="img"
      aria-label="关注点迁移堆叠面积图：设计、工程、对外三类关键词的每日占比"
    />
  );
}
