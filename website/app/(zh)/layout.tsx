import type { Metadata } from "next";
import "../globals.css";
import { LocaleHtml } from "@/components/locale-html";
import { getDict } from "@/content";
import { satoshi } from "@/lib/fonts";
import { zhDisplay } from "@/lib/fonts-zh";
import { buildMetadata } from "@/lib/metadata";

const dict = getDict("zh");

export const metadata: Metadata = buildMetadata(dict);

export default function ZhRootLayout({ children }: { children: React.ReactNode }) {
  return (
    <LocaleHtml dict={dict} fontClass={`${satoshi.variable} ${zhDisplay.variable}`}>
      {children}
    </LocaleHtml>
  );
}
