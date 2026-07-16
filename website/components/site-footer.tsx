import type { Dict } from "@/content/types";

export function SiteFooter({ dict }: { dict: Dict }) {
  return (
    <footer className="border-t border-hairline bg-mist">
      <div className="mx-auto max-w-[1200px] px-5 py-14 md:px-8">
        <div className="flex flex-col items-start justify-between gap-8 md:flex-row md:items-center">
          <div>
            <p className="font-display text-[19px] font-semibold">
              {dict.footer.tagline}{" "}
              <a href={dict.footer.starLink} className="text-coral-text underline decoration-coral/40 underline-offset-4 transition-colors hover:decoration-coral">
                {dict.footer.star}
              </a>
            </p>
            <p className="mt-2 text-[14px] text-ink-soft">{dict.footer.license}</p>
          </div>
          <nav className="flex gap-6">
            {dict.footer.links.map((l) => (
              <a key={l.href} href={l.href} className="text-[14px] text-ink-soft transition-colors hover:text-ink">
                {l.label}
              </a>
            ))}
          </nav>
        </div>
        <div className="mt-10 h-[2px] w-full rounded bg-gradient-to-r from-coral/15 via-coral to-coral/15" aria-hidden="true" />
        <p className="mt-6 flex items-center gap-2 text-[13px] text-ink-soft">
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img src="/logo.png" alt="" width={18} height={18} className="rounded-[5px]" />
          DeskMakeover · 桌面美颜
        </p>
      </div>
    </footer>
  );
}
