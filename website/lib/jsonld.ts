import type { Dict } from "@/content/types";
import { localeDef, pageUrl as localePageUrl, siteLanguages } from "@/lib/locales";
import { RELEASE } from "@/lib/release";
import { GITHUB_URL, RELEASES_LATEST_URL, RELEASE_READY, SITE_URL } from "@/lib/site";

/**
 * One connected @graph per page: Organization -> WebSite -> SoftwareApplication
 * -> FAQPage, joined by stable @id references so search and answer engines can
 * link the product, the site and the GitHub project as one entity.
 *
 * Release facts (downloadUrl, softwareVersion) are emitted only when an
 * installer actually exists: publishing a version or a download link for an
 * empty Releases page feeds engines contradictory facts.
 */
const ORG_ID = `${SITE_URL}/#organization`;
const SITE_ID = `${SITE_URL}/#website`;
const APP_ID = `${SITE_URL}/#software`;

/**
 * /story/ — the making-of report. An Article node joined to the same
 * Organization / WebSite / SoftwareApplication graph via stable @ids, so
 * engines link the build story to the product entity.
 */
export function storyJsonLdScript(meta: {
  path: string;
  title: string;
  description: string;
  datePublished: string;
}): string {
  const url = `${SITE_URL}${meta.path}`;
  return JSON.stringify({
    "@context": "https://schema.org",
    "@graph": [
      {
        "@type": "Organization",
        "@id": ORG_ID,
        name: "nicepkg",
        url: "https://github.com/nicepkg",
        logo: `${SITE_URL}/logo.png`,
        sameAs: [GITHUB_URL],
      },
      {
        "@type": "WebSite",
        "@id": SITE_ID,
        url: `${SITE_URL}/`,
        name: "DeskMakeover",
        inLanguage: siteLanguages(),
        publisher: { "@id": ORG_ID },
      },
      {
        // slim reference node — the full software facts live on the landing pages
        "@type": "SoftwareApplication",
        "@id": APP_ID,
        name: "DeskMakeover",
        alternateName: "桌面美颜",
        url: `${SITE_URL}/`,
      },
      {
        "@type": "Article",
        "@id": `${url}#article`,
        headline: meta.title,
        description: meta.description,
        inLanguage: "zh-CN",
        datePublished: meta.datePublished,
        url,
        mainEntityOfPage: url,
        image: `${SITE_URL}/social-card.png`,
        isPartOf: { "@id": SITE_ID },
        about: { "@id": APP_ID },
        publisher: { "@id": ORG_ID },
        author: {
          "@type": "Person",
          name: "Jinming Yang",
          url: "https://github.com/2214962083",
        },
      },
    ],
  });
}

/**
 * /engine/ — the pixel-engine deep-dive. A TechArticle node per locale joined
 * to the same Organization / WebSite / SoftwareApplication graph.
 */
export function engineJsonLdScript(dict: Dict, meta: { title: string; description: string }): string {
  const def = localeDef(dict.locale);
  const url = `${SITE_URL}${def.path}engine/`;
  return JSON.stringify({
    "@context": "https://schema.org",
    "@graph": [
      {
        "@type": "Organization",
        "@id": ORG_ID,
        name: "nicepkg",
        url: "https://github.com/nicepkg",
        logo: `${SITE_URL}/logo.png`,
        sameAs: [GITHUB_URL],
      },
      {
        "@type": "WebSite",
        "@id": SITE_ID,
        url: `${SITE_URL}/`,
        name: "DeskMakeover",
        inLanguage: siteLanguages(),
        publisher: { "@id": ORG_ID },
      },
      {
        "@type": "SoftwareApplication",
        "@id": APP_ID,
        name: "DeskMakeover",
        alternateName: "桌面美颜",
        url: `${SITE_URL}/`,
      },
      {
        "@type": "TechArticle",
        "@id": `${url}#article`,
        headline: meta.title,
        description: meta.description,
        inLanguage: def.hreflang,
        url,
        mainEntityOfPage: url,
        image: `${SITE_URL}/social-card.png`,
        isPartOf: { "@id": SITE_ID },
        about: { "@id": APP_ID },
        publisher: { "@id": ORG_ID },
        author: {
          "@type": "Person",
          name: "Jinming Yang",
          url: "https://github.com/2214962083",
        },
      },
    ],
  });
}

export function jsonLdScript(dict: Dict): string {
  const pageUrl = localePageUrl(dict.locale);
  const inLanguage = localeDef(dict.locale).hreflang;
  return JSON.stringify({
    "@context": "https://schema.org",
    "@graph": [
      {
        "@type": "Organization",
        "@id": ORG_ID,
        name: "nicepkg",
        url: "https://github.com/nicepkg",
        logo: `${SITE_URL}/logo.png`,
        sameAs: [GITHUB_URL],
      },
      {
        "@type": "WebSite",
        "@id": SITE_ID,
        url: `${SITE_URL}/`,
        name: "DeskMakeover",
        inLanguage: siteLanguages(),
        publisher: { "@id": ORG_ID },
      },
      {
        "@type": "SoftwareApplication",
        "@id": APP_ID,
        name: "DeskMakeover",
        alternateName: "桌面美颜",
        description: dict.meta.description,
        applicationCategory: "UtilitiesApplication",
        operatingSystem: "Windows 10, Windows 11",
        url: pageUrl,
        mainEntityOfPage: pageUrl,
        image: `${SITE_URL}/social-card.png`,
        license: `${GITHUB_URL}/blob/main/LICENSE`,
        isAccessibleForFree: true,
        offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
        publisher: { "@id": ORG_ID },
        author: {
          "@type": "Person",
          name: "Jinming Yang",
          url: "https://github.com/2214962083",
        },
        inLanguage,
        // real facts from the actual published release, resolved at build time
        ...(RELEASE_READY
          ? {
              softwareVersion: RELEASE.version,
              datePublished: RELEASE.publishedAt,
              downloadUrl: RELEASES_LATEST_URL,
              releaseNotes: RELEASE.notesUrl,
            }
          : {}),
      },
      {
        "@type": "FAQPage",
        "@id": `${pageUrl}#faq`,
        inLanguage,
        mainEntity: dict.faq.items.map((item) => ({
          "@type": "Question",
          name: item.q,
          acceptedAnswer: { "@type": "Answer", text: item.a },
        })),
      },
    ],
  });
}
