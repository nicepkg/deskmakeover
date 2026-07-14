# M6 Performance Architecture — how the Rust icon pipeline goes fast without moving a byte

> **This is a DESIGN NOTE (reference material), not an executable plan.** It records the perf
> architecture rationale; the executable M6 plans are `2026-07-11-m6-kernel-speed.md` +
> `2026-07-11-m6-p4-cutover.md`. (Belongs under `docs/reference/` by dev-cycle structure — kept here
> for now to avoid churning links while the M6 perf line is active.)

Design input for the M6 dual-target cutover (and the M7 resident). Synthesized from two
independent performance reviews that **converged on the same architecture** — a senior systems
engineer (with live micro-benchmarks) and a cross-vendor Codex architect. Where they agree the
confidence is high; disagreements and unknowns are called out. This is a design note, not a task
plan: it decides *shape and priority*, and every recommendation is scored for byte-exactness risk.

Anchor commit for the measurements: the all-real corpus at setHash `8a6c19ee69235d95`
(1487/1487 cells byte-identical). Baseline machine: Apple M2, Bun 1.3.11.

## 0. The one-paragraph decision

M6 keeps the **N Web Workers × one independent single-threaded WASM `RenderSession` each**,
source-sharded by stable id-hash (the current adapter shape) — and adds four surgical upgrades:
per-worker in-WASM profile cache, latest-generation job coalescing, a real register-once ABI, and
a one-copy pixel path. It does **NOT** adopt wasm threads / `wasm-bindgen-rayon`. The native path
(manual apply + M7 resident) uses **rayon per-icon** (provably byte-safe) behind a dedicated pool
and a single Operations actor, **after** one hard correctness prerequisite is fixed. The WASM flip
is a single-truth-source move, **not a speedup** — the real levers are not re-analyzing, not
rendering stale frames, and the parallel sharding we already have.

## 1. Measured reality (why "flip to WASM" is not the win)

Committed baseline (`testdata/icons/perf-baseline.json`, 124 icons, warm single-thread):
48px = 72.16 ms full set (0.58 ms/icon); 256px = 710.7 ms (5.73 ms/icon). These are **warm**
(profile + geometry masks already cached, `Filter=None`), so they measure compose+marks+shadow,
not analysis.

Fresh probes (same machine): cold `icon_profile` (analysis) = **7.6–18.4 ms/icon**; native
`render_tile` warm = 0.67–2.5 ms/icon. wasm-vs-native on the spike slice under Bun/JSC: **2.0× at
48px, ~parity at 256px** — the core is deliberately scalar f64 + libm + JS-rounding, so wasm is
1.5–2× the frozen TS per icon at preview sizes.

Three implications, both reviews agree:
1. The preview workload is small (124 icons × 24–256px, `displaySize` clamps at
   `src/stores/icons.ts:104-108`): a full settings-change re-render is ~85–280 ms native
   single-thread, ÷6 workers ≈ 15–50 ms wall.
2. **The M6 wasm flip buys single-truth, not speed.** Do not sell it as a perf win.
3. **The profile cache is make-or-break**: cold analysis dominates — ~74% of analysis+compose at
   256px, ~96% at 48px. Without it every render pays 8–18 ms; with it a settings re-render is
   nearly all cache hits.

Codex adds a quantified downstream warning: once pixels are fast, the **WAL/ledger fsync cost**
likely becomes the bottleneck — a 124-icon apply issues `TxnBegin + 4×124 + TxnCommitted` = **498
journal fsyncs** plus 124 full-file JSON ledger rewrites (~O(N²)). Measure on NTFS + Defender.

## 2. Recommended M6 architecture

Keep the id-hash source sharding (one worker owns an id for its whole life, so its cache always
hits; total source RGBA stays ~31 MiB, not N× copies). Replace each shard's TS Raster/Profile with
a long-lived WASM `RenderSession`. Global hue-spread is computed **once** by worker 0 calling a
Rust export and frozen before any derived-field render consumes it — never sharded.

