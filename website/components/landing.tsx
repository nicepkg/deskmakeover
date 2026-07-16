import type { Dict } from "@/content/types";
import { SiteHeader } from "@/components/site-header";
import { SiteFooter } from "@/components/site-footer";
import { Hero } from "@/components/sections/hero";
import { Promise as PromiseSection } from "@/components/sections/promise";
import { Looks } from "@/components/sections/looks";
import { Customize } from "@/components/sections/customize";
import { Zones } from "@/components/sections/zones";
import { Studio } from "@/components/sections/studio";
import { DownloadSection } from "@/components/sections/download-section";
import { Faq } from "@/components/sections/faq";

export function Landing({ dict }: { dict: Dict }) {
  return (
    <>
      <SiteHeader dict={dict} />
      <main>
        <Hero dict={dict} />
        <PromiseSection dict={dict} />
        <Looks dict={dict} />
        <Customize dict={dict} />
        <Zones dict={dict} />
        <Studio dict={dict} />
        <DownloadSection dict={dict} />
        <Faq dict={dict} />
      </main>
      <SiteFooter dict={dict} />
    </>
  );
}
