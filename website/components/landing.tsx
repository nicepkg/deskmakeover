import type { Dict } from "@/content/types";
import { SiteNav } from "@/components/site-nav";
import { Hero, SpecStrip } from "@/components/hero";
import {
  ProofSection,
  LooksSection,
  ZonesSection,
  StudioSection,
  DownloadSection,
  FaqSection,
} from "@/components/sections";
import { SiteFooter } from "@/components/site-footer";
import { EngineBand } from "@/components/engine-band";
import { DownloadModal } from "@/components/download/modal";

export function Landing({ dict }: { dict: Dict }) {
  return (
    <>
      <SiteNav dict={dict} />
      <main>
        <Hero dict={dict} />
        <SpecStrip dict={dict} />
        <ProofSection dict={dict} />
        <LooksSection dict={dict} />
        <ZonesSection dict={dict} />
        <StudioSection dict={dict} />
        <EngineBand dict={dict} />
        <DownloadSection dict={dict} />
        <FaqSection dict={dict} />
      </main>
      <SiteFooter dict={dict} />
      <DownloadModal dict={dict} />
    </>
  );
}
