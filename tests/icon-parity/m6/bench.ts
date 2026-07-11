#!/usr/bin/env bun
// Kernel-speed bench — TS oracle vs scalar-wasm vs fast-wasm, warm, at preview (96)
// and bake (256) sizes over the 124-icon set. Two outputs:
//
//   1. Directional bun numbers (this process). JSC tiers wasm UP on short runs, so
//      the ABSOLUTE bun ms are a tier-up artifact (~40× at 96) — read them only for
//      TS-vs-TS or a rough scalar-vs-fast direction, NEVER as the shippable cost.
//   2. A self-contained WebView2/V8 bench page → public/icon-kernel-bench.html. Both
//      wasm kernels and a 24-icon source fixture are embedded as base64 (no server,
//      no CORS — works over file:// and in the Tauri WebView2 shell), so the
//      AUTHORITATIVE per-icon ms and the fast/scalar delta are measured in the same
//      V8/WebView2 engine the preview actually ships on.
//
//   bun tests/icon-parity/m6/bench.ts
//
// MEASURED baseline (Apple M2, warm, in-browser V8 — Chrome 150, P5): TS 1.27 / 3.85
// ms/icon at 96, 6.94 / 19.2 at 256 (WASM ~3× TS). The M6 kernel-speed phases close
// that gap by porting the TS mask/source caches to Rust; each phase re-runs the page.

import { copyFileSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

import type { ConfigDto } from '../../../src/bridge/types'
import { renderTile } from '../../../src/icon-compositor/compose'
import { setNativeArrowRaster } from '../../../src/icon-compositor/marks'
import { encodeConfig, hexToInt } from '../../../src/icon-wasm/config-abi'
import { WasmIconRenderer } from '../../../src/icon-wasm/wasm-loader'
import { buildWasm, loadArrow, loadCells, readSource, REPO_ROOT } from './harness'

const N = 124 // one settings-change worth of icons
const FIXTURE_N = 24 // embedded in the browser page (keeps the html a few MB)
const SIZES = [96, 256]

function median(xs: number[]): number {
  return [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)]
}

interface BenchCell {
  sourceId: string
  config: ConfigDto
  isShortcut: boolean
  fieldSeed: string | null
}

function b64(bytes: Uint8Array): string {
  return Buffer.from(bytes).toString('base64')
}

async function main(): Promise<void> {
  // Build both kernels and stage them for the browser page.
  const scalarWasm = buildWasm('scalar')
  const fastWasm = buildWasm('fast')
  copyFileSync(scalarWasm, join(REPO_ROOT, 'public/dm_icon_wasm.scalar.wasm'))
  copyFileSync(fastWasm, join(REPO_ROOT, 'public/dm_icon_wasm.fast.wasm'))

  // One representative cell per source.
  const bySource = new Map<string, BenchCell>()
  for (const c of loadCells()) {
    if (!bySource.has(c.sourceId)) bySource.set(c.sourceId, { sourceId: c.sourceId, config: c.config, isShortcut: c.isShortcut, fieldSeed: c.fieldSeed })
    if (bySource.size >= N) break
  }
  const cells = [...bySource.values()]
  const rgba = new Map(cells.map((c) => [c.sourceId, readSource(c.sourceId)]))
  const arrow = loadArrow()

  // ── directional bun bench ──────────────────────────────────────────────────
  setNativeArrowRaster({ width: arrow.width, height: arrow.height, data: new Uint8ClampedArray(arrow.bytes) })
  const tsRaster = new Map(cells.map((c) => [c.sourceId, { width: 256, height: 256, data: new Uint8ClampedArray(rgba.get(c.sourceId)!) }]))
  const benchTs = (size: number): number => {
    const t = performance.now()
    for (const c of cells) renderTile(tsRaster.get(c.sourceId)!, c.config, c.isShortcut, false, size, { fieldSeed: c.fieldSeed })
    return performance.now() - t
  }
  const mkWasmBench = async (wasmPath: string) => {
    const w = await WasmIconRenderer.fromBytes(new Uint8Array(readFileSync(wasmPath)))
    w.setArrow(new Uint8ClampedArray(arrow.bytes), arrow.width, arrow.height)
    for (const c of cells) w.registerSource(c.sourceId, new Uint8ClampedArray(rgba.get(c.sourceId)!))
    return (size: number): number => {
      const t = performance.now()
      for (const c of cells) w.render(c.sourceId, c.config, c.isShortcut, false, size, { fieldSeed: c.fieldSeed })
      return performance.now() - t
    }
  }
  const benchScalar = await mkWasmBench(scalarWasm)
  const benchFast = await mkWasmBench(fastWasm)

  console.log(`\nDirectional bun bench — ${N} icons warm, median of 5 (JSC; absolutes are a tier-up artifact)`)
  console.log(`${'size'.padEnd(6)}${'TS/icon'.padStart(10)}${'scalar/icon'.padStart(13)}${'fast/icon'.padStart(12)}${'fast/scalar'.padStart(13)}`)
  for (const size of SIZES) {
    benchTs(size), benchScalar(size), benchFast(size) // warm
    const ts = median(Array.from({ length: 5 }, () => benchTs(size)))
    const sc = median(Array.from({ length: 5 }, () => benchScalar(size)))
    const fa = median(Array.from({ length: 5 }, () => benchFast(size)))
    console.log(`${String(size).padEnd(6)}${(ts / N).toFixed(2).padStart(10)}${(sc / N).toFixed(2).padStart(13)}${(fa / N).toFixed(2).padStart(12)}${`${(fa / sc).toFixed(2)}×`.padStart(13)}`)
  }

  // ── self-contained WebView2/V8 page ─────────────────────────────────────────
  const fixture = cells.slice(0, FIXTURE_N).map((c) => ({
    src: b64(rgba.get(c.sourceId)!),
    cfg: b64(encodeConfig(c.config)),
    shortcut: c.isShortcut,
    seed: c.fieldSeed == null ? null : hexToInt(c.fieldSeed),
  }))
  const page = benchPage({
    scalarB64: b64(new Uint8Array(readFileSync(scalarWasm))),
    fastB64: b64(new Uint8Array(readFileSync(fastWasm))),
    arrowB64: b64(arrow.bytes),
    arrowW: arrow.width,
    arrowH: arrow.height,
    fixture,
    sizes: SIZES,
  })
  const out = join(REPO_ROOT, 'public/icon-kernel-bench.html')
  writeFileSync(out, page)
  console.log(`\nWebView2/V8 bench → ${out}`)
  console.log('  authoritative run: `bun run dev` then open /icon-kernel-bench.html (or open the file directly in the Tauri WebView2 shell)')
}

