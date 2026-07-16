import type { Dict } from "@/content/types";
import { SiteNav } from "@/components/site-nav";
import { Hero } from "@/components/hero";
import {
  ProofSection,
  LooksSection,
  ZonesSection,
  StudioSection,
  DownloadSection,
  FaqSection,
} from "@/components/sections";
import { SiteFooter } from "@/components/site-footer";

export function Landing({ dict }: { dict: Dict }) {
  return (
    <>
      <SiteNav dict={dict} />
      <main>
        <Hero dict={dict} />
        <ProofSection dict={dict} />
        <LooksSection dict={dict} />
        <ZonesSection dict={dict} />
        <StudioSection dict={dict} />
        <DownloadSection dict={dict} />
        <FaqSection dict={dict} />
      </main>
      <SiteFooter dict={dict} />
    </>
  );
}
