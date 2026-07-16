import type { Dict } from "@/content/types";
import { DOWNLOAD_URL, GITHUB_URL, SITE_URL } from "@/lib/site";

export function softwareApplicationJsonLd(dict: Dict) {
  return {
    "@context": "https://schema.org",
    "@type": "SoftwareApplication",
    name: "DeskMakeover",
    alternateName: "桌面美颜",
    description: dict.meta.description,
    applicationCategory: "UtilitiesApplication",
    operatingSystem: "Windows 10, Windows 11",
    softwareVersion: "0.1.0-beta",
    url: dict.locale === "zh" ? `${SITE_URL}/zh/` : `${SITE_URL}/`,
    image: `${SITE_URL}/social-card.png`,
    downloadUrl: DOWNLOAD_URL,
    license: `${GITHUB_URL}/blob/main/LICENSE`,
    isAccessibleForFree: true,
    offers: {
      "@type": "Offer",
      price: "0",
      priceCurrency: "USD",
    },
    author: {
      "@type": "Person",
      name: "Jinming Yang",
      url: "https://github.com/2214962083",
    },
    inLanguage: dict.locale === "zh" ? "zh-CN" : "en",
  };
}

export function faqPageJsonLd(dict: Dict) {
  return {
    "@context": "https://schema.org",
    "@type": "FAQPage",
    inLanguage: dict.locale === "zh" ? "zh-CN" : "en",
    mainEntity: dict.faq.items.map((item) => ({
      "@type": "Question",
      name: item.q,
      acceptedAnswer: {
        "@type": "Answer",
        text: item.a,
      },
    })),
  };
}

export function jsonLdScript(dict: Dict): string {
  return JSON.stringify([softwareApplicationJsonLd(dict), faqPageJsonLd(dict)]);
}
