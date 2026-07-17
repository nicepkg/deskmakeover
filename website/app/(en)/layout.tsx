import type { Metadata } from "next";
import "../globals.css";
import { LocaleHtml } from "@/components/locale-html";
import { getDict } from "@/content";
import { satoshi } from "@/lib/fonts";
import { buildMetadata } from "@/lib/metadata";

const dict = getDict("en");

export const metadata: Metadata = buildMetadata(dict);

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <LocaleHtml dict={dict} fontClass={satoshi.variable}>
      {children}
    </LocaleHtml>
  );
}