```
React / Zustand ── resolved config + monotonic generation (small DTO)
        │
        ▼
PreviewCoordinator (main thread)
  ├─ latest-only / bounded queue        ← the #1 interactive lever
  ├─ id → worker/source-handle routing
  ├─ byte-budgeted ImageBitmap LRU
  └─ collect seeds → worker 0 Rust hue-spread → frozen fieldSeed map
        │
        ├─ Worker 0 ─ WASM instance ─ RenderSession shard 0
        ├─ Worker 1 ─ WASM instance ─ RenderSession shard 1
        └─ Worker N ─ WASM instance ─ RenderSession shard N
             source URL → browser decode → ONE JS→WASM copy into linear memory
             cached source/profile/geometry + worker-local scratch
             WASM output view → OffscreenCanvas → transferable ImageBitmap → main
```

**Data ownership** (single writer per boundary): main owns generation + routing + bitmap LRU; each
WASM shard owns its normalized RGBA + profile/mask + geometry cache + scratch; native SourceCatalog
owns `Arc<Raster>` + `Arc<IconProfile>`; the Operations actor owns WAL + ledger + AssetStore + STA.
Still a modular monolith — no new processes.

## 3. Preview path: the three-way comparison

| Option | Verdict | Why |
|--------|---------|-----|
| **N workers × N independent WASM instances** | **M6 pick** | 124 independent icons already fill the cores; source RGBA is one copy total via sharding; `&mut self` session needs no lock; a worker trap loses only one shard |
| shared-memory WASM + `wasm-bindgen-rayon` | **rejected for M6** | still runs on Web Workers; needs SAB + COOP/COEP + atomics + bindgen glue + a shared allocator; the core is `&mut self`; and the correctness-bearing arrow is thread-local. Buys milliseconds on a 124×1–2 ms workload |
| N outer workers × inner rayon | **banned** | CPU over-subscription, larger memory + fault surface |
| native Tauri command returns preview pixels | **banned** | 124 RGBA buffers across IPC; breaks browser-dev, the uniform worker boundary, and fault isolation |

**Cross-origin isolation (for the rejected threads path) is achievable but unused today** — Tauri
2.11's `security.headers` emits COOP/COEP and Wry hands them to WebView2's `CreateWebResourceResponse`;
vite dev needs the same via `server.headers`. Neither is configured now. Since the M6 pick needs no
SharedArrayBuffer, M6 takes on none of this regression surface. If threads are ever revisited,
COI-header plumbing in both environments is the precondition, not the blocker.

**The real interactive hot path is stale-job burn, not missing threads.** Today a superseded style
only drops stale *results* (`icon-renderer.ts:135-148`); the worker still computes every queued job
(`render.worker.ts:126-132`). A fast slider drag through K styles queues up to K×124 renders. The
new protocol: monotonic `generation` per settings change; each slot keeps only its latest pending
job; started icons finish but their result is dropped; yield to the event loop between icons; drag
intermediates never enter the 20-style LRU; main removes stale in-flight counts immediately so
`pendingCount()` can't wedge the first-paint gate.

## 4. Native path (manual apply + M7 resident)

- **Per-icon rayon only.** `render_tile` is a pure function; `par_iter().map().collect()` over icons
  preserves output order (indexed collect) and touches no shared float state → **provably
  byte-identical**. Within-icon stage parallelism is banned (adds barriers + determinism risk for
  no gain at these sizes).
