"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { EngineDict } from "@/content/engine-types";
import { Head } from "../head";
import { EngineRenderer, MASTER_SIZE, type PlaygroundConfig } from "./renderer";
import { SAMPLES, rasterizeSample, rasterizeUserImage } from "./samples";

type Status = "idle" | "loading" | "ready" | "failed";

function hslHex(h: number, s: number, l: number): string {
  const a = (s * Math.min(l, 1 - l)) / 100 / 100;
  const f = (n: number) => {
    const k = (n + h / 30) % 12;
    const c = l / 100 - a * Math.max(-1, Math.min(k - 3, Math.min(9 - k, 1)));
    return Math.round(255 * c)
      .toString(16)
      .padStart(2, "0");
  };
  return `#${f(0)}${f(8)}${f(4)}`;
}

const HUE_TRACK = `linear-gradient(to right, ${Array.from({ length: 13 }, (_, i) => `hsl(${i * 30} 62% 58%)`).join(", ")})`;

function Chip({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={active}
      className={`border px-3 py-1.5 font-mono text-[11.5px] tracking-[0.04em] transition-colors ${
        active
          ? "border-coral bg-coral text-white"
          : "border-line bg-card text-ink-2 hover:border-ink-3 hover:text-ink"
      }`}
    >
      {children}
    </button>
  );
}

function ControlRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <p className="font-mono text-[10.5px] tracking-[0.18em] text-ink-3">{label.toUpperCase()}</p>
      <div className="mt-2 flex flex-wrap gap-1.5">{children}</div>
    </div>
  );
}

/**
 * The finale: dm-icon-wasm — the exact pipeline the desktop app ships —
 * running live in the page. Lazy-loads on approach; every frame is computed
 * on the spot (rAF-coalesced). No WASM → an honest pre-rendered fallback.
 */
