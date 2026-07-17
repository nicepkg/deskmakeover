import { DEFAULT_LOCALE, LOCALES, localeDef } from "@/lib/locales";

/**
 * First-visit locale routing for the static export (owner decision
 * 2026-07-17, reversing the earlier no-redirect rule). Generated from the
 * locale registry: the DEFAULT-locale root page walks navigator.languages in
 * preference order and routes the first match — a non-default locale's
 * prefix redirects to its path, the default locale's prefix stays. An
 * explicit language-switch click (components/lang.tsx) stores a preference
 * that always wins, arriving from a non-default locale's own path counts as
 * an explicit choice of the default (covers pre-hydration clicks), and
 * non-default pages never auto-redirect, so shared links and crawlers keep
 * stable, indexable URLs. hreflang alternates stay the SEO source of truth.
 *
 * Inlined by LocaleHtml as a blocking <script> at the top of <body> on the
 * default locale only — no hydration, runs before first paint.
 */
const nonDefault = LOCALES.filter((l) => l.code !== DEFAULT_LOCALE);

/** navigator primary subtag -> path, e.g. { zh: "/zh/" } */
const NAV_ROUTES = Object.fromEntries(
  nonDefault.flatMap((l) => l.navPrefixes.map((p) => [p, l.path]))
);
/** stored dm-lang code -> path for non-default locales */
const PREF_ROUTES = Object.fromEntries(nonDefault.map((l) => [l.code, l.path]));
/** default-locale subtags that stop the walk */
const STOP_PREFIXES = localeDef(DEFAULT_LOCALE).navPrefixes;
/** referrer prefixes that mean "user left a non-default locale on purpose" */
const REFERRER_PREFIXES = nonDefault.map((l) => l.path.replace(/\/$/, ""));

export const LANG_REDIRECT_JS = `(function () {
  try {
    var path = location.pathname;
    if (path !== "/" && path !== "/index.html") return;
    var pref = localStorage.getItem("dm-lang");
    if (pref === ${JSON.stringify(DEFAULT_LOCALE)}) return;
    var refs = ${JSON.stringify(REFERRER_PREFIXES)};
    for (var r = 0; r < refs.length; r++) {
      if (document.referrer && document.referrer.indexOf(location.origin + refs[r]) === 0) {
        try { localStorage.setItem("dm-lang", ${JSON.stringify(DEFAULT_LOCALE)}); } catch (e) {}
        return;
      }
    }
    var prefRoutes = ${JSON.stringify(PREF_ROUTES)};
    if (pref && prefRoutes[pref]) { location.replace(prefRoutes[pref]); return; }
    var navRoutes = ${JSON.stringify(NAV_ROUTES)};
    var stops = ${JSON.stringify(STOP_PREFIXES)};
    var langs = navigator.languages && navigator.languages.length
      ? navigator.languages
      : [navigator.language || ""];
    for (var i = 0; i < langs.length; i++) {
      var base = String(langs[i]).toLowerCase().split("-")[0];
      if (navRoutes[base]) { location.replace(navRoutes[base]); return; }
      for (var s = 0; s < stops.length; s++) if (base === stops[s]) return;
    }
  } catch (e) { /* never block the page over locale sniffing */ }
})();`;
