"use client";

import { useEffect, useRef, type CSSProperties, type ReactNode } from "react";

/**
 * Scroll enter. Content is visible by default (SSR, no-JS, print). JS arms the
 * animation ONLY for elements that start below the fold; elements already
 * scrolled past (top < 0) reveal immediately so fast scrolling never strands
 * anything hidden.
 */
export function Reveal({
  children,
  delay = 0,
  className,
}: {
  children: ReactNode;
  delay?: number;
  className?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    if (el.getBoundingClientRect().top < window.innerHeight) return;

    el.classList.add("rise-armed");
    const io = new IntersectionObserver(
      (records) => {
        for (const r of records) {
          if (r.isIntersecting || r.boundingClientRect.top < 0) {
            el.classList.add("in-view");
            io.disconnect();
          }
        }
      },
      { rootMargin: "0px 0px -8% 0px" },
    );
    io.observe(el);
    return () => io.disconnect();
  }, []);

  return (
    <div
      ref={ref}
      className={className}
      style={delay ? ({ "--rise-delay": `${delay}ms` } as CSSProperties) : undefined}
    >
      {children}
    </div>
  );
}
