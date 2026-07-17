import type { Dict } from "@/content/types";
import { LangSwitch } from "@/components/lang";
import { DownloadCta } from "@/components/download/button";
import { ThemeToggle } from "@/components/theme-toggle";
import { DOWNLOAD_URL, GITHUB_URL, RELEASE_READY } from "@/lib/site";

export function SiteNav({ dict }: { dict: Dict }) {
  const home = dict.locale === "zh" ? "/zh/" : "/";
  return (
    <header className="sticky top-0 z-50 border-b border-line bg-canvas/85 backdrop-blur-sm">
      <div className="mx-auto flex h-14 max-w-[1200px] items-center justify-between px-5 md:px-8">
        <a href={home} className="flex items-center gap-2.5">
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img src="/logo.png" alt="" width={22} height={22} className="h-[22px] w-[22px]" />
          <span className="font-display text-[15px] font-semibold tracking-tight text-ink">
            DeskMakeover
          </span>
        </a>
        <nav className="hidden items-center gap-7 font-mono text-[12px] tracking-[0.12em] text-ink-2 md:flex">
          <a href="#proof" className="transition-colors hover:text-ink">
            {dict.nav.proof.toUpperCase()}
          </a>
          <a href="#looks" className="transition-colors hover:text-ink">
            {dict.nav.looks.toUpperCase()}
          </a>
          <a href="#zones" className="transition-colors hover:text-ink">
            {dict.nav.zones.toUpperCase()}
          </a>
          <a href="/story/" className="transition-colors hover:text-ink">
            {dict.nav.story.toUpperCase()}
          </a>
          <a
            href={GITHUB_URL}
            target="_blank"
            rel="noreferrer"
            className="transition-colors hover:text-ink"
          >
            {dict.nav.github.toUpperCase()}
          </a>
        </nav>
        <div className="flex items-center gap-3">
          <LangSwitch
            href={dict.nav.langHref}
            lang={dict.locale === "zh" ? "en" : "zh"}
            className="font-mono text-[12px] tracking-[0.08em] text-ink-3 transition-colors hover:text-ink"
          >
            {dict.nav.langLabel}
          </LangSwitch>
          <ThemeToggle />
          {RELEASE_READY ? (
            <DownloadCta
              href={DOWNLOAD_URL}
              className="inline-flex h-8 items-center bg-ink px-3.5 text-[13px] font-semibold text-canvas transition-colors hover:bg-coral-deep hover:text-white"
            >
              {dict.nav.download}
            </DownloadCta>
          ) : (
            <a
              href="#download"
              className="inline-flex h-8 items-center bg-ink px-3.5 text-[13px] font-semibold text-canvas transition-colors hover:bg-coral-deep hover:text-white"
            >
              {dict.nav.download}
            </a>
          )}
        </div>
      </div>
    </header>
  );
}
