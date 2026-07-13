# Rust Full-Audit Round 2 — Dispositions & Fixes (2026-07-14)

> **What this is.** Round-2 carpet codex review of the whole Rust workspace + Rust↔frontend bridge,
> run the day after round 1 ([2026-07-13-rust-full-audit.md](2026-07-13-rust-full-audit.md)). Four
> parallel codex slices (A txn/recovery+regression · B bridge · C icon-core · D windows blind),
> each given the round-1 closed-list so it went for NEW ground + regressions rather than re-litigating.
> 26 findings. calm/清爽 (`system_tweaks/**`, `calm/**`, m6 plans) EXCLUDED — concurrent session.
>
> **Discipline.** Every fixed finding got 二次核实 (read the source, confirm real) before the edit.
> All commits explicit-path-isolated from the calm session, gated, NOT pushed (owner-gated).
> Baseline before changes: `cargo test --workspace` = 654 passed.

## Fixed (committed on main, not pushed)

| Commit | Findings | Summary |
|---|---|---|
| `75c5607` | A-1, A-2 | recovery never-clobber reconciles deletion + stale/poison ledger rows |
| `cadbe13` | D-2, D-3, D-4 | System/RecycleBin blind-layer: restore CLSID re-validate, HKCR fallback, opt-in system-icon default |
| `41ff448` | C-1, C-3 | non-square analysis panic + WASM render buffer-overrun guard |
| `afc871c` | C-2 | ICO validator enforces the writer's exact DIB contract |
| `52369e5` | B-2, B-3, B-7 | overlay finalize honesty (apply/switch fold outcome; set_arrow returns Result) |
| `78e27cf` | B-1, B-11, B-12, B-8 | wallpaper content-addressed URL + decode-off-lock; icon PNG move; DTO drift |
| `220ef5b` | C-5 | skip discarded analysis on no-look + original renders (byte-identical) |

### Finding detail (fixed)

- **A-1 🟠** `recovery.rs` — a deleted target (`read_fingerprint` → NotFound) was recorded `degraded`,
  withholding the journal checkpoint forever → every future apply/reset deferred on a phantom crash.
  NotFound is now a FINAL preserved outcome; the fence lifts.
- **A-2 🟠** `recovery.rs` — the never-clobber preserve branch left a committed ledger row that
  contradicted the live desktop (a prior-txn style + a user edit) → poison row. Now reconciles: a row
  still matching live is kept (correctly-tracked prior style); any mismatch/deletion drops it.
- **D-4 🟠** `apply/system.rs` restore interpolated the anchor CLSID into an HKCU path with no
  validation/target match → a corrupt/stale ledger could mutate the WRONG key + report success. Now
  re-derives + requires an exact target-CLSID match (apply already did).
- **D-2 🟠** `source.rs` — an empty per-user Recycle Bin key made `extract_recycle_bin` fall to the LIVE
  (possibly styled) shell image instead of the machine default. Added `recyclebin::machine_state()`
  HKCR fallback, mirroring `original_system`.
- **D-3 🟠** `shell/scan.rs` — absent `HideDesktopIcons` value/key was treated as "shown", injecting
  This PC / User Files / Network / Control Panel as phantom desktop tiles on a clean profile. These
  four are opt-in: an explicit DWORD 0 is now required.
- **C-1 🟠** `dm-icon-wasm/lib.rs` — `dm_session_render` wrote size²·4 bytes into the caller's fixed
  256²·4 buffer with no capacity check → size > 256 overran linear memory. `MAX_RENDER_SIZE` guard
  (code 6) before rendering; JS loader rejects too.
- **C-2 🟠** `dm-icon-codec/ico.rs` — `parse` validated only that bytesInRes matched a 32-bpp calc; a
  crafted header (biSize/planes/bit-count/compression, or a PNG frame) passed to the elevated helper.
  Now enforces the writer's exact BITMAPINFOHEADER contract (also rejects palette + PNG frames).
- **C-3 🟠** `analysis/background.rs` — `try_uniform_rect_ring` used width for both axes → `pixel_at`
  (no bounds check) panicked on a non-square raster reaching SourceFacts/free-renderer/batch. Now
  dimension-safe; byte-identical for square masters.
- **B-2 🟠** `icon_host.rs` apply computed ok/toast from the icon txn alone; a declined/failed/errored
  overlay returned ok:true with the native arrow still showing. Now folds the overlay outcome in.
- **B-3 🟠** `icon_host.rs` `set_arrow` persisted best-effort; a lost `Hidden` marker → restart residue
  (host forgets the install, skips restore). `set_arrow` now returns the write result; a lost install
  marker surfaces. [WV]: the complete fix also probes the real registry state on startup/before restore.
- **B-7 🟠** `icon_host.rs` `switch_version` committed styled icons but never installed the overlay
  (browser mock did → mispredicted Windows state). Now installs + folds like apply.
- **B-1 🟠** `wallpaper_host.rs` protocol URL used a per-process `rev` (reset to 1 each launch) → a
  restart could serve a prior immutable WebView2 image for new content. Now a content-hash `?v=`.
