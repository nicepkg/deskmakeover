import { SITE_URL } from "@/lib/site";

/**
 * THE single source of truth for every locale this site ships.
 *
 * Adding a language (say Japanese) touches exactly three places:
 *   1. one LocaleDef entry here;
 *   2. content/ja.ts (a Dict) + its line in content/index.ts;
 *   3. app/(ja)/ja/{layout,page}.tsx — thin wrappers mirroring app/(zh)
 *      (a locale-specific display font, if any, stays in that layout so
 *      next/font only preloads it on that locale's pages).
 * Metadata alternates, og locales, the sitemap, JSON-LD language tags and
 * the first-visit redirect script all derive from this table.
 *
 * NOTE: the header/footer language switch renders one link to "the other"
 * locale; at three or more locales replace it with a small menu.
 */
export type LocaleCode = "en" | "zh";

export interface LocaleDef {
  code: LocaleCode;
  /** URL prefix; the DEFAULT locale lives at the root. */
  path: string;
  /** hreflang value: metadata alternates + sitemap key */
  hreflang: string;
  /** <html lang> */
  htmlLang: string;
  /** og:locale */
  ogLocale: string;
  /** navigator.language primary subtags that select this locale on first visit */
  navPrefixes: string[];
}

export const DEFAULT_LOCALE: LocaleCode = "en";

export const LOCALES: readonly LocaleDef[] = [
  { code: "en", path: "/", hreflang: "en", htmlLang: "en", ogLocale: "en_US", navPrefixes: ["en"] },
  { code: "zh", path: "/zh/", hreflang: "zh-CN", htmlLang: "zh-CN", ogLocale: "zh_CN", navPrefixes: ["zh"] },
];

export function localeDef(code: LocaleCode): LocaleDef {
  const def = LOCALES.find((l) => l.code === code);
  if (!def) throw new Error(`unknown locale: ${code}`);
  return def;
}

export function pagePath(code: LocaleCode): string {
  return localeDef(code).path;
}

export function pageUrl(code: LocaleCode): string {
  return `${SITE_URL}${localeDef(code).path}`;
}

/** hreflang -> relative path, incl. x-default (Next metadata alternates). */
export function alternatePaths(): Record<string, string> {
  const map: Record<string, string> = {};
  for (const l of LOCALES) map[l.hreflang] = l.path;
  map["x-default"] = pagePath(DEFAULT_LOCALE);
  return map;
}

/** hreflang -> absolute URL, incl. x-default (sitemap alternates). */
export function alternateUrls(): Record<string, string> {
  const map: Record<string, string> = {};
  for (const l of LOCALES) map[l.hreflang] = pageUrl(l.code);
  map["x-default"] = pageUrl(DEFAULT_LOCALE);
  return map;
}

/** og:locale:alternate values for a page: every other locale. */
export function ogAlternates(code: LocaleCode): string[] {
  return LOCALES.filter((l) => l.code !== code).map((l) => l.ogLocale);
}

/** Every language the site is available in (WebSite JSON-LD inLanguage). */
export function siteLanguages(): string[] {
  return LOCALES.map((l) => l.hreflang);
}
