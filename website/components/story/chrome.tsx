import { DownloadCta } from "@/components/download/button";
import { ThemeToggle } from "@/components/theme-toggle";
import { DOWNLOAD_URL, GITHUB_URL, RELEASE_READY } from "@/lib/site";

/*
 * Page chrome for /story/ — same visual language as the landing SiteNav /
 * SiteFooter, but this is a single-language document page: no locale switch,
 * no section anchors; the logo and the home link route back to the landing
 * ("/" lets the locale script pick the visitor's language).
 */

export function StoryNav() {
  return (
    <header className="sticky top-0 z-50 border-b border-line bg-canvas/85 backdrop-blur-sm">
      <div className="mx-auto flex h-14 max-w-[1200px] items-center justify-between px-5 md:px-8">
        <a href="/" className="flex items-center gap-2.5">
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img src="/logo.png" alt="" width={22} height={22} className="h-[22px] w-[22px]" />
          <span className="hidden font-display text-[15px] font-semibold tracking-tight text-ink sm:inline">
            DeskMakeover
          </span>
          <span className="hidden font-mono text-[11px] tracking-[0.12em] text-ink-3 sm:inline">
            / 创作历程
          </span>
        </a>
        <div className="flex items-center gap-4">
          <a
            href={GITHUB_URL}
            target="_blank"
            rel="noreferrer"
            className="hidden font-mono text-[12px] tracking-[0.12em] text-ink-2 transition-colors hover:text-ink sm:inline"
          >
            GITHUB
          </a>
          <ThemeToggle />
          {RELEASE_READY ? (
            <DownloadCta
              href={DOWNLOAD_URL}
              className="inline-flex h-8 items-center bg-ink px-3.5 text-[13px] font-semibold text-canvas transition-colors hover:bg-coral-deep hover:text-white"
            >
              下载
            </DownloadCta>
          ) : (
            <a
              href="/#download"
              className="inline-flex h-8 items-center bg-ink px-3.5 text-[13px] font-semibold text-canvas transition-colors hover:bg-coral-deep hover:text-white"
            >
              下载
            </a>
          )}
        </div>
      </div>
    </header>
  );
}

export function StoryFooter() {
  return (
    <footer className="border-t border-line">
      <div className="mx-auto max-w-[1200px] px-5 py-12 md:px-8">
        <p className="text-[12.5px] leading-[1.75] text-ink-3">
          <b className="font-semibold text-ink-2">方法</b>：源自一次跨九天的 Claude Code
          会话（原始 570MB / 63,439 行）。从 12,325 条 user
          记录中剔除工具结果、上下文压缩摘要、任务通知与协作智能体消息等系统注入内容，得到 341
          条真人发言（64,427 字）为分析底本。词频与情绪基于精选领域词库统计，非通用分词。工作项目：DeskMakeover（Windows
          桌面美化）。
          <br />
          <b className="font-semibold text-ink-2">配色</b>取自 DeskMakeover
          自身的珊瑚色设计系统 · 全量发言原文另存于本地语料档案。
        </p>
        <div className="mt-8 flex flex-wrap items-center gap-x-7 gap-y-2 font-mono text-[12px] tracking-[0.1em] text-ink-2">
          <a href="/" className="transition-colors hover:text-ink">
            官网首页
          </a>
          <a
            href={GITHUB_URL}
            target="_blank"
            rel="noreferrer"
            className="transition-colors hover:text-ink"
          >
            GitHub
          </a>
          <span className="text-ink-3">© 2026 nicepkg</span>
        </div>
      </div>
    </footer>
  );
}
