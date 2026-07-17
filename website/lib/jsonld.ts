import type { Dict } from "@/content/types";
import { localeDef, pageUrl as localePageUrl, siteLanguages } from "@/lib/locales";
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
        // version should come from the actual published release, not a guess
        ...(RELEASE_READY ? { downloadUrl: RELEASES_LATEST_URL } : {}),
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
