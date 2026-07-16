import type { Dict } from "@/content/types";

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
        </div>
        <div className="flex flex-wrap items-center gap-x-7 gap-y-2 font-mono text-[12px] tracking-[0.1em] text-ink-2">
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
          <a href={dict.nav.langHref} className="transition-colors hover:text-ink">
            {dict.nav.langLabel}
          </a>
          <span className="text-ink-3">© 2026 nicepkg</span>
        </div>
      </div>
    </footer>
  );
}
