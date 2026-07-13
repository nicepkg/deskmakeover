# M6 kernel speed — byte-safe optimization plan

**Date:** 2026-07-11 · **Mode:** dev-cycle iterate · **Owner-approved:** the synthesis below (2026-07-11).
**Invariant (never violate):** the icon kernel's RGBA output stays **byte-identical** across wasm
(preview) ↔ native (apply/background), and byte-identical to the frozen TS oracle until it retires.
Cert anchor: 1487-cell corpus, **0 / 389,808,128 diff bytes**, setHash `8a6c19ee69235d95…`.

**Owner decision 2026-07-11 (SIMD stays in scope):** Boss confirmed the FULL byte-safe perf line runs to
completion — **Phase 6 SIMD is NOT dropped**, done properly (opcode disassembly + four-way scalar/SIMD ×
native/wasm cert) rather than skipped for speed ("不差时间"). Rationale accepted: the cert is the fuse —
any SIMD path that diverges by even one bit on any target reddens the cert and is rejected, so *attempting*
SIMD carries zero single-truth risk (worst case: that path doesn't ship, output untouched); the only cost
is engineering time, which is explicitly not the constraint. Bet remains caching(1-4)+LUT(5) for the
guaranteed wins; SIMD(6) is pursued too, gated on a profile proving it's still needed AND the cert proving
bit-identity. Only the genuinely non-reproducible SIMD (vectorized transcendentals, FMA, horizontal
reductions, fast-math/relaxed) stays permanently red-lined — those are cross-system-inconsistent by nature.

## Implementation status — 2026-07-14 (all m6-cert byte-verified, unpushed)

| Phase | What | Status |
|-------|------|--------|
| 0 cert-harden | anchor + four-way + sweep | ✅ done — and `614dcd5` restored the synthetic shape×mark sweep guard that R2's C-1 `MAX_RENDER_SIZE=256` clamp had silently disabled since `41ff448` (512 probe → 255, in-contract) |
| 1 mask cache | `mask_cache.rs` | ✅ done + wired (production) |
| 2 source-fact cache | `source_facts.rs` | ✅ done — **C-5 shared analysis bundle** (`fe0aaf8`) collapsed the double `segment_subject`/background/bounds into one compute; **P2-SCRATCH** (`4ecbc06`) reuses the shadow/blur/seat scratch instead of allocating per render (helps warm renders) |
| 3 rayon batch | `batch.rs` | ✅ **wired** into version-switch (`5347bb5`, C-7) via `native_bake::bake_masters_par`; resident wiring deferred (its per-item `is_desktop_busy` abort granularity is a design decision, and the resident is still unwired) |
| 4 output cache | `output_cache.rs` | 🔨 built + tested, unwired — needs cross-operation ownership from IconHost; the real ROI is the frequent resident path (unwired), not the occasional version-switch, so wire it with the resident |
| 5 sRGB pow LUT | `color.rs` | ⏳ deferred — NEEDS a reprofile confirming `libm::pow` is still material AND a cell-enclosure verifier (any cell touching a byte transition / branch join falls back to scalar) |
| 6 SIMD | — | ⏳ deferred — RED-to-ship; needs opcode disassembly proving lane-wise add/sub/mul/div only (no FMA / horizontal reduction / reassociation) across native+wasm before the cert can even judge it |

Remaining PROVEN-byte-safe wins still queued (codex R2 focused perf pass, each cert-gated): exact Glass-distance
`(size,int-dist)` cache, `matches_shape`+`max_scale_auto` config/shape memo, `subject_rim` erosion-buffer reuse,
RGB→OKLab 24-bit memo (cold-only). Full ranking + exact changes: `/private/tmp/dm-r2-review/out-perf2.json` +
the R2 review ledger `docs/reviews/2026-07-14-rust-audit-round2.md`.

## Why this exists (the load-bearing finding)

Two independent researchers (senior perf-eng subagent + Codex) profiled the ~3× WASM-vs-TS preview
gap. **The 3× is mostly NOT the determinism tax — it is Rust not yet memoizing what the frozen TS
already caches.** Codex's directional profile put ~86.5% of render samples inside the two geometry
**mask** calls (`compose/mod.rs:143,169` → `raster.rs:129` corner tests + 16×16 boundary supersample
+ per-polygon TLS hashmap lookup at `shapes/mod.rs:57`); frozen TS memoizes masks at `raster.ts:109`,
Rust recomputes them every render. Only after removing polygon work does `libm::pow` (`color.rs:45`,
called 3×/output-pixel in resampling) rise to ~18% locally, ~3-10% corpus-wide. So:

- Most of the 3× is recoverable by **porting TS's caching to Rust — zero byte risk, no math touched.**
- The genuine determinism cost (per-pixel `pow`) is second-order and also has a byte-safe fix.
- **This reframes the M6 flip decision:** "single truth costs 3×" is largely false; with caching
  restored the preview approaches parity and native apply gets *faster*. The flip can ship near-parity.

The determinism doctrine (libm-only, no FMA/SIMD/fast-math) is **preserved throughout**: single truth
= byte-identical *output*, not a single implementation. Any faster kernel must pass the (hardened) cert.

## Phase 0 — HARDEN THE CERTIFICATE FIRST (prerequisite for everything)

Codex found the current harness does not enforce its own anchor: `tests/icon-parity/m6/run.ts:38`
accepts any non-empty corpus; `:164` passes for any ≥1 equal cells; it never asserts the setHash,
the 1487 cell / 124 source / lane counts, file lengths, or the 389,808,128 byte total; it tests only
256px, only wasm→TS, and builds without `--locked`. **No fast-kernel work may land until the cert is
a real safety net.**

Tasks:
1. Assert the full manifest hash (prefix `8a6c19ee69235d95`), all counts (1487 cells, 124 sources,
   per-lane), file lengths, and the exact `389,808,128` compared-byte total. Fail on any mismatch.
2. Add a **four-way differential scaffold**: scalar-native, fast-native, scalar-wasm, fast-wasm — all
   vs frozen TS at 256px. (Fast kernels don't exist yet; wire the harness to accept a `KERNEL=scalar|fast`
   selector so later phases plug in.)
3. Add fast-vs-scalar differentials at **96px** and representative small/odd/large sizes.
4. Exercise cold/hot caches, eviction, Fold copy-on-write, randomized render order, and 1/2/4/8 native
   threads (scaffold now; assertions fill in as phases land).
5. Build with `--locked`; pin Rust toolchain (`rust-toolchain.toml` currently says moving `stable`),
   `Cargo.lock`, target-features, `wasm-opt`/Binaryen version + artifact hashes.
6. Add stage timers in the authoritative WebView2/V8 path (not bun/JSC — the 40× there is a tier-up
   artifact) so every later phase is measured where it ships.

**Gate:** cert asserts the anchor and goes red if the setHash/counts/byte-total drift. Commit.

## Phase 1 — Geometry mask interning/memoization  ⭐ highest leverage

`shape_mask` (`raster.rs:129`) is a pure function of `(shape, buffer_size, card_size, pad)` but is
recomputed every render; TS memoizes it. Corpus fact: 2,887 `shape_mask` calls collapse to **16
distinct keys at 256px** (99.45% warm-hit potential).

Tasks:
1. Session-owned mask cache keyed by `(shape, dims, raw offset bits, algo-version)`; return
   `Arc<[f64]>` / borrowed slices; share tile+card masks when identical; clone **only** before Fold
   carving (COW). ~~Hoist the polygon resolution to once per cold build (kill the per-test TLS hashmap
   lookup at `shapes/mod.rs:57`).~~ **DEFERRED to Phase 5 reprofile (owner-approved 2026-07-11):** the
   cache absorbs it — 2871/2887 corpus `shape_mask` calls are warm hits that never touch POLY_CACHE, so
   the hoist only speeds the 16 cold computes (ROI≈0), and it overlaps #57's just-hardened
   `shapes/mod.rs` (needless collision). Revisit only if the Phase-5 reprofile still shows POLY_CACHE
   material on the cold path.
   **✅ DONE — double-green (lead cert 0/389,808,128 every round + Codex PASS).** Commits: `f312867`
   (cache: MaskKey = algo+shape+buffer_size+shape_size+offset_x/y raw bits; FIFO byte-cap;
   eviction_cert co-committed) · `f8353b5` (ship the fast wasm feature + Shadow stamp routed through the
   cache with a correct None-shape exclusion + cap 8→16 MiB) · `b5e9de0` (wire `build:ship` = wasm&&build
   into Tauri `beforeBuildCommand` + regenerate the shipped `public/dm_icon_wasm.wasm` → 6d44b866 fast;
   per-step FIFO victim trace; permanent None-exclusion regression test). 3 adversarial rounds — byte
   safety green throughout (no P1 ever); the rounds closed the "does the speedup actually reach the
   shipped product" gap (the build pipeline wasn't producing the fast artifact). Estimated ~58% preview
   speedup (3×TS → ~1.3×TS near-parity), in the predicted 40-65% band; warmed-app absolute number is the
   last mile (the CDP bench can't tier wasm past Liftoff — a known artifact).
2. Gate mark-only work: when a cell renders no modern mark (1,081 of 1,429 non-original cells), skip
   `tile_alpha` / `MarkContext` / `composed_luminance`. **Do NOT skip final compositing** — it also
   canonicalizes hidden RGB under zero alpha (byte-load-bearing).
3. Byte cap the cache (a 16-mask 256px f64 cache ≈ 8 MiB/worker).

**Byte risk:** very low (pure function). **Certify:** compare raw mask `f64::to_bits()` on
miss/hit/eviction/random-order; identical-tile/card-key + Fold-COW tests; then the full RGBA cert.
**HARD GATE (Phase-0 audit #5/#10, folded here — a test can't precede the code it guards):** the SAME
commit that lands the cache MUST ship a **low-cap forced eviction test** (`f64::to_bits()` assertions
over miss → hit → forced-evict → reuse-slot → revisit, randomized order) — a stale-key/reused-slot or
nondeterministic-victim regression must go red. No deferring this past the cache. Plus the synthetic
shape×mark cross-product sweep (Phase-0 round-3) must stay green.
**Expected:** 50-75% on the polygon-heavy path; plausibly 40-65% corpus-wide — may alone recover most
of 3.85→1.27 ms. Commit.

## Phase 2 — Immutable source-fact caching + scratch reuse

Rust caches only `IconProfile` (`render_session.rs:25`); TS caches full analysis / segmentation /
scratch (`analysis.ts:501`, `segment.ts:79`, `compose.ts:491`).

Tasks: cache content/solid/foreground bounds, background detection, segmentation, dominant facts and
lightness by **source digest + schema key** (immutable values only, never partial sums); reuse
per-size shadow/blur scratch buffers.

**Byte risk:** low. **Certify:** cache-off/on differential, replacement/invalidation, randomized
order, source/schema-key tests, then RGBA cert. **Expected:** 5-20% warm remainder; 20-60% on an
exact reusable-stage hit. Commit.

## Phase 3 — Native rayon across icons (unblocked by the P2-7 RwLock arrow fix)

Icons are independent outputs. `NATIVE_ARROW` is now a process-global `RwLock` (`marks/mod.rs:184`),
so cross-icon parallelism is byte-safe.

Tasks: freeze config + arrow + hue-spread inputs, then `par_iter` independent icons into disjoint
outputs with indexed collection. **Do NOT** mutex `RenderSession::render(&mut self)` — expose
immutable registered inputs or per-task render contexts. Browser keeps its outer N-worker parallelism;
**no nested wasm threads.**

**Byte risk:** very low per icon. **Certify:** hash outputs at 1/2/4/8 threads + randomized job order.
**HARD GATE (Phase-0 audit #7, folded here):** the SAME commit that lands the real parallel collector
MUST wire the determinism scaffold to that ACTUAL collector (not the current per-thread independent-
session serial stand-in) and assert completion-order → input-order association is correct — a collector
that returns completion-order pixels labeled in input order must go red.
**Expected:** ~2.5-6× native render throughput (apply eventually becomes encoder/WAL/disk-bound). Commit.

## Phase 4 — Persistent content-addressed output cache

Key = `hash(full source digest, resolved 24-byte config, size, shortcut/original flags, field seed,
arrow digest, pixel-schema/kernel version, encoder version)`. Memory LRU (raw RGBA) + deterministic
PNG/ICO on disk (native). Use BLAKE3/SHA-256 (the current sequential `nextHash` is not persistence-safe),
atomic writes, byte-budget LRU, length+content verification, periodic recompute-and-compare sampling.

**Byte risk:** low only with a *complete* key. **Expected:** 90-99% of kernel work saved on a hit
(reopen / re-apply / slider-return); no help on a novel first render. Commit.

## Phase 5 — Reprofile, then guarded sRGB byte quantizer (only if pow still material)

After Phases 1-4, re-profile at 96px + 256px in WebView2. If `srgb_encode`'s `pow` (`color.rs:45`) is
still material: build a **guarded** table (Codex's safer variant) — a 4K-64K table whose cells are
either a proven output byte or `FALLBACK`; cells intersecting any byte transition call scalar
`libm::pow`. (A naked 255-threshold table first needs a monotonicity proof of the current libm mapping,
incl. the `0.0031308` segment join; the guarded table sidesteps that.)

**Byte risk:** low-medium. **Certify:** pin libm version; test all 255 transition neighborhoods, edge
guards, `0.0031308` branch, ±0, NaN/inf, millions of ordered-bit inputs, the existing 4097-point
fixture, and the corpus. **Expected:** 3-10% corpus-wide (Amdahl-capped). Commit.

## Phase 6 — Residual, measured-only (do last, each gated on a profile)

- Correctly-rounded hardware `f64::sqrt` on proven finite-nonneg domains + exact Glass distance tables
  (`glass.rs:81`, integer distances → exact `(size,distance)` cache; do NOT rewrite `pow(edge,1.5)`).
- Cold RGB→OKLab/dominant memo by 24-bit RGB (`color.rs:158`, ~2.3k unique RGBs vs 65,536 px).
- Strict lane-wise `f64x2` SIMD on **non-transcendental** independent-pixel arithmetic only (paired
  blur rows, alpha/clip, two independent dest accumulators). Hard rules: no horizontal FP reduction,
  no changed accumulation order, no FMA/relaxed/reassoc, no NaN-sensitive min/max; inspect emitted
  opcodes; certify all four scalar/SIMD × native/wasm.
- Pinned LTO + `codegen-units=1` + `wasm-opt -O3`, fast-math OFF.

## Red lines (both researchers agree — never, byte-breaking)

Vectorized `pow`/`cbrt`/`exp` (SLEEF is 1-ULP, not equal to this libm), JS `Math.pow`/`Math.cbrt`
(implementation-approximated), fast-math, FMA/`mul_add` contraction, relaxed-SIMD madd, Kahan/pairwise
reductions, horizontal SIMD, f32 fast paths, approximate/interpolated LUTs, algebraic rewrites like
`pow(edge,1.5)→edge*sqrt(edge)`, GPU/canvas/shader rendering. A FMA/fast-math CI guard (Phase 0.5)
protects all of the above from silent drift.

## Dependency / relation to M6 flip

This work is **independent of the flip decision** and worth doing regardless: it speeds the Rust
kernel (native apply/background now, wasm preview for the flip later) with zero byte risk. It also
**de-risks the flip** — with Phases 0-4 the preview approaches parity, so the "3× regression" argument
against flipping largely dissolves. Sequence: cert-harden → mask memo → source-fact cache → native
rayon → content cache → reprofile → (guarded sRGB) → SIMD/residual last.

## Verify (every phase)

`cargo test -p dm-icon-core` + the **hardened** `tests/icon-parity/m6` cert (0/389,808,128, setHash
asserted) + `bun tests/icon-parity/m5` battery + a WebView2 stage-timer before/after number. Phase-6
adversarial review (Codex) on each landed phase. No completion claim without the fresh cert + a bench
delta.
