import type { Metadata } from "next";
import type { EngineDict } from "@/content/engine-types";
import type { Dict } from "@/content/types";
import { LOCALES, DEFAULT_LOCALE, alternatePaths, localeDef, ogAlternates, pagePath } from "@/lib/locales";
import { SITE_URL } from "@/lib/site";

export function buildMetadata(dict: Dict): Metadata {
  const def = localeDef(dict.locale);
  return {
    metadataBase: new URL(SITE_URL),
    title: dict.meta.title,
    description: dict.meta.description,
    alternates: {
      canonical: def.path,
      languages: alternatePaths(),
    },
    openGraph: {
      type: "website",
      url: def.path,
      siteName: "DeskMakeover",
      title: dict.meta.title,
      description: dict.meta.description,
      locale: def.ogLocale,
      alternateLocale: ogAlternates(dict.locale),
      images: [
        {
          url: "/social-card.png",
          width: 1280,
          height: 640,
          alt: dict.meta.ogAlt,
        },
      ],
    },
    twitter: {
      card: "summary_large_image",
      title: dict.meta.title,
      description: dict.meta.description,
      images: [{ url: "/social-card.png", alt: dict.meta.ogAlt }],
    },
    icons: {
      icon: "/logo.png",
      apple: "/logo.png",
    },
  };
}

/** hreflang -> relative path for a locale subpage (e.g. "engine/"), incl. x-default. */
export function subpageAlternatePaths(sub: string): Record<string, string> {
  const map: Record<string, string> = {};
  for (const l of LOCALES) map[l.hreflang] = `${l.path}${sub}`;
  map["x-default"] = `${pagePath(DEFAULT_LOCALE)}${sub}`;
  return map;
}

/** Metadata for the /engine/ page in either locale (path: /engine/ or /zh/engine/). */
export function buildEngineMetadata(dict: Dict, engine: EngineDict): Metadata {
  const def = localeDef(dict.locale);
  const path = `${def.path}engine/`;
  return {
    metadataBase: new URL(SITE_URL),
    title: engine.meta.title,
    description: engine.meta.description,
    alternates: {
      canonical: path,
      languages: subpageAlternatePaths("engine/"),
    },
    openGraph: {
      type: "article",
      url: path,
      siteName: "DeskMakeover",
      title: engine.meta.title,
      description: engine.meta.description,
      locale: def.ogLocale,
      alternateLocale: ogAlternates(dict.locale),
      images: [
        {
          url: "/social-card.png",
          width: 1280,
          height: 640,
          alt: dict.meta.ogAlt,
        },
      ],
    },
    twitter: {
      card: "summary_large_image",
      title: engine.meta.title,
      description: engine.meta.description,
      images: [{ url: "/social-card.png", alt: dict.meta.ogAlt }],
    },
    icons: {
      icon: "/logo.png",
      apple: "/logo.png",
    },
  };
}
