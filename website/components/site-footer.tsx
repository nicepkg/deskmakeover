import type { Dict } from "@/content/types";
import { LangSwitch } from "@/components/lang";

export function SiteFooter({ dict }: { dict: Dict }) {
  const f = dict.footer;
  return (
    <footer className="border-t border-line">
      <div className="mx-auto flex max-w-[1200px] flex-col gap-8 px-5 py-14 md:flex-row md:items-end md:justify-between md:px-8">
        <div>
          <p className="font-display text-[22px] font-semibold tracking-tight text-ink">
            {f.tagline}
          </p>
          <p className="mt-2 text-[14px] text-ink-3">{f.license}</p>
          <p className="mt-1 text-[12px] text-ink-3">
            3D:{" "}
            <a
              href="https://sketchfab.com/3d-models/apple-studio-display-f56b9892c6b941168f64bc8323c98875"
              target="_blank"
              rel="noreferrer"
              className="underline decoration-line underline-offset-2 transition-colors hover:text-ink"
            >
              Apple Studio Display
            </a>{" "}
            by alboxer2000_ ·{" "}
            <a
              href="https://creativecommons.org/licenses/by/4.0/"
              target="_blank"
              rel="noreferrer"
              className="underline decoration-line underline-offset-2 transition-colors hover:text-ink"
            >
              CC BY 4.0
            </a>
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-x-7 gap-y-2 font-mono text-[12px] tracking-[0.1em] text-ink-2">
          <a href="/story/" className="transition-colors hover:text-ink">
            {dict.nav.story}
          </a>
          {f.links.map((l) => (
            <a
              key={l.href}
              href={l.href}
              target="_blank"
              rel="noreferrer"
              className="transition-colors hover:text-ink"
            >
              {l.label}
            </a>
          ))}
          <LangSwitch
            href={dict.nav.langHref}
            lang={dict.locale === "zh" ? "en" : "zh"}
            className="transition-colors hover:text-ink"
          >
            {dict.nav.langLabel}
          </LangSwitch>
          <span className="text-ink-3">© 2026 nicepkg</span>
        </div>
      </div>
    </footer>
  );
}