- **Never block the async executor.** Tauri commands are `async fn` that submit an owned job to a
  long-lived RenderExecutor and await a oneshot; the executor owns a fixed, capacity-capped rayon
  pool. **Not** `spawn_blocking` per icon (that's the blocking-I/O pool, default 512 threads). WAL
  fsync + `StaExecutor::run` are blocking → a single Operations actor, off the async runtime.
- **Two pools**: interactive (manual apply, `cores-1`) and background (resident reconciler, 2–4
  threads with `THREAD_MODE_BACKGROUND_BEGIN` + Win11 EcoQoS to hit spec 07's ≈0%-idle gate).
- **Priority = queue discipline** (rayon has no native priority): user apply > user "check desktop
  now" > watcher reconcile; single-flight per item id; check a generation token between icons and
  between stages; a superseded reconcile wave drops not-yet-started jobs, in-flight ones finish.
- **The biggest native saving is not rendering at all**: the reconciler compares `style_hash` first
  and skips untouched icons (a settings-noop reconcile ≈ 0 render time), and the persisted profile
  cache (`source_hash + ANALYSIS_SCHEMA_VERSION`) skips analysis. Together they meet spec 07's
  "<250 ms warm single-icon" budget ~10× over.

### 4a. HARD PREREQUISITE before any native rayon (correctness, not perf)

`NATIVE_ARROW` is a `thread_local!` (`crates/dm-icon-core/src/marks/mod.rs:178-190`). The corpus
runner sets it on one thread then renders serially. **The moment native work goes multi-threaded,
worker threads see `None` and render the drawn fallback instead of the real arrow — output changes
with which thread runs the job.** This must be fixed FIRST: pass the arrow as an explicit immutable
`RenderContext` input (e.g. `Arc<Raster>`), covering the `show_original && is_shortcut` and
`Distinction::Keep` paths, with a corpus-hash-equality test across 1/2/4/8 threads and randomized
job order. (Both reviews flag this; Codex rates naive rayon-without-this-fix as BREAKS.)
`RAMP_CACHE` / `POLY_CACHE` are also thread-local but are pure caches — byte-neutral, fine.

## 5. Determinism-safe parallelism — the proof map

**Provably byte-safe (risk NONE, but still run the frozen goldens):**
- Independent icons/tiles (disjoint outputs, pure function) — worker sharding and rayon indexed collect.
- Parallel seed *collection* for hue-spread — output is a `BTreeMap`, order-independent; the spread
  is a sync barrier before any derived-field render.
- Row/column splits of the sliding-window blurs at exactly row/column granularity; per-channel
  backdrop blur; exact-integer histograms merged in fixed bin order.

**MUST stay sequential / fixed-order — f64 reductions whose order is load-bearing (these cites ARE
the certification anchor):** `analysis/dominant.rs` (chroma/RGB/cos/sin sums, insertion order),
`analysis/background.rs` ring accumulator, `analysis/mod.rs` visible-lightness mean, `profile.rs`
lightness passes, `compose/mod.rs` composed-luminance (drives mark contrast), `raster.rs` box-blur
recurrence, `compose/field.rs` box_blur_in_place (f64 accumulate / f32 store — the JS-parity
subtlety), `sampling.rs` area-average (strict y-then-x), `hue_spread.rs` relaxation sweeps + wrap
seam, `mono.rs` cumulative percentile. Fixed-chunk partial-f64 merge still changes bracketing —
Kahan / pairwise / rayon `sum`/`reduce` are **not** lossless here.

**Post-M6 certification anchor (after the TS oracle is deleted):** wasm↔native byte equality
(structural — same libm bits, same codegen) **+** the frozen 1487-cell goldens **+** hash equality
across worker/rayon thread counts and randomized job order. Consequence: any pixel-changing
optimization doesn't merely "need recert" — it **fails the frozen goldens**, and re-blessing them is
an owner-level decision that severs the chain back to the TS/C# oracle. Treat NEEDS-RECERT as
"requires owner sign-off to re-anchor" ≈ BREAKS for v1. Two aligned targets can be consistently
wrong, so wasm↔native equality alone is never sufficient — the frozen goldens stay an independent anchor.

## 6. Ranked optimizations (all risk NONE unless noted)

| # | Optimization | Expected impact (124-icon settings change) | Cost | Byte risk |
|---|---|---|---|---|
| 0 | **Move the thread-local arrow to an explicit RenderContext** | rayon correctness prerequisite, not optional | M | NONE (skipping it = BREAKS) |
| 1 | Latest-generation coalescing + bounded queue + drag not in LRU | worst-case drag ~300 ms+ dead work → one wave (~30 ms); slider locks to frame rate | M | NONE |
| 2 | Per-worker in-WASM `RenderSession` profile cache (never re-analyze per render) | the difference between ~30 ms and ~1 s per wave — **mandatory for M6 parity** | part of the Config ABI | NONE |
| 3 | Real M6 ABI: register-once sources (kill the per-render `to_vec`), `reset()`, out-buffer reuse | ~30% of a 48px render per call + leak fix | S | NONE |
| 4 | One-copy pixel path (WASM view → putImageData → transferToImageBitmap → transfer); config as small message; pixels never JSON/base64/clone | keeps transport ≈0.1 ms/tile; avoids accidental 10× regressions | S | NONE |
| 5 | Rust geometry cache + `MarkContext` borrows the tile mask (no 512 KiB clone when mark is None) | removes two mask builds + a 512 KiB copy per 256px tile | M | NONE |
| 6 | Byte-budget bitmap LRU + release the JS Raster after WASM registration | avoids a theoretical ~620 MiB high-res cache; stability win | S–M | NONE |
| 7 | Native: reconciler `style_hash` short-circuit + persisted profile cache (rusqlite) | background reconcile of an unchanged desktop ≈ 0 render ms | M | NONE |
| 8 | Native: dedicated rayon pools + oneshot bridge; background threads EcoQoS | full 124-icon apply 0.7–3 s → ~150–400 ms; UI never janks | M | NONE |
| 9 | Ledger `upsert_batch` + kept-open journal handle | likely the biggest native apply wall-clock win once pixels are fast | M–H | NONE (needs kill-point re-cert) |
| 10 | `instantiateStreaming` + stable module URL (V8 code cache ≥128 KiB); verify `application/wasm` MIME | few ms/worker boot | S | NONE |

## 7. DO-NOT-DO

- **Enable rayon before fixing the thread-local arrow** — output varies by thread → BREAKS.
- **wasm-bindgen-rayon / SharedArrayBuffer threads for M6** — nightly + build-std + atomics codegen
  in the cert-critical crate; breaks the structural wasm↔native argument; one trap poisons all
  preview; milliseconds of gain.
- **SIMD / `mul_add` / any reassociation / Kahan / pairwise / rayon float `sum`/`reduce`** — the §5
  reductions reorder → frozen goldens fail.
- **A `Mutex<RenderSession>` pretending to be native parallelism** — `render(&mut self)` serializes it.
- Register the whole desktop's sources in every worker (defeats shard affinity's 31 MiB total).
- Shard-local hue spread, or organizing global inputs by worker completion order.
- JSON / base64 on pixel buffers or profile masks anywhere in the loop (kill the base64 bake path
  at `icon-renderer.ts:268-289` once native apply lands).
- Transfer/detach the WASM linear-memory buffer (use the transferable `ImageBitmap`).
- Cache a mutable card mask then carve it in place (poisons later renders).
- Porting PNG decode into WASM for preview sources (browser decode is free and off the certified path).
- `spawn_blocking` per icon render (it's the blocking-I/O pool).
- Configuring COOP/COEP on only one of Tauri / vite.

## 8. Open questions — need the owner's Windows box or a real benchmark

1. V8-vs-JSC wasm ratio (measured 2.0×/~1.0× on Bun/JSC; WebView2 is V8 — re-run the spike timing there).
2. Real-corpus lane mix + true per-lane native/wasm cost (instrument the M5 harness over the 124 real icons).
3. Full M6 `.wasm` size, compile/instantiate time, per-instance committed memory; does the custom
   protocol serve `Content-Type: application/wasm` (needed for streaming compile + V8 code cache)?
4. `crossOriginIsolated` / SAB / CSP behaviour in WebView2 vs dev Chrome (only matters if threads revisited).
5. 2/4/6-worker whole-generation p50/p95 at real 48/96/high-DPI sizes; longest shard; UI frame time.
6. Rust/WASM stage benchmarks (analysis / compose / marks / each filter / resample / ICO) — no Criterion harness exists yet.
7. Filter peak scratch (Glass is the most expensive: ~2 s for 124 icons at 256px) → sets the pool + memory hard cap.
8. NTFS + Defender cost of 498 journal fsyncs + 124 ledger rewrites (validates the P9/P8 ordering above).
9. Browser session cache / cold-start cache / resident disk cache hit rates.
10. EcoQoS + `THREAD_MODE_BACKGROUND_BEGIN` behaviour under memory-priority throttling (spec 07 idle gate).

## 9. Cross-references

- Determinism doctrine + one-parallel-layer decision: ADR-0019 (`docs/decisions/0019-tauri-rust-replatform.md`).
- M6/M7 milestone framing: `docs/plans/2026-07-10-tauri-migration.md` §M6/§M7.
- Resident spec (persisted cache, idle budget): `docs/specs/07-background-resident.md`.
- Certification anchor: `testdata/icons/README.md` + the M5.12 DONE block in the migration plan.
