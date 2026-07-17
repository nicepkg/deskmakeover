"use client";

import { useEffect, useRef, useState } from "react";
import type { Dict } from "@/content/types";
import { RELEASE, type ReleaseEntry } from "@/lib/release";
import { RELEASE_MIRRORS, mirrorUrl } from "@/lib/site";
import { detectOs, type OsKind } from "./detect";

/** Any download CTA dispatches this; the single modal host listens. */
export const OPEN_DOWNLOAD_EVENT = "dm-open-download";

export function openDownloadModal() {
  window.dispatchEvent(new CustomEvent(OPEN_DOWNLOAD_EVENT));
}

type DeviceKey = keyof Dict["downloadModal"]["device"];

const DEVICE_KEY: Record<OsKind, DeviceKey | null> = {
  "win-x64": "win-x64",
  "win-unknown": "win-unknown",
  "win-arm": "win-arm",
  "win-32": "win-32",
  "win-old": "win-old",
  mac: "desktop-other",
  linux: "desktop-other",
  mobile: "mobile",
  unknown: null,
};

/** teal check for full support, gold caution otherwise */
const DEVICE_TONE: Record<DeviceKey, "ok" | "warn"> = {
  "win-x64": "ok",
  "win-unknown": "ok",
  "win-arm": "warn",
  "win-32": "warn",
  "win-old": "warn",
  "desktop-other": "warn",
  mobile: "warn",
};

function DeviceLine({ dict, os }: { dict: Dict; os: OsKind | null }) {
  if (!os) return null;
  const key = DEVICE_KEY[os];
  if (!key) return null;
  const ok = DEVICE_TONE[key] === "ok";
  return (
    <p className="flex items-start gap-2 text-[13px] leading-[1.55] text-ink-2">
      <svg
        viewBox="0 0 14 14"
        className="mt-[3px] h-3.5 w-3.5 flex-none"
        fill="none"
        stroke={ok ? "var(--color-teal)" : "var(--color-gold)"}
        strokeWidth="1.6"
        aria-hidden
      >
        {ok ? (
          <path d="M2.5 7.5 5.5 10.5 11.5 3.5" />
        ) : (
          <path d="M7 2v6.2M7 11.4v.2" strokeLinecap="square" />
        )}
      </svg>
      {dict.downloadModal.device[key]}
    </p>
  );
}

function MirrorLinks({
  dict,
  url,
  sizeMB,
  onStart,
  prominent,
}: {
  dict: Dict;
  url: string;
  sizeMB: number;
  onStart: () => void;
  prominent: boolean;
}) {
  const m = dict.downloadModal;
  const note = m.mirrorNote.replace("{size}", String(sizeMB));
  if (prominent) {
    return (
      <div>
        <p className="font-mono text-[11px] tracking-[0.1em] text-ink-3">{m.mirrorsLead}</p>
        <div className="mt-2 grid grid-cols-2 gap-px border border-line bg-line">
          {RELEASE_MIRRORS.map((mir, i) => (
            <a
              key={mir.id}
              href={mirrorUrl(mir.base, url)}
              onClick={onStart}
              className="bg-card px-3 py-2.5 text-center transition-colors hover:bg-panel"
            >
              <span className="block text-[13px] font-semibold text-ink">线路{i === 0 ? "一" : "二"}</span>
              <span className="mt-0.5 block font-mono text-[10.5px] text-ink-3">{mir.label}</span>
            </a>
          ))}
        </div>
        <p className="mt-2 text-[11.5px] leading-[1.5] text-ink-3">{note}</p>
      </div>
    );
  }
  return (
    <p className="text-[12px] leading-[1.6] text-ink-3">
      {m.mirrorsLead}
      {": "}
      {RELEASE_MIRRORS.map((mir, i) => (
        <span key={mir.id}>
          {i > 0 && " · "}
          <a
            href={mirrorUrl(mir.base, url)}
            onClick={onStart}
            className="font-mono text-ink-2 underline decoration-line underline-offset-2 transition-colors hover:text-ink"
          >
            {mir.label}
          </a>
        </span>
      ))}
    </p>
  );
}

