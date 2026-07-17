/**
 * Theme plumbing. Preference model: localStorage "dm-theme" holds "light" or
 * "dark" for an explicit choice; absence means AUTO (follow the system, the
 * default). The explicit choice is stamped pre-paint as html[data-theme] by
 * the blocking snippet below (inlined at the top of <body> in every root
 * layout); app/globals.css keys all color tokens off that attribute plus the
 * system media query. components/theme-toggle.tsx mutates both at runtime.
 */
export const THEME_STORAGE_KEY = "dm-theme";

export const THEME_INIT_JS = `(function () {
  try {
    var t = localStorage.getItem(${JSON.stringify(THEME_STORAGE_KEY)});
    if (t === "light" || t === "dark") document.documentElement.setAttribute("data-theme", t);
  } catch (e) { /* never block paint over a theme preference */ }
})();`;
