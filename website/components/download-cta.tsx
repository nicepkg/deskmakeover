"use client";

import { useEffect, useRef, useState } from "react";
import { CheckIcon, CopyIcon, DownloadSimpleIcon, EnvelopeSimpleIcon, GithubLogoIcon } from "@/components/icons";
import type { Dict } from "@/content/types";
import { DOWNLOAD_URL, RELEASE_READY, SITE_URL } from "@/lib/site";

/**
 * The download moment. Dual build state (pre-release capture vs
 * releases/latest). Non-Windows clients: pre-release the GitHub capture CTA
 * works everywhere so it stays; post-release the useless .exe button swaps
 * for copy-link / mail-yourself. Server render (and no-JS) shows the full
 * download UI.
 */
export function DownloadCta({ dict }: { dict: Dict }) {
  const d = dict.download;
  const [platform, setPlatform] = useState<"windows" | "other">("windows");
  const [copied, setCopied] = useState(false);
  const copyTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pageUrl = dict.locale === "zh" ? `${SITE_URL}/zh/` : `${SITE_URL}/`;

  useEffect(() => {
    if (!/Windows/i.test(navigator.userAgent)) setPlatform("other");
    return () => {
      if (copyTimer.current) clearTimeout(copyTimer.current);
    };
  }, []);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(pageUrl);
      setCopied(true);
      if (copyTimer.current) clearTimeout(copyTimer.current);
      copyTimer.current = setTimeout(() => setCopied(false), 2000);
    } catch {
      /* clipboard unavailable; the mailto path remains */
    }
  };

  const mailto = `mailto:?subject=${encodeURIComponent(d.mailSubject)}&body=${encodeURIComponent(d.mailBody)}`;
  const hideDownload = RELEASE_READY && platform === "other";

  return (
    <div className="flex flex-col items-center">
      {!hideDownload && (
        <a
          href={DOWNLOAD_URL}
          data-analytics="download"
          className="inline-flex items-center gap-2.5 rounded-full bg-gradient-to-br from-coral-deep to-coral-ink px-8 py-4 text-[19px] font-bold text-white shadow-[0_8px_32px_-8px_var(--coral-glow)] transition-[filter,transform] duration-150 hover:brightness-105 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-coral active:scale-[0.98]"
        >
          {RELEASE_READY ? <DownloadSimpleIcon size={20} aria-hidden="true" /> : <GithubLogoIcon size={20} aria-hidden="true" />}
          {RELEASE_READY ? d.ctaRelease : d.ctaPending}
        </a>
      )}
      {!RELEASE_READY && <p className="mt-3 max-w-[46ch] text-center text-[13.5px] text-text-dim">{d.pendingNote}</p>}
      <p className="mt-3 text-[13px] text-text-dim">{d.requirements}</p>
      {platform === "other" && (
        <div className="mt-5 w-full max-w-[52ch] rounded-card border border-hairline bg-surface-2 px-5 py-4 text-center">
          <p className="text-[14px] text-text-mid">{d.nonWindowsNote}</p>
          <div className="mt-3 flex flex-wrap items-center justify-center gap-3">
            <button
              type="button"
              onClick={copy}
              className="inline-flex items-center gap-1.5 rounded-full border border-hairline px-4 py-2 text-[14px] font-medium text-text-mid transition-colors hover:border-coral hover:text-text-hi"
            >
              {copied ? <CheckIcon size={16} aria-hidden="true" /> : <CopyIcon size={16} aria-hidden="true" />}
              {copied ? d.copied : d.copyLink}
            </button>
            <a
              href={mailto}
              className="inline-flex items-center gap-1.5 rounded-full border border-hairline px-4 py-2 text-[14px] font-medium text-text-mid transition-colors hover:border-coral hover:text-text-hi"
            >
              <EnvelopeSimpleIcon size={16} aria-hidden="true" />
              {d.mailLink}
            </a>
          </div>
        </div>
      )}
    </div>
  );
}
