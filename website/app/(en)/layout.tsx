import type { Metadata } from "next";
import "../globals.css";
import { satoshi } from "@/lib/fonts";
import { en } from "@/content/en";
import { buildMetadata } from "@/lib/metadata";
import { jsonLdScript } from "@/lib/jsonld";
import { LANG_REDIRECT_JS } from "@/lib/lang-redirect";
import { CF_BEACON_TOKEN } from "@/lib/site";

export const metadata: Metadata = buildMetadata(en);

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={satoshi.variable}>
      <body>
        {/* blocking on purpose: routes zh-preferring first visits before paint */}
        <script dangerouslySetInnerHTML={{ __html: LANG_REDIRECT_JS }} />
        {children}
        <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: jsonLdScript(en) }} />
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
