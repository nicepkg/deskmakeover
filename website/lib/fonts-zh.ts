import localFont from "next/font/local";

/**
 * Display-only MiSans Semibold subset (heading glyphs, ~13 KB).
 * Imported only by the zh root layout so English pages never preload it.
 */
export const zhDisplay = localFont({
  src: "../app/fonts/misans-display-zh.woff2",
  weight: "600",
  display: "swap",
  variable: "--font-zh-display",
});
