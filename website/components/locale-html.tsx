import type { Dict } from "@/content/types";
import { jsonLdScript } from "@/lib/jsonld";
import { LANG_REDIRECT_JS } from "@/lib/lang-redirect";
import { DEFAULT_LOCALE, localeDef } from "@/lib/locales";
import { CF_BEACON_TOKEN } from "@/lib/site";
import { THEME_INIT_JS } from "@/lib/theme";

/**
 * Shared root document for every locale tree. Locale layouts stay thin
 * wrappers: they pick the dict and compose fonts (fonts remain per-layout so
 * next/font preloads a locale-specific face only on that locale's pages).
 */
export function LocaleHtml({
  dict,
  fontClass,
  children,
}: {
  dict: Dict;
  fontClass: string;
  children: React.ReactNode;
}) {
  return (
    <html lang={localeDef(dict.locale).htmlLang} className={fontClass}>
      <body>
        {/* blocking on purpose: stamps an explicit theme choice before paint */}
        <script dangerouslySetInnerHTML={{ __html: THEME_INIT_JS }} />
        {dict.locale === DEFAULT_LOCALE ? (
          // blocking on purpose: routes first visits before paint
          <script dangerouslySetInnerHTML={{ __html: LANG_REDIRECT_JS }} />
        ) : null}
        {children}
        <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: jsonLdScript(dict) }} />
        {CF_BEACON_TOKEN ? (
          <script
            defer
            src="https://static.cloudflareinsights.com/beacon.min.js"
            data-cf-beacon={JSON.stringify({ token: CF_BEACON_TOKEN })}
          />
        ) : null}
      </body>
    </html>
  );
}
