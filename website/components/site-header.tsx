import { GithubLogoIcon } from "@/components/icons";
import type { Dict } from "@/content/types";
import { DOWNLOAD_URL, GITHUB_URL, RELEASE_READY } from "@/lib/site";

export function SiteHeader({ dict }: { dict: Dict }) {
  const home = dict.locale === "zh" ? "/zh/" : "/";
  return (
    <header className="sticky top-0 z-40 border-b border-hairline bg-canvas/85 backdrop-blur-[14px] backdrop-saturate-[1.1]">
      <div className="mx-auto flex h-16 max-w-[1240px] items-center justify-between px-5 md:px-8">
        <a href={home} className="flex items-center gap-2.5 font-display text-[17px] font-semibold text-text-hi">
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img src="/logo.png" alt="" width={28} height={28} className="rounded-[8px]" />
          DeskMakeover
        </a>
        <nav aria-label={dict.locale === "zh" ? "主导航" : "Primary"} className="flex items-center gap-1 md:gap-2">
          <a href="#looks" className="hidden rounded-full px-3 py-2 text-[14.5px] text-text-mid transition-colors hover:text-text-hi md:block">
            {dict.nav.looks}
          </a>
          <a href="#features" className="hidden rounded-full px-3 py-2 text-[14.5px] text-text-mid transition-colors hover:text-text-hi md:block">
            {dict.nav.features}
          </a>
          <a href="#faq" className="hidden rounded-full px-3 py-2 text-[14.5px] text-text-mid transition-colors hover:text-text-hi md:block">
            {dict.nav.faq}
          </a>
          <a
            href={GITHUB_URL}
            className="hidden rounded-full p-2 text-text-mid transition-colors hover:text-text-hi sm:block"
            aria-label={dict.nav.github}
          >
            <GithubLogoIcon size={20} aria-hidden="true" />
          </a>
          <a
            href={dict.nav.langHref}
            className="rounded-full border border-hairline px-3 py-1.5 text-[13px] font-medium text-text-mid transition-colors hover:border-text-dim hover:text-text-hi"
            lang={dict.locale === "zh" ? "en" : "zh-CN"}
          >
            {dict.nav.langLabel}
          </a>
          <a
            href={RELEASE_READY ? DOWNLOAD_URL : "#download"}
            className="ml-1 rounded-full bg-coral-ink px-4 py-2 text-[13.5px] font-bold text-white transition-[filter,transform] duration-150 hover:brightness-105 active:scale-[0.97]"
          >
            {dict.nav.download}
          </a>
        </nav>
      </div>
    </header>
  );
}
