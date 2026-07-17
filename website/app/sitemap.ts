import type { MetadataRoute } from "next";
import { LOCALES, alternateUrls, pageUrl } from "@/lib/locales";
import { LAST_CONTENT_DATE } from "@/lib/site";

export const dynamic = "force-static";

export default function sitemap(): MetadataRoute.Sitemap {
  const languages = alternateUrls();
  // a real date (hand-maintained content date or release date), never build time
  return LOCALES.map((l) => ({
    url: pageUrl(l.code),
    lastModified: LAST_CONTENT_DATE,
    alternates: { languages },
  }));
}
