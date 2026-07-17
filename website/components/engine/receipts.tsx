import type { EngineReceipt } from "@/content/engine-types";

/**
 * The receipts strip — every metric deep-links to the exact source on
 * GitHub. Unlinked numbers read as padding; linked ones are evidence.
 */
export function Receipts({ lead, receipts }: { lead?: string; receipts: EngineReceipt[] }) {
  return (
    <div>
      {lead ? <p className="mb-5 max-w-[46rem] text-[14px] leading-[1.6] text-ink-2">{lead}</p> : null}
      <div className="grid gap-px border border-line bg-line sm:grid-cols-3">
        {receipts.map((r, i) => (
          <a
            key={r.href + r.value}
            href={r.href}
            target="_blank"
            rel="noreferrer"
            data-fx-cell
            className="group bg-card px-4 py-4 transition-colors hover:bg-panel"
            style={{ ["--fxd" as string]: `${i * 90}ms` }}
          >
            <p className="break-all font-mono text-[15px] font-semibold text-ink">
              {r.value}
              <span aria-hidden className="ml-1.5 inline-block text-[11px] text-ink-3 transition-transform group-hover:-translate-y-0.5 group-hover:translate-x-0.5">
                ↗
              </span>
            </p>
            <p className="mt-1 text-[11.5px] leading-[1.5] text-ink-3">{r.label}</p>
          </a>
        ))}
      </div>
    </div>
  );
}
