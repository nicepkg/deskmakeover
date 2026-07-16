import type { Dict } from "@/content/types";
import { DownloadCta } from "@/components/download-cta";
import { Reveal } from "@/components/reveal";

export function DownloadSection({ dict }: { dict: Dict }) {
  const d = dict.download;
  return (
    <section id="download" className="scroll-mt-20 bg-mist">
      <div className="mx-auto max-w-[1200px] px-5 py-[clamp(5rem,10vw,7.5rem)] md:px-8">
        <Reveal>
          <div className="mx-auto max-w-[62ch] rounded-card border border-hairline bg-paper px-7 py-6">
            <h3 className="text-[17px] font-semibold">{dict.beta.title}</h3>
            <p className="mt-2 text-[15px] text-ink-soft">{dict.beta.body}</p>
          </div>
        </Reveal>
        <Reveal delay={80}>
          <div className="mt-16 text-center">
            <h2 className="font-display text-[clamp(1.9rem,3.6vw,2.6rem)] font-semibold leading-[1.12] tracking-[-0.01em]">
              {d.title}
            </h2>
            <p className="mx-auto mt-3 max-w-[52ch] text-[17px] text-ink-soft">{d.body}</p>
            <div className="mt-8">
              <DownloadCta dict={dict} />
            </div>
          </div>
        </Reveal>
      </div>
    </section>
  );
}
