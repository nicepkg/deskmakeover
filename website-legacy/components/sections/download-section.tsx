import type { Dict } from "@/content/types";
import { DownloadCta } from "@/components/download-cta";
import { Reveal } from "@/components/reveal";

export function DownloadSection({ dict }: { dict: Dict }) {
  const d = dict.download;
  return (
    <section id="download" className="scroll-mt-20">
      <div className="mx-auto max-w-[1240px] px-5 pb-[clamp(5rem,10vw,8rem)] md:px-8">
        <Reveal>
          <div className="mx-auto max-w-[720px] rounded-card border border-hairline bg-surface-1 px-6 py-12 text-center md:px-12">
            <h2 className="font-display text-[clamp(2rem,3.6vw,3rem)] font-semibold leading-[1.05] tracking-[-0.02em]">
              {d.title}
            </h2>
            <p className="mx-auto mt-3 max-w-[48ch] text-[16px] text-text-mid">{d.body}</p>
            <div className="mt-7">
              <DownloadCta dict={dict} />
            </div>
          </div>
        </Reveal>
      </div>
    </section>
  );
}
