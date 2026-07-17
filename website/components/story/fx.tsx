"use client";

import { useEffect, useLayoutEffect } from "react";

const useIsoLayoutEffect = typeof window !== "undefined" ? useLayoutEffect : useEffect;

/**
 * Chart/number entrance FX for /story/. Server markup always carries the
 * FINAL state (no-JS, crawlers and print see complete charts); this component
 * arms only the scopes still below the fold, then plays them when they enter:
 *
 *   [data-fx]          scope root (one per chart card)
 *   [data-fx-w="43.2"] bar fill  -> width animates 0 -> N%
 *   [data-fx-h="61.8"] column    -> height animates 0 -> N%
 *   [data-fx-grow="9"] flex seg  -> flex-grow animates 0.0001 -> N
 *   [data-fx-count]    number    -> counts up to its rendered value
 *   [data-fx-cell]     grid cell -> fades in with --fxd inline delay
 *   [data-fx-wipe]     svg wrap  -> left-to-right clip reveal
 *
 * Respects prefers-reduced-motion (never arms, final state stays).
 */
export function StoryFx() {
  useIsoLayoutEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;

    // every scope arms; the ones already on screen play on the next frame
    // (numbers jump from 0 on page load), the rest wait for intersection
    const scopes = Array.from(document.querySelectorAll<HTMLElement>("[data-fx]"));
    if (scopes.length === 0) return;
    const aboveFold = new Set(
      scopes.filter((el) => el.getBoundingClientRect().top < window.innerHeight),
    );

    const counters = new Map<HTMLElement, { target: number; decimals: number; grouped: boolean }>();

    for (const scope of scopes) {
      // fx-wait persists from arming until play — pure-CSS scene elements
      // (engine scenes) key their initial state off it; fx-armed only spans
      // the zeroing commit below.
      scope.classList.add("fx-armed", "fx-wait");
      for (const el of scope.querySelectorAll<HTMLElement>("[data-fx-w]")) el.style.width = "0%";
      for (const el of scope.querySelectorAll<HTMLElement>("[data-fx-h]")) el.style.height = "0%";
      for (const el of scope.querySelectorAll<HTMLElement>("[data-fx-grow]"))
        el.style.flexGrow = "0.0001";
      for (const el of scope.querySelectorAll<HTMLElement>("[data-fx-cell]"))
        el.style.opacity = "0";
      for (const el of scope.querySelectorAll<HTMLElement>("[data-fx-wipe]"))
        el.style.clipPath = "inset(0 100% 0 0)";
      for (const el of scope.querySelectorAll<HTMLElement>("[data-fx-count]")) {
        const raw = el.textContent ?? "0";
        const grouped = raw.includes(","); // "11,946" keeps its thousands grouping
        const target = parseFloat(raw.replace(/,/g, ""));
        if (Number.isNaN(target)) continue;
        const decimals = raw.includes(".") ? raw.split(".")[1].length : 0;
        counters.set(el, { target, decimals, grouped });
        el.textContent = (0).toFixed(decimals);
      }
      void scope.offsetWidth; // commit the zero state before re-enabling transitions
      scope.classList.remove("fx-armed");
    }

    const play = (scope: HTMLElement) => {
      scope.classList.remove("fx-wait");
      scope.classList.add("fx-in");
      for (const el of scope.querySelectorAll<HTMLElement>("[data-fx-w]"))
        el.style.width = `${el.dataset.fxW}%`;
      for (const el of scope.querySelectorAll<HTMLElement>("[data-fx-h]"))
        el.style.height = `${el.dataset.fxH}%`;
      for (const el of scope.querySelectorAll<HTMLElement>("[data-fx-grow]"))
        el.style.flexGrow = el.dataset.fxGrow ?? "1";
      for (const el of scope.querySelectorAll<HTMLElement>("[data-fx-cell]"))
        el.style.opacity = "1";
      for (const el of scope.querySelectorAll<HTMLElement>("[data-fx-wipe]"))
        el.style.clipPath = "inset(0 0 0 0)";
      for (const el of scope.querySelectorAll<HTMLElement>("[data-fx-count]")) {
        const spec = counters.get(el);
        if (!spec) continue;
        const start = performance.now();
        const dur = 950;
        const tick = (now: number) => {
          const t = Math.min(1, (now - start) / dur);
          const eased = 1 - Math.pow(1 - t, 3);
          const v = spec.target * eased;
          el.textContent = spec.grouped
            ? Math.round(v).toLocaleString("en-US")
            : v.toFixed(spec.decimals);
          if (t < 1) requestAnimationFrame(tick);
        };
        requestAnimationFrame(tick);
      }
    };

    const io = new IntersectionObserver(
      (records) => {
        for (const r of records) {
          if (r.isIntersecting || r.boundingClientRect.top < 0) {
            play(r.target as HTMLElement);
            io.unobserve(r.target);
          }
        }
      },
      { threshold: 0.18 },
    );
    for (const scope of scopes) {
      if (aboveFold.has(scope)) requestAnimationFrame(() => play(scope));
      else io.observe(scope);
    }
    return () => io.disconnect();
  }, []);

  return null;
}
