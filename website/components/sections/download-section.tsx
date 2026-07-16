import type { Dict } from "@/content/types";
import { DownloadCta } from "@/components/download-cta";
import { Reveal } from "@/components/reveal";
import { GITHUB_URL } from "@/lib/site";

export function DownloadSection({ dict }: { dict: Dict }) {
  const d = dict.download;
  return (
    <section id="download" className="scroll-mt-20">
      <div className="mx-auto max-w-[1240px] px-5 pb-[clamp(5rem,10vw,8rem)] md:px-8">
        <Reveal>
          <div className="relative mx-auto max-w-[760px] overflow-hidden rounded-card border border-hairline bg-surface-1 px-6 py-12 text-center shadow-card md:px-12">
            <div
              aria-hidden="true"
              className="pointer-events-none absolute -top-24 left-1/2 h-48 w-[130%] -translate-x-1/2 rounded-full bg-[radial-gradient(ellipse,rgb(255_111_94/0.14),transparent_70%)]"
            />
            <h2 className="relative font-display text-[clamp(2rem,3.6vw,3rem)] font-semibold leading-[1.05] tracking-[-0.02em]">
              {d.title}
            </h2>
            <p className="relative mx-auto mt-3 max-w-[48ch] text-[16px] text-text-mid">{d.body}</p>
            <div className="relative mt-8">
              <DownloadCta dict={dict} />
            </div>
            <div className="relative mx-auto mt-8 max-w-[54ch] border-t border-hairline pt-6 text-left">
              <p className="text-[14px] font-medium text-text-mid">{d.smartscreenLead}</p>
              <p className="mt-1.5 text-[13.5px] leading-relaxed text-text-dim">{d.smartscreenDetail}</p>
              <p className="mt-4 text-[13px] text-text-dim">{dict.beta.title}: {dict.beta.body}</p>
              <p className="mt-4 font-mono text-[12.5px] text-text-dim">
                <a href={GITHUB_URL} className="transition-colors hover:text-text-mid">
                  github.com/nicepkg/deskmakeover
                </a>
                {" · MIT"}
              </p>
            </div>
          </div>
        </Reveal>
      </div>
    </section>
  );
}