export function Playground({ engine }: { engine: EngineDict }) {
  const p = engine.playground;
  const sectionRef = useRef<HTMLElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const rendererRef = useRef<EngineRenderer | null>(null);
  const rafRef = useRef(0);
  const fileRef = useRef<HTMLInputElement>(null);
  const userThumbRef = useRef<HTMLCanvasElement>(null);

  const [status, setStatus] = useState<Status>("idle");
  const [sel, setSel] = useState("note");
  const [shape, setShape] = useState("Apple");
  const [look, setLook] = useState("Original");
  const [finish, setFinish] = useState("None");
  const [hue, setHue] = useState<number | null>(null);
  const [original, setOriginal] = useState(false);
  const [ms, setMs] = useState<number | null>(null);
  const [hasUser, setHasUser] = useState(false);

  // lazy boot when the section approaches the viewport
  useEffect(() => {
    const el = sectionRef.current;
    if (!el || status !== "idle") return;
    const io = new IntersectionObserver(
      (records) => {
        if (!records.some((r) => r.isIntersecting)) return;
        io.disconnect();
        setStatus("loading");
        (async () => {
          try {
            if (typeof WebAssembly !== "object") throw new Error("no wasm");
            const renderer = await EngineRenderer.create();
            for (const s of SAMPLES) renderer.registerSource(s.id, rasterizeSample(s.draw));
            rendererRef.current = renderer;
            setStatus("ready");
          } catch {
            setStatus("failed");
          }
        })();
      },
      { rootMargin: "600px" },
    );
    io.observe(el);
    return () => io.disconnect();
  }, [status]);

  useEffect(() => () => rendererRef.current?.dispose(), []);

  // one render per animation frame, latest settings win
  const hueHex = hue == null ? null : hslHex(hue, 62, 58);
  useEffect(() => {
    if (status !== "ready") return;
    cancelAnimationFrame(rafRef.current);
    rafRef.current = requestAnimationFrame(() => {
      const renderer = rendererRef.current;
      const canvas = canvasRef.current;
      if (!renderer || !canvas) return;
      const config: PlaygroundConfig = {
        shape,
        subject: look,
        monoStyle: "Tonal",
        plateBand: "Vivid",
        distinction: "None",
        markStyle: "Glass",
        filter: finish,
        plateFallback: "derived",
        shortcutShape: null,
        markColor: null,
        plateColor: hueHex,
        autoSeparation: true,
        tint: hueHex ?? "#ff6f5e",
      };
      const t0 = performance.now();
      try {
        const rgba = renderer.render(sel, config, original, MASTER_SIZE);
        if (!rgba) return;
        canvas.getContext("2d")?.putImageData(new ImageData(rgba, MASTER_SIZE, MASTER_SIZE), 0, 0);
        setMs(performance.now() - t0);
      } catch {
        setStatus("failed");
      }
    });
    return () => cancelAnimationFrame(rafRef.current);
  }, [status, sel, shape, look, finish, hueHex, original]);

  const onUpload = useCallback(async (file: File) => {
    const renderer = rendererRef.current;
    if (!renderer) return;
    try {
      const bitmap = await createImageBitmap(file);
      const rgba = rasterizeUserImage(bitmap);
      bitmap.close();
      renderer.registerSource("user", rgba);
      const thumb = userThumbRef.current;
      if (thumb) {
        const tctx = thumb.getContext("2d");
        tctx?.clearRect(0, 0, thumb.width, thumb.height);
        tctx?.putImageData(new ImageData(rgba.slice(), MASTER_SIZE, MASTER_SIZE), 0, 0);
      }
      setHasUser(true);
      setSel("user");
    } catch {
      // unreadable image — leave the current selection in place
    }
  }, []);

  return (
    <section id="live" ref={sectionRef} className="border-t border-line bg-panel">
      <div className="mx-auto max-w-[1200px] px-5 py-20 md:px-8 md:py-28">
        <Head head={p} />

        <div className="mt-10 grid gap-10 lg:grid-cols-12">
          {/* controls */}
          <div className="space-y-7 lg:col-span-5">
            <div>
              <p className="font-mono text-[10.5px] tracking-[0.18em] text-ink-3">{p.sampleLabel.toUpperCase()}</p>
              <div className="mt-2 flex flex-wrap items-center gap-2">
                {SAMPLES.map((s) => (
                  <SampleThumb key={s.id} id={s.id} draw={s.draw} active={sel === s.id} onClick={() => setSel(s.id)} />
                ))}
                <button
                  type="button"
                  onClick={() => setSel("user")}
                  aria-pressed={sel === "user"}
                  className={`relative h-[56px] w-[56px] overflow-hidden border transition-colors ${
                    sel === "user" ? "border-coral" : "border-line hover:border-ink-3"
                  } ${hasUser ? "" : "hidden"} bg-card`}
                >
                  <canvas ref={userThumbRef} width={MASTER_SIZE} height={MASTER_SIZE} className="h-full w-full" />
                </button>
                <button
                  type="button"
                  onClick={() => fileRef.current?.click()}
                  disabled={status !== "ready"}
                  className="h-[56px] border border-dashed border-ink-3 bg-card px-3 font-mono text-[11px] text-ink-2 transition-colors hover:border-ink hover:text-ink disabled:opacity-40"
                >
                  + {p.uploadCta}
                </button>
                <input
                  ref={fileRef}
                  type="file"
                  accept="image/png,image/jpeg,image/webp,image/svg+xml"
                  className="hidden"
                  onChange={(e) => {
                    const f = e.target.files?.[0];
                    if (f) void onUpload(f);
                    e.target.value = "";
                  }}
                />
              </div>
              <p className="mt-2 text-[11.5px] leading-[1.55] text-ink-3">{p.uploadNote}</p>
            </div>

            <ControlRow label={p.controls.shape}>
              {p.options.shapes.map((o) => (
                <Chip key={o.tag} active={shape === o.tag} onClick={() => setShape(o.tag)}>
                  {o.label}
                </Chip>
              ))}
            </ControlRow>

            <ControlRow label={p.controls.look}>
              {p.options.looks.map((o) => (
                <Chip key={o.tag} active={look === o.tag} onClick={() => setLook(o.tag)}>
                  {o.label}
                </Chip>
              ))}
            </ControlRow>

            <ControlRow label={p.controls.finish}>
              {p.options.finishes.map((o) => (
                <Chip key={o.tag} active={finish === o.tag} onClick={() => setFinish(o.tag)}>
                  {o.label}
                </Chip>
              ))}
            </ControlRow>

            <div>
              <p className="font-mono text-[10.5px] tracking-[0.18em] text-ink-3">{p.controls.hue.toUpperCase()}</p>
              <div className="mt-2 flex items-center gap-2.5">
                <Chip active={hue == null} onClick={() => setHue(null)}>
                  {p.autoHue}
                </Chip>
                <input
                  type="range"
                  min={0}
                  max={359}
                  value={hue ?? 25}
                  onChange={(e) => setHue(Number(e.target.value))}
                  aria-label={p.controls.hue}
                  className="eng-hue h-[10px] w-full"
                  style={{ background: HUE_TRACK }}
                />
              </div>
            </div>
          </div>

          {/* the live tile */}
          <div className="lg:col-span-6 lg:col-start-7">
            <div className="border border-line bg-card p-6 md:p-8">
              <div className="relative mx-auto w-full max-w-[300px]">
                {status === "failed" ? (
                  <div>
                    {/* eslint-disable-next-line @next/next/no-img-element */}
                    <img src="/engine/fallback.webp" alt="" className="block w-full border border-line" />
                    <p className="mt-3 text-[12px] leading-[1.6] text-ink-3">{p.fallbackNote}</p>
                  </div>
                ) : (
                  <>
                    <canvas
                      ref={canvasRef}
                      width={MASTER_SIZE}
                      height={MASTER_SIZE}
                      aria-label={p.title}
                      // faint checkerboard so transparency and white plates read against the card
                      className="block aspect-square w-full [background:repeating-conic-gradient(var(--color-panel)_0%_25%,transparent_0%_50%)_0_0/20px_20px]"
                    />
                    {status !== "ready" ? (
                      <p className="absolute inset-x-0 top-1/2 -translate-y-1/2 text-center font-mono text-[12px] text-ink-3">
                        {status === "loading" ? `${p.loading}…` : ""}
                      </p>
                    ) : null}
                  </>
                )}
              </div>

              {status !== "failed" ? (
                <div className="mt-6 flex flex-wrap items-center justify-between gap-3 border-t border-line pt-4">
                  <button
                    type="button"
                    onClick={() => setOriginal((v) => !v)}
                    aria-pressed={original}
                    disabled={status !== "ready"}
                    className={`border px-3 py-1.5 font-mono text-[11.5px] transition-colors disabled:opacity-40 ${
                      original ? "border-ink bg-ink text-canvas" : "border-line bg-card text-ink-2 hover:border-ink-3"
                    }`}
                  >
                    {p.controls.original}
                  </button>
                  <span className="font-mono text-[11px] text-ink-3 tabular-nums">
                    {ms != null ? `RENDER ${ms.toFixed(1)} MS` : ""}
                  </span>
                </div>
              ) : null}
            </div>

            <div className="mt-4 flex flex-wrap items-center gap-2.5">
              <span className="inline-flex items-center gap-1.5 border border-teal px-2.5 py-1 font-mono text-[10.5px] tracking-[0.06em] text-teal">
                <svg viewBox="0 0 12 12" className="h-3 w-3" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden>
                  <path d="M2 6.5 5 9.5 10 3" />
                </svg>
                {p.badge}
              </span>
              <span className="border border-line bg-card px-2.5 py-1 font-mono text-[10.5px] tracking-[0.06em] text-ink-3">
                DM-ICON-WASM · 88 KB GZIP
              </span>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

function SampleThumb({
  id,
  draw,
  active,
  onClick,
}: {
  id: string;
  draw: (ctx: CanvasRenderingContext2D, size: number) => void;
  active: boolean;
  onClick: () => void;
}) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = ref.current;
    const ctx = c?.getContext("2d");
    if (c && ctx) draw(ctx, c.width);
  }, [draw]);
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={active}
      aria-label={id}
      className={`h-[56px] w-[56px] border bg-card p-1.5 transition-colors ${
        active ? "border-coral" : "border-line hover:border-ink-3"
      }`}
    >
      <canvas ref={ref} width={96} height={96} className="h-full w-full" />
    </button>
  );
}
