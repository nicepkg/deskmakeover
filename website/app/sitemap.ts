import type { MetadataRoute } from "next";
import { CONTENT_UPDATED, SITE_URL } from "@/lib/site";

export const dynamic = "force-static";

export default function sitemap(): MetadataRoute.Sitemap {
  const languages = {
    en: `${SITE_URL}/`,
    "zh-CN": `${SITE_URL}/zh/`,
    "x-default": `${SITE_URL}/`,
  };
  // lastModified is a real content date maintained in source, never build time
  const lastModified = CONTENT_UPDATED;
  return [
    { url: `${SITE_URL}/`, lastModified, alternates: { languages } },
    { url: `${SITE_URL}/zh/`, lastModified, alternates: { languages } },
  ];
}
