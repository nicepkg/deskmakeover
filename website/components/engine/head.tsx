import type { EngineHead } from "@/content/engine-types";

/** Section head shared by the /engine/ sections and the playground finale. */
export function Head({ head, className }: { head: EngineHead; className?: string }) {
  return (
    <div className={className}>
      <p className="font-mono text-[12px] tracking-[0.22em] text-ink-3">
        <span className="text-coral-ink">{head.index}</span>
        {"  ·  "}
        {head.kicker}
      </p>
      <h2 className="mt-4 max-w-[24ch] text-[28px] font-bold leading-[1.14] tracking-[-0.015em] md:text-[36px]">
        {head.title}
      </h2>
      <p className="mt-4 max-w-[44rem] text-[15.5px] leading-[1.65] text-ink-2">{head.body}</p>
    </div>
  );
}
