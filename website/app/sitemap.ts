import type { MetadataRoute } from "next";
import { STORY_META } from "@/content/story";
import { LOCALES, alternateUrls, pageUrl } from "@/lib/locales";
import { LAST_CONTENT_DATE, SITE_URL } from "@/lib/site";

export const dynamic = "force-static";

export default function sitemap(): MetadataRoute.Sitemap {
  const languages = alternateUrls();
  const engineLanguages = Object.fromEntries(
    Object.entries(languages).map(([k, v]) => [k, `${v}engine/`]),
  );
  // real dates (hand-maintained content date or release date), never build time
  return [
    ...LOCALES.map((l) => ({
      url: pageUrl(l.code),
      lastModified: LAST_CONTENT_DATE,
      alternates: { languages },
    })),
    ...LOCALES.map((l) => ({
      url: `${pageUrl(l.code)}engine/`,
      lastModified: LAST_CONTENT_DATE,
      alternates: { languages: engineLanguages },
    })),
    // /story/ is a single-language document page — no hreflang alternates
    {
      url: `${SITE_URL}${STORY_META.path}`,
      lastModified: STORY_META.datePublished,
    },
  ];
}
