import type { Metadata } from "next";
import "../globals.css";
import { satoshi } from "@/lib/fonts";
import { zhDisplay } from "@/lib/fonts-zh";
import { zh } from "@/content/zh";
import { buildMetadata } from "@/lib/metadata";
import { jsonLdScript } from "@/lib/jsonld";
import { CF_BEACON_TOKEN } from "@/lib/site";

export const metadata: Metadata = buildMetadata(zh);

export default function ZhRootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="zh-CN" className={`${satoshi.variable} ${zhDisplay.variable}`}>
      <body>
        {children}
        <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: jsonLdScript(zh) }} />
        {CF_BEACON_TOKEN ? (
          <script
            defer
            src="https://static.cloudflareinsights.com/beacon.min.js"
            data-cf-beacon={JSON.stringify({ token: CF_BEACON_TOKEN })}
          />
        ) : null}
      </body>
    </html>
  );
}
