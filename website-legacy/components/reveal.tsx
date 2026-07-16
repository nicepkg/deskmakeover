"use client";

import { useEffect, useRef, type ReactNode } from "react";

interface RevealProps {
  children: ReactNode;
  /** stagger delay in ms */
  delay?: number;
  className?: string;
}

/**
 * Scroll-in reveal that never authors visibility: content is visible by
 * default (no JS, headless, print, reduced motion — all fine). After
 * hydration, elements safely BELOW the fold get armed (hidden) and revealed
 * by IntersectionObserver as they scroll in.
 */
export function Reveal({ children, delay = 0, className }: RevealProps) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const rect = el.getBoundingClientRect();
    if (rect.top <= window.innerHeight * 0.92) return; // visible or nearly so: no entrance
    el.classList.add("reveal-armed");
    const io = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          // fast scrolling coalesces records: also reveal anything already
          // scrolled past, or it would stay armed (hidden) forever
          if (entry.isIntersecting || entry.boundingClientRect.top < 0) {
            el.classList.add("in-view");
            io.disconnect();
          }
        }
      },
      { rootMargin: "0px 0px -8% 0px", threshold: 0.1 },
    );
    io.observe(el);
    return () => io.disconnect();
  }, []);

  return (
    <div
      ref={ref}
      className={`reveal ${className ?? ""}`}
      style={delay ? ({ "--reveal-delay": `${delay}ms` } as React.CSSProperties) : undefined}
    >
      {children}
    </div>
  );
}
