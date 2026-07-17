import type { Metadata } from "next";
import type { Dict } from "@/content/types";
import { alternatePaths, localeDef, ogAlternates } from "@/lib/locales";
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
