import type { Metadata } from "next";
import "../globals.css";
import { STORY_META } from "@/content/story";
import { satoshi } from "@/lib/fonts";
import { storyZhDisplay } from "@/lib/fonts-story";
import { storyJsonLdScript } from "@/lib/jsonld";
import { CF_BEACON_TOKEN, SITE_URL } from "@/lib/site";
import { THEME_INIT_JS } from "@/lib/theme";

/**
 * Root layout for /story/ — a single-language (zh) document page outside the
 * locale trees: no hreflang alternates, no locale redirect script, its own
 * Article JSON-LD instead of the landing's Software/FAQ graph.
 */

export const metadata: Metadata = {
  metadataBase: new URL(SITE_URL),
  title: STORY_META.title,
  description: STORY_META.description,
  alternates: { canonical: STORY_META.path },
  openGraph: {
    type: "article",
    url: STORY_META.path,
    siteName: "DeskMakeover",
    title: STORY_META.title,
    description: STORY_META.description,
    locale: "zh_CN",
    publishedTime: STORY_META.datePublished,
    images: [{ url: "/social-card.png", width: 1280, height: 640, alt: STORY_META.title }],
  },
  twitter: {
    card: "summary_large_image",
    title: STORY_META.title,
    description: STORY_META.description,
    images: [{ url: "/social-card.png", alt: STORY_META.title }],
  },
  icons: {
    icon: "/logo.png",
    apple: "/logo.png",
  },
};

export default function StoryRootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="zh-CN" className={`${satoshi.variable} ${storyZhDisplay.variable}`}>
      <body>
        {/* blocking on purpose: stamps an explicit theme choice before paint */}
        <script dangerouslySetInnerHTML={{ __html: THEME_INIT_JS }} />
        {children}
        <script
          type="application/ld+json"
          dangerouslySetInnerHTML={{ __html: storyJsonLdScript(STORY_META) }}
        />
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
