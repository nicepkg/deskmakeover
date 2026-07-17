"use client";

import { openDownloadModal } from "./modal";

/**
 * Progressive-enhancement download CTA: server-rendered as a plain link to
 * the latest-release page (works without JS), upgraded on click to open the
 * download dialog instead. Styling is caller-supplied so every existing CTA
 * keeps its exact look.
 */
export function DownloadCta({
  href,
  className,
  children,
}: {
  href: string;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <a
      href={href}
      className={className}
      onClick={(e) => {
        e.preventDefault();
        openDownloadModal();
      }}
    >
      {children}
    </a>
  );
}
