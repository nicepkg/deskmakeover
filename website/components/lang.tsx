"use client";

/**
 * Locale preference plumbing for a fully static export.
 *
 * - The root (en) page runs a tiny blocking script (see LANG_REDIRECT_JS in
 *   lib/lang-redirect.ts) that routes first-time zh-preferring visitors to
 *   /zh/ before paint.
 * - Every explicit language-switch click records the choice here so the
 *   redirect never fights the user afterwards.
 */
export function LangSwitch({
  href,
  lang,
  className,
  children,
}: {
  href: string;
  lang: "en" | "zh";
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <a
      href={href}
      className={className}
      onClick={() => {
        try {
          localStorage.setItem("dm-lang", lang);
        } catch {
          /* storage unavailable — redirect script degrades gracefully */
        }
      }}
    >
      {children}
    </a>
  );
}