// The page embeds both kernels + a source fixture and drives the raw render_tile
// ABI inline (no app imports, no fetch), timing scalar vs fast in the host engine.
function benchPage(d: {
  scalarB64: string
  fastB64: string
  arrowB64: string
  arrowW: number
  arrowH: number
  fixture: { src: string; cfg: string; shortcut: boolean; seed: number | null }[]
  sizes: number[]
}): string {
  return `<!doctype html><meta charset=utf8><title>icon kernel bench</title>
<style>body{font:14px ui-monospace,monospace;margin:2rem;max-width:40rem}table{border-collapse:collapse;margin-top:1rem}td,th{border:1px solid #8884;padding:.3rem .8rem;text-align:right}th:first-child,td:first-child{text-align:left}#s{color:#888}</style>
<h1>icon kernel bench — scalar vs fast (this engine)</h1>
<div id=s>building…</div><table id=t hidden><thead><tr><th>size<th>scalar ms/icon<th>fast ms/icon<th>fast/scalar</thead><tbody></tbody></table>
<script>
const B=s=>{const b=atob(s),u=new Uint8Array(b.length);for(let i=0;i<b.length;i++)u[i]=b.charCodeAt(i);return u};
const ARROW=B(${JSON.stringify(d.arrowB64)}),AW=${d.arrowW},AH=${d.arrowH};
const FIX=${JSON.stringify(d.fixture)}.map(f=>({src:B(f.src),cfg:B(f.cfg),shortcut:f.shortcut,seed:f.seed}));
const SIZES=${JSON.stringify(d.sizes)},SRC=256,REPS=8,WARM=3;
function drive(bytes){
  const {exports:e}=new WebAssembly.Instance(new WebAssembly.Module(bytes),{});
  const mem=()=>new Uint8Array(e.memory.buffer);
  const ap=e.dm_alloc(AW*AH*4);mem().set(ARROW,ap);e.dm_set_native_arrow(ap,AW,AH);
  const s=e.dm_session_new(),sp=e.dm_alloc(SRC*SRC*4),cp=e.dm_alloc(24),ip=e.dm_alloc(8),op=e.dm_alloc(512*512*4);
  const enc=new TextEncoder();
  FIX.forEach((f,i)=>{const id=enc.encode('s'+i);mem().set(id,ip);mem().set(f.src,sp);e.dm_session_register(s,ip,id.length,BigInt(i+1),sp,SRC,SRC)});
  return (size)=>{for(let i=0;i<FIX.length;i++){const f=FIX[i],id=enc.encode('s'+i);mem().set(id,ip);mem().set(f.cfg,cp);e.dm_session_set_config(s,cp,24);e.dm_session_render(s,ip,id.length,f.shortcut?1:0,0,size,f.seed==null?0:1,f.seed==null?0:f.seed>>>0,op)}};
}
function med(a){a=[...a].sort((x,y)=>x-y);return a[a.length>>1]}
requestAnimationFrame(()=>{
  const scalar=drive(B(${JSON.stringify(d.scalarB64)})),fast=drive(B(${JSON.stringify(d.fastB64)}));
  const tb=document.querySelector('#t tbody');
  for(const size of SIZES){
    for(let i=0;i<WARM;i++){scalar(size);fast(size)}
    const sc=med(Array.from({length:REPS},()=>{const t=performance.now();scalar(size);return performance.now()-t}))/FIX.length;
    const fa=med(Array.from({length:REPS},()=>{const t=performance.now();fast(size);return performance.now()-t}))/FIX.length;
    const r=tb.insertRow();r.insertCell().textContent=size;r.insertCell().textContent=sc.toFixed(3);r.insertCell().textContent=fa.toFixed(3);r.insertCell().textContent=(fa/sc).toFixed(2)+'×';
  }
  document.getElementById('s').textContent='engine: '+navigator.userAgent;
  document.getElementById('t').hidden=false;
});
</script>`
}

main()