- **B-11 🔵** `wallpaper_host.rs` decode held the cache mutex across the full image decode → `png_for`
  and other monitors blocked. Decode now runs outside the lock.
- **B-12 🔵** `icon_host.rs` cloned each extracted source PNG though extract returns owned + drops them.
  Now consumes (into_iter) and moves the buffer.
- **B-8 🔵** `bridge/types.ts` `LookVersionDto.createdAt` was `number` vs the generated `number | null`
  (Rust `Option<i64>`); a null timestamp would violate it. Aligned; `formatTime` handles null (tsc
  surfaced the `parseHistory` consumer).
- **C-5 🔵** `render_session.rs` ran both analyses before the look-check and for show_original (which
  ignores them). Now skips analysis when its result is discarded — byte-identical.

## Recorded — not a plain fix (owner decision / [WINDOWS-VERIFY] / deeper follow-up)

- **D-1 🟠** `shell/scan.rs:88` — a fresh regular file scans `Ready` + `requires_explicit_consent=false`
  with NO production consent transition, so Apply may wrap it (create `.lnk`, Hidden+System the
  original) without the per-file confirmation the design implies. **OWNER product decision**: do we
  require explicit per-file consent before wrapping a loose file, and what is the acknowledgement flow?
- **B-5 🟠** `icon_host.rs:986` — `SourceCache` pins the whole current generation and stops trimming
  when only pinned keys remain, so one scan of thousands of high-entropy 256² icons can hold hundreds
  of MiB past the 32 MiB cap (never evicts a live key mid-serve = a correctness reason). **OWNER
  policy**: a hard per-scan item/byte budget → a bounded/degraded scan means dropping some icons'
  previews; which policy (drop oldest / show a warning / raise the cap)?
- **B-6 🟠** `bridge/tauri.ts` + `stores/app.ts` — `app.getInfo` isn't in HANDLED, so under Tauri the
  startup schema handshake compares the web const against itself (mock) and version shows 0.0.0. FIX:
  Rust `app_get_host_info` → `HostInfoDto{schemaVersion,version}` (Rust `BRIDGE_SCHEMA_VERSION` const +
  `env!(CARGO_PKG_VERSION)`) + specta regen + a new bridge method used by the handshake. **Deferred**:
  heaviest plumbing for the lowest impact (host+bundle ship together → prod mismatch ~impossible).
- **B-4 🟠** `src-tauri/src/lib.rs:105` — `IconHost::new(..., 1, ...)` hard-codes `activeUserProfiles=1`,
  so a multi-profile PC lets the user dismiss a non-skippable disclosure. Real count needs the Windows
  ProfileList enumeration — **[WINDOWS-VERIFY]**, blind-writable but box-confirmed.
- **C-4 🟠** `dm-icon-codec/ico.rs:106` — a fully-transparent overlay frame uses all-zero alpha + an
  all-zero AND mask; a legacy zero-alpha decoder reads that as opaque black. **[WINDOWS-VERIFY]**: test
  the overlay through the real Shell path; if the heuristic applies, set AND-mask bits to 1.
- **C-6 / C-7 🔵** `output_cache.rs` + `batch.rs` — the content-addressed output cache and the Rayon
  multi-icon batcher have NO production caller (native bake/switch/reconcile call RenderSession
  directly, sequentially). Their advertised warm-render + parallel speedups are currently zero →
  wire with the native resident/version-switch loop (**T8**, the recorded unwired-decision-core gap).
- **C-8 🔵** `marks/styles.rs:411` — Glass runs a full-tile 4-channel f64 `backdrop_blur` but reads only
  the ~34% seat. **Deferred (parity-sensitive)**: needs a windowed blur that outputs only the seat±blur
  radius ROI while SAMPLING the full tile with the same tile-relative clamping (byte-identical at the
  seat), plus a targeted parity test (seat near a tile edge). Left for test-first dedicated work.
- **B-9 🔵** `icon_host.rs` (~1100 prod lines) is a god-module coordinating 7 ports + 4 mutex state
  areas. Split candidate: `scan` / `source_cache` / `apply_session` / `mutations` / `overlay_state` /
  `persisted` / `export` behind an `IconHost` façade. **OWNER**: worth the churn now, or defer?
- **B-10 🟡** `bridge/mock-desktop.ts` accepts apply sequences the real host rejects (no revision/count/
  budget/style-envelope checks), so a regression passes browser dev but fails at Tauri. Dev-parity
  hardening — low priority.

## Prior round-1 residuals (unchanged, still open)
F1 CAS atomicity [WV] · F4b IPC payload size [WV] · F6 elevated packaging (M8) · intended-fp L2 (owner
declined) · never-clobber full-restore-surface identity [WV].

## Gates (this round)
`cargo test --workspace` 654 baseline → all touched crates re-verified green · msvc dm-windows/dm-domain
(--tests) clean · tsc clean · check:bindings clean. NOTE: `cargo test --workspace` can transiently fail
in `system_tweaks` while the calm session edits its WIP — attribute + verify own crates independently.
