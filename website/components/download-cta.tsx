"use client";

import { useEffect, useState } from "react";
import { CheckIcon, CopyIcon, DownloadSimpleIcon, EnvelopeSimpleIcon, GithubLogoIcon } from "@/components/icons";
import type { Dict } from "@/content/types";
import { DOWNLOAD_URL, RELEASE_READY, SITE_URL } from "@/lib/site";

/**
 * The download moment. Dual build state (pre-release capture vs
 * releases/latest) plus a client-side non-Windows fallback: copy the link or
 * mail it to yourself. Server render assumes Windows so the no-JS path still
 * shows the full download UI.
 */
export function DownloadCta({ dict }: { dict: Dict }) {
  const d = dict.download;
  const [platform, setPlatform] = useState<"windows" | "other">("windows");
  const [copied, setCopied] = useState(false);
  const pageUrl = dict.locale === "zh" ? `${SITE_URL}/zh/` : `${SITE_URL}/`;

  useEffect(() => {
    const ua = navigator.userAgent;
    if (!/Windows/i.test(ua)) setPlatform("other");
  }, []);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(pageUrl);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      /* clipboard unavailable; the mailto path remains */
    }
  };

  const mailto = `mailto:?subject=${encodeURIComponent(d.mailSubject)}&body=${encodeURIComponent(d.mailBody)}`;

  return (
    <div className="flex flex-col items-center">
      <a
        href={DOWNLOAD_URL}
        data-analytics="download"
        className="inline-flex items-center gap-2.5 rounded-btn bg-gradient-to-br from-coral to-coral-deep px-7 py-3.5 text-[17px] font-semibold text-cream shadow-lift transition-transform duration-150 hover:brightness-[1.05] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-coral active:scale-[0.98]"
      >
        {RELEASE_READY ? (
          <DownloadSimpleIcon size={20} aria-hidden="true" />
        ) : (
          <GithubLogoIcon size={20} aria-hidden="true" />
        )}
        {RELEASE_READY ? d.ctaRelease : d.ctaPending}
      </a>
      {!RELEASE_READY && <p className="mt-3 max-w-[46ch] text-center text-[14px] text-ink-soft">{d.pendingNote}</p>}
      <div className="mt-5 max-w-[52ch] rounded-card border border-hairline bg-mist px-5 py-4 text-left">
        <p className="text-[14px] font-medium">{d.smartscreenLead}</p>
        <p className="mt-1.5 text-[14px] text-ink-soft">{d.smartscreenDetail}</p>
      </div>
      <p className="mt-4 text-[13px] text-ink-soft">{d.requirements}</p>
      {platform === "other" && (
        <div className="mt-6 w-full max-w-[52ch] rounded-card border border-hairline bg-paper px-5 py-4 text-center">
          <p className="text-[14px] text-ink-soft">{d.nonWindowsNote}</p>
          <div className="mt-3 flex flex-wrap items-center justify-center gap-3">
            <button
              type="button"
              onClick={copy}
              className="inline-flex items-center gap-1.5 rounded-btn border border-hairline px-4 py-2 text-[14px] font-medium transition-colors hover:border-coral hover:text-coral-text"
            >
              {copied ? <CheckIcon size={16} aria-hidden="true" /> : <CopyIcon size={16} aria-hidden="true" />}
              {copied ? d.copied : d.copyLink}
            </button>
            <a
              href={mailto}
              className="inline-flex items-center gap-1.5 rounded-btn border border-hairline px-4 py-2 text-[14px] font-medium transition-colors hover:border-coral hover:text-coral-text"
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
