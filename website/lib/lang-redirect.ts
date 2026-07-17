/**
 * First-visit locale routing for the static export (owner decision
 * 2026-07-17, reversing the earlier no-redirect rule): the root page sniffs
 * the browser language list and sends zh-preferring visitors to /zh/ before
 * paint. An explicit language-switch click (components/lang.tsx) stores a
 * preference that always wins, and /zh/ never auto-redirects, so shared
 * links and crawlers (which fetch without executing preference state) keep
 * stable, indexable URLs. hreflang alternates stay the SEO source of truth.
 *
 * Kept as a prebuilt string so the (en) layout can inline it as a blocking
 * <script> at the top of <body> — no hydration, runs before first paint.
 */
export const LANG_REDIRECT_JS = `(function () {
  try {
    var path = location.pathname;
    if (path !== "/" && path !== "/index.html") return;
    var pref = localStorage.getItem("dm-lang");
    if (pref === "en") return;
    // arriving from /zh/ = an explicit choice to read English; honor and
    // remember it even if the switch link's handler had not hydrated yet
    if (document.referrer && document.referrer.indexOf(location.origin + "/zh") === 0) {
      try { localStorage.setItem("dm-lang", "en"); } catch (e) {}
      return;
    }
    if (pref === "zh") { location.replace("/zh/"); return; }
    var langs = navigator.languages && navigator.languages.length
      ? navigator.languages
      : [navigator.language || ""];
    for (var i = 0; i < langs.length; i++) {
      var l = String(langs[i]).toLowerCase();
      if (l === "zh" || l.indexOf("zh-") === 0) { location.replace("/zh/"); return; }
      if (l === "en" || l.indexOf("en-") === 0) return;
    }
  } catch (e) { /* never block the page over locale sniffing */ }
})();`;
