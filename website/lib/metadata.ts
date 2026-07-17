import type { Metadata } from "next";
import type { Dict } from "@/content/types";
import { SITE_URL } from "@/lib/site";

export function buildMetadata(dict: Dict): Metadata {
  const path = dict.locale === "zh" ? "/zh/" : "/";
  return {
    metadataBase: new URL(SITE_URL),
    title: dict.meta.title,
    description: dict.meta.description,
    alternates: {
      canonical: path,
      languages: {
        en: "/",
        "zh-CN": "/zh/",
        "x-default": "/",
      },
    },
    openGraph: {
      type: "website",
      url: path,
      siteName: "DeskMakeover",
      title: dict.meta.title,
      description: dict.meta.description,
      locale: dict.locale === "zh" ? "zh_CN" : "en_US",
      alternateLocale: [dict.locale === "zh" ? "en_US" : "zh_CN"],
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
