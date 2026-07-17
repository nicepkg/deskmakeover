"use client";

import { useEffect, useState } from "react";
import { THEME_STORAGE_KEY } from "@/lib/theme";

type Mode = "light" | "auto" | "dark";

const ICONS: Record<Mode, React.ReactNode> = {
  light: (
    // sun
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" aria-hidden>
      <circle cx="8" cy="8" r="3" />
      <path d="M8 1.2v1.8M8 13v1.8M1.2 8H3M13 8h1.8M3.2 3.2l1.3 1.3M11.5 11.5l1.3 1.3M12.8 3.2l-1.3 1.3M4.5 11.5l-1.3 1.3" />
    </svg>
  ),
  auto: (
    // monitor = follow the system
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" aria-hidden>
      <rect x="1.8" y="3" width="12.4" height="8" />
      <path d="M5.5 13.8h5M8 11v2.8" />
    </svg>
  ),
  dark: (
    // moon
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" aria-hidden>
      <path d="M13.2 9.6A5.6 5.6 0 1 1 6.4 2.8a4.4 4.4 0 0 0 6.8 6.8Z" />
    </svg>
  ),
};

const LABELS: Record<Mode, string> = { light: "浅色 Light", auto: "跟随系统 Auto", dark: "深色 Dark" };

function apply(mode: Mode) {
  const el = document.documentElement;
  if (mode === "auto") {
    el.removeAttribute("data-theme");
    try {
      localStorage.removeItem(THEME_STORAGE_KEY);
    } catch {}
  } else {
    el.setAttribute("data-theme", mode);
    try {
      localStorage.setItem(THEME_STORAGE_KEY, mode);
    } catch {}
  }
}

/**
 * Light / auto / dark segmented control (nav chrome). Auto (follow the
 * system) is the default; an explicit pick is persisted and stamped as
 * html[data-theme] — see lib/theme.ts for the pre-paint half.
 */
export function ThemeToggle({ className = "" }: { className?: string }) {
  const [mode, setMode] = useState<Mode>("auto");

  useEffect(() => {
    try {
      const t = localStorage.getItem(THEME_STORAGE_KEY);
      if (t === "light" || t === "dark") setMode(t);
    } catch {}
  }, []);

  return (
    <div
      role="radiogroup"
      aria-label="配色主题"
      className={`flex border border-line bg-canvas ${className}`}
    >
      {(["light", "auto", "dark"] as const).map((m) => (
        <button
          key={m}
          type="button"
          role="radio"
          aria-checked={mode === m}
          title={LABELS[m]}
          onClick={() => {
            setMode(m);
            apply(m);
          }}
          className={`grid h-7 w-7 place-items-center transition-colors [&_svg]:h-[14px] [&_svg]:w-[14px] ${
            mode === m ? "bg-panel text-ink" : "text-ink-3 hover:text-ink"
          }`}
        >
          {ICONS[m]}
        </button>
      ))}
    </div>
  );
}
