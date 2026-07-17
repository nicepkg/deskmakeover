import type { Metadata } from "next";
import "./globals.css";
import { satoshi } from "@/lib/fonts";

export const metadata: Metadata = {
  title: "404 · DeskMakeover",
  robots: { index: false },
};

/**
 * Branded bilingual 404 for the whole export (both locale trees share it).
 * Served by Cloudflare's not_found_handling: "404-page" with a real 404 status.
 */
export default function GlobalNotFound() {
  return (
    <html lang="en" className={satoshi.variable}>
      <body>
        <main className="flex min-h-svh flex-col items-center justify-center bg-canvas px-6 text-center">
          <img src="/logo.png" alt="" width={44} height={44} className="h-11 w-11" />
          <p className="mt-6 font-mono text-[12px] tracking-[0.22em] text-ink-3">404</p>
          <h1 className="mt-3 font-display text-[28px] font-semibold tracking-tight text-ink">
            Page not found · 页面不存在
          </h1>
          <p className="mt-3 max-w-[38rem] text-[15px] leading-[1.6] text-ink-2">
            This DeskMakeover page does not exist. The site lives at the two homes below.
            这个页面不存在，站点入口在下面两处。
          </p>
          <div className="mt-8 flex flex-wrap items-center justify-center gap-3">
            <a
              href="/"
              className="border border-line bg-card px-4 py-2 text-[14px] font-semibold text-ink transition-colors hover:border-ink-3"
            >
              English home
            </a>
            <a
              href="/zh/"
              className="border border-line bg-card px-4 py-2 text-[14px] font-semibold text-ink transition-colors hover:border-ink-3"
            >
              中文首页
            </a>
          </div>
        </main>
      </body>
    </html>
  );
}
