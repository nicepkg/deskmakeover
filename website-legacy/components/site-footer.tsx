import type { Dict } from "@/content/types";

export function SiteFooter({ dict }: { dict: Dict }) {
  const localLine = dict.locale === "zh" ? "本地运行 · 无遥测" : "Runs locally · No telemetry";
  return (
    <footer className="border-t border-hairline">
      <div className="mx-auto grid max-w-[1240px] gap-10 px-5 py-14 md:grid-cols-3 md:px-8">
        <div>
          <p className="font-display text-[18px] font-semibold text-coral-ink">DeskMakeover</p>
          <p className="mt-1 text-[13.5px] text-text-dim">桌面美颜 · {localLine}</p>
        </div>
        <nav aria-label={dict.locale === "zh" ? "页脚链接" : "Footer"} className="flex flex-wrap items-start gap-x-8 gap-y-2 md:justify-center">
          {dict.footer.links.map((l) => (
            <a key={l.href} href={l.href} className="text-[14px] text-text-mid transition-colors hover:text-text-hi">
              {l.label}
            </a>
          ))}
        </nav>
        <div className="md:text-right">
          <p className="text-[14px] text-text-mid">
            {dict.footer.tagline}{" "}
            <a href={dict.footer.starLink} className="font-semibold text-coral-ink underline decoration-coral/40 underline-offset-4 transition-colors hover:decoration-coral">
              {dict.footer.star}
            </a>
          </p>
          <p className="mt-2 text-[13px] text-text-dim">{dict.footer.license}</p>
        </div>
      </div>
    </footer>
  );
}
