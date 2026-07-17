"use client";

import { useEffect, useRef } from "react";
import { CAT_DESIGN, CAT_ENG, WORDCLOUD } from "@/content/story-data";
import { onThemeChange, readTones, type ResolvedTones } from "./palette";

const catColor = (w: string, c: ResolvedTones) =>
  CAT_DESIGN.has(w) ? c.coral : CAT_ENG.has(w) ? c.teal : c.gold;

interface Placed {
  word: string;
  x: number;
  y: number;
  w: number;
  h: number;
  fs: number;
  t: number;
}

/**
 * Canvas word cloud (archimedean spiral, collision-packed) — a straight port
 * of the source dashboard's layout so every term lands, restyled to the site
 * palette. Words stagger in (fade + rise) the first time the card scrolls
 * into view; reduced motion draws the final frame immediately.
 */
export function WordCloud() {
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const cv = ref.current;
    if (!cv) return;
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    let raf = 0;
    let played = reduce;

    function layout(W: number, H: number, ctx: CanvasRenderingContext2D): Placed[] {
      const words = WORDCLOUD.slice(0, 42);
      const max = words[0][1];
      const min = words[words.length - 1][1];
      const minF = Math.max(13, Math.min(16, W / 28));
      const maxF = Math.min(66, W / 9.5);
      const placed: Placed[] = [];
      const cx = W / 2;
      const cy = H / 2;
      const fam = getComputedStyle(document.body).fontFamily;
      for (const [word, freq] of words) {
        const t = (Math.sqrt(freq) - Math.sqrt(min)) / (Math.sqrt(max) - Math.sqrt(min) || 1);
        const fs = Math.round(minF + t * (maxF - minF));
        ctx.font = `${fs < 20 ? 600 : 750} ${fs}px ${fam}`;
        const w = ctx.measureText(word).width + fs * 0.34;
        const h = fs * 1.16;
        let ok = false;
        let px = cx;
        let py = cy;
        for (let a = 0; a < 560; a += 0.32) {
          const r = 3.4 * a;
          px = cx + r * Math.cos(a) - w / 2;
          py = cy + r * Math.sin(a) * 0.62 - h / 2;
          if (px < 4 || py < 4 || px + w > W - 4 || py + h > H - 4) continue;
          let hit = false;
          for (const p of placed) {
            if (px < p.x + p.w && px + w > p.x && py < p.y + p.h && py + h > p.y) {
              hit = true;
              break;
            }
          }
          if (!hit) {
            ok = true;
            break;
          }
        }
        if (!ok) continue;
        placed.push({ word, x: px, y: py, w, h, fs, t });
      }
      return placed;
    }

    function draw(progress: number) {
      if (!cv) return;
      const rect = cv.getBoundingClientRect();
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      const W = rect.width;
      const H = rect.height;
      cv.width = W * dpr;
      cv.height = H * dpr;
      const ctx = cv.getContext("2d");
      if (!ctx) return;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, W, H);
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      const tones = readTones();
      const placed = layout(W, H, ctx);
      const fam = getComputedStyle(document.body).fontFamily;
      const n = placed.length;
      placed.forEach((p, i) => {
        // per-word window: word i runs from i/(n+6) to (i+7)/(n+6) of the timeline
        const local = Math.max(0, Math.min(1, (progress * (n + 6) - i) / 7));
        if (local === 0) return;
        const ease = 1 - Math.pow(1 - local, 3);
        ctx.font = `${p.fs < 20 ? 600 : 750} ${p.fs}px ${fam}`;
        ctx.fillStyle = catColor(p.word, tones);
        ctx.globalAlpha = (0.45 + 0.55 * p.t) * ease;
        ctx.fillText(p.word.replace(/\/.*$/, ""), p.x + p.w / 2, p.y + p.h / 2 + (1 - ease) * 8);
      });
      ctx.globalAlpha = 1;
    }

    function play() {
      const start = performance.now();
      const dur = 1500;
      const tick = (now: number) => {
        const t = Math.min(1, (now - start) / dur);
        draw(t);
        if (t < 1) raf = requestAnimationFrame(tick);
      };
      raf = requestAnimationFrame(tick);
    }

    if (reduce) {
      draw(1);
    } else {
      draw(0);
      const io = new IntersectionObserver(
        (records) => {
          for (const r of records) {
            if (r.isIntersecting) {
              played = true;
              play();
              io.disconnect();
            }
          }
        },
        { threshold: 0.25 },
      );
      io.observe(cv);
    }

    let rt: ReturnType<typeof setTimeout>;
    const onResize = () => {
      clearTimeout(rt);
      rt = setTimeout(() => draw(played ? 1 : 0), 180);
    };
    window.addEventListener("resize", onResize);
    const offTheme = onThemeChange(() => draw(played ? 1 : 0));
    return () => {
      window.removeEventListener("resize", onResize);
      cancelAnimationFrame(raf);
      clearTimeout(rt);
      offTheme();
    };
  }, []);

  return (
    <canvas
      ref={ref}
      className="block h-[clamp(320px,46vw,460px)] w-full"
      role="img"
      aria-label="词云：43 个主题词按出现次数加权，三色区分设计、工程与态度"
    />
  );
}
