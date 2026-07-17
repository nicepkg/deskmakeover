import type { MetadataRoute } from "next";
import { LOCALES, alternateUrls, pageUrl } from "@/lib/locales";
import { CONTENT_UPDATED } from "@/lib/site";

export const dynamic = "force-static";

export default function sitemap(): MetadataRoute.Sitemap {
  const languages = alternateUrls();
  // lastModified is a real content date maintained in source, never build time
  return LOCALES.map((l) => ({
    url: pageUrl(l.code),
    lastModified: CONTENT_UPDATED,
    alternates: { languages },
  }));
}