/**
 * The click-to-download dialog. One host per page, opened by any CTA via
 * openDownloadModal(). All release facts are baked in at build time
 * (lib/release.ts) — the dialog is fully static, downloads are direct asset
 * links (never a trip to the Releases page). Device support is advisory:
 * detection never blocks the download.
 */
export function DownloadModal({ dict }: { dict: Dict }) {
  const ref = useRef<HTMLDialogElement>(null);
  const [os, setOs] = useState<OsKind | null>(null);
  const [started, setStarted] = useState(false);
  const [copied, setCopied] = useState(false);
  const m = dict.downloadModal;
  const zh = dict.locale === "zh";
  const current = RELEASE;
  const history: ReleaseEntry[] = (RELEASE.releases ?? []).slice(1);

  useEffect(() => {
    const onOpen = () => {
      const el = ref.current;
      if (!el || el.open) return;
      el.showModal();
      void detectOs().then(setOs);
    };
    window.addEventListener(OPEN_DOWNLOAD_EVENT, onOpen);
    return () => window.removeEventListener(OPEN_DOWNLOAD_EVENT, onOpen);
  }, []);

  if (!current.ready || !current.installer) return null;
  const asset = current.installer;

  const close = () => ref.current?.close();
  const onStart = () => setStarted(true);

  return (
    <dialog
      ref={ref}
      className="dm-modal m-auto w-[min(560px,calc(100vw-2rem))] border border-line bg-canvas p-0 text-ink-2 backdrop:bg-transparent"
      onClick={(e) => {
        if (e.target === ref.current) close();
      }}
      aria-label={m.title}
    >
      <div className="dm-modal-body p-6 md:p-8">
        <div className="flex items-start justify-between gap-4">
          <div>
            <p className="font-mono text-[11px] tracking-[0.2em] text-coral-ink">
              DOWNLOAD · {current.tag?.toUpperCase()}
            </p>
            <h2 className="mt-2 text-[24px] font-bold leading-[1.15] tracking-[-0.01em] md:text-[27px]">
              {m.title}
            </h2>
          </div>
          <button
            type="button"
            onClick={close}
            aria-label={m.close}
            className="grid h-8 w-8 flex-none place-items-center border border-line text-ink-3 transition-colors hover:border-ink-3 hover:text-ink"
          >
            <svg viewBox="0 0 14 14" className="h-3.5 w-3.5" stroke="currentColor" strokeWidth="1.5" aria-hidden>
              <path d="M2 2l10 10M12 2 2 12" />
            </svg>
          </button>
        </div>

        <div className="mt-4 min-h-[20px]">
          <DeviceLine dict={dict} os={os} />
        </div>

        {/* current version — the one obvious action */}
        <div className="mt-4 border border-line bg-card p-4 md:p-5">
          <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
            <span className="break-all font-mono text-[12.5px] text-ink">{asset.name}</span>
            <span className="font-mono text-[11.5px] text-ink-3 tabular-nums">
              {current.tag} · {current.publishedAt} · {asset.sizeMB} MB
            </span>
          </div>
          {os === "mobile" ? (
            <>
              <button
                type="button"
                onClick={() => {
                  navigator.clipboard?.writeText(asset.url).then(() => setCopied(true));
                }}
                className="mt-4 flex w-full items-center justify-center gap-2.5 bg-coral px-6 py-3 text-[15px] font-semibold text-white transition-colors hover:bg-coral-deep"
              >
                <svg viewBox="0 0 14 14" className="h-3.5 w-3.5" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden>
                  <rect x="4.5" y="4.5" width="8" height="8" />
                  <path d="M9.5 4.5v-3h-8v8h3" />
                </svg>
                {copied ? m.mobileCopied : m.mobileCopyCta}
              </button>
              <a
                href={asset.url}
                onClick={onStart}
                className="mt-2.5 block text-center font-mono text-[11.5px] text-ink-3 underline decoration-line underline-offset-2 transition-colors hover:text-ink"
              >
                {m.mobileStillDownload}
              </a>
            </>
          ) : (
            <>
              <a
                href={asset.url}
                onClick={onStart}
                className="mt-4 flex w-full items-center justify-center gap-2.5 bg-coral px-6 py-3 text-[15px] font-semibold text-white transition-colors hover:bg-coral-deep"
              >
                <svg viewBox="0 0 14 14" className="h-3.5 w-3.5" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden>
                  <path d="M7 1.5v8m0 0L3.8 6.3M7 9.5l3.2-3.2M1.8 12.5h10.4" />
                </svg>
                {m.primaryCta}
              </a>
              <p className="mt-2 text-center font-mono text-[10.5px] tracking-[0.08em] text-ink-3">
                {m.viaGithub}
              </p>
            </>
          )}
        </div>

        {/* mirrors: prominent for zh, one quiet line for en */}
        <div className="mt-5">
          <MirrorLinks dict={dict} url={asset.url} sizeMB={asset.sizeMB} onStart={onStart} prominent={zh} />
        </div>

        {/* SmartScreen walkthrough appears once a download actually starts */}
        {started ? (
          <div className="mt-5 border border-line bg-panel px-4 py-3 text-[12.5px] leading-[1.65] text-ink-2">
            {m.smartscreenStarted}
          </div>
        ) : null}

        <div className="mt-5 flex flex-wrap items-center justify-between gap-3 border-t border-line pt-4">
          <a
            href={current.notesUrl}
            target="_blank"
            rel="noreferrer"
            className="font-mono text-[11.5px] tracking-[0.08em] text-ink-3 transition-colors hover:text-ink"
          >
            {m.releaseNotes} ↗
          </a>
          {history.length > 0 ? (
            <span className="font-mono text-[11.5px] text-ink-3">
              {m.historyLabel} · {history.length}
            </span>
          ) : null}
        </div>

        {history.length > 0 ? (
          <details className="group mt-3">
            <summary className="cursor-pointer list-none font-mono text-[12px] tracking-[0.08em] text-ink-2 transition-colors hover:text-ink">
              <span className="mr-1.5 inline-block transition-transform group-open:rotate-90">▸</span>
              {m.historyLabel}
            </summary>
            <div className="mt-3 border border-line">
              {history.map((r, i) => (
                <div
                  key={r.tag}
                  className={`flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 px-4 py-2.5 ${i > 0 ? "border-t border-line" : ""}`}
                >
                  <span className="font-mono text-[12.5px] text-ink tabular-nums">{r.tag}</span>
                  <span className="font-mono text-[11px] text-ink-3 tabular-nums">
                    {r.publishedAt}
                    {r.installer ? ` · ${r.installer.sizeMB} MB` : ""}
                  </span>
                  <span className="flex items-center gap-3 font-mono text-[11.5px]">
                    {r.installer ? (
                      <>
                        <a
                          href={r.installer.url}
                          onClick={onStart}
                          className="text-coral-ink transition-colors hover:text-coral-deep"
                        >
                          {m.primaryCta}
                        </a>
                        {zh ? (
                          <a
                            href={mirrorUrl(RELEASE_MIRRORS[0].base, r.installer.url)}
                            onClick={onStart}
                            className="text-ink-3 transition-colors hover:text-ink"
                          >
                            加速
                          </a>
                        ) : null}
                      </>
                    ) : (
                      <a
                        href={r.notesUrl}
                        target="_blank"
                        rel="noreferrer"
                        className="text-ink-3 transition-colors hover:text-ink"
                      >
                        {m.releaseNotes}
                      </a>
                    )}
                  </span>
                </div>
              ))}
            </div>
          </details>
        ) : null}
      </div>
    </dialog>
  );
}
