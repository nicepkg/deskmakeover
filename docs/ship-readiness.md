# DeskMakeover — Ship-Readiness Inventory

> **What this is.** The authoritative, honest "what is left before a Windows user can install this
> and it works" list. A standing companion to `STATE.md` (which is the ~150-line pointer); this doc
> holds the detail. Update it in place as items close — it is a living tracker, not a dated snapshot.
>
> **Last reconciled:** 2026-07-12 (full repo audit — specs, plans, STATE, journal, code stubs).

## The one fact that frames everything

**Not one line of the Windows platform layer has ever run on Windows.** The entire
`dm-windows` / `dm-elevated` surface was blind-written on Mac, kept compiling via
`cargo check --target x86_64-pc-windows-msvc`, and unit-tested only through Mac fakes. Every real
COM / WIC / registry / shell call is `[WINDOWS-VERIFY]` and unproven at runtime. This is the
dominant ship risk and it colours every section below.

**Working model (owner decision 2026-07-12):** polish EVERYTHING that can be done + verified on this
Mac to near-perfection first; Windows is then only final integration + the `[WINDOWS-VERIFY]` runtime
pass. So this doc splits each gap into **Mac-closable now** vs **Windows-runtime-only**.

## Milestone status

| Milestone | Status | Remaining |
|---|---|---|
| **M3/M4 Windows platform layer (blind-write)** | Code DONE, **runtime 0% verified** | 55 Mac tests + msvc-clean, but the whole `[WINDOWS-VERIFY]` checklist (9 items) + 11 blind-audit follow-ups are all pending the Windows box. Two never-finished stubs: `shell/layout.rs` (positions), and `source.rs` icon extraction (see Ship-blockers §1). |
| **M5 icon core** | ✅ DONE + byte-certified (1,487-cell real corpus) | Only the held ICON-5/9/11 dead-code questions (§Owner decisions). |
| **M6 kernel-speed + WASM cutover** | ✅ EXECUTED (WASM is the only pixel path) | Physical deletion of the frozen TS compositor (`src/icon-compositor/*.ts`, 10 files) held to M8. |
| **M6-WIRE Wave A (wallpaper)** | DONE **on Mac only** | All Windows COM/WIC/topology (`topology.rs`, `decode.rs`, `wallpaper.rs`) `[WV]`. |
| **M6-WIRE Wave B foundation (B6-B9 + fs_atomic)** | ✅ DONE (Mac-green) | The four Windows durability defects in `m6-wire-host.md §8a` are **not Mac-fixable** and gate shipping. |
| **M6-WIRE Wave B icon bridge (B1-B5)** | ✅ **CONVERGED — codex R12 = Approve (2026-07-13)** | 8 adversarial rounds R5→R12, ~50 findings fixed (4🔴→1🔴→0🔴×5→Approve); owner-informed residuals in §Icon-bridge. Windows runtime still [WV]. |
| **M6-WIRE Wave B B10 (desktop watcher)** | ✅ DONE 2026-07-12 (`37f4b13`) | Real `notify`+`notify-debouncer-full`, Mac-live-verified (FSEvents), msvc-clean. 3 runtime semantics `[WV]` (self-write suppression / restart catch-up / overflow→rescan). |
| **M6-WIRE Wave C (Windows handoff doc)** | **NOT STARTED** | Spec'd at `docs/references/windows-wiring-handoff/README.md` (m6-wire-host §8); directory does not exist. No systematic Windows verification recipe yet. |
| **M7 resident auto-format** | **NOT STARTED** (design finalized: ADR-0022 + spec 07 + `m7-resident.md`) | `crates/dm-resident/src/lib.rs` is an empty crate (doc comment only), NOT wired into `src-tauri`. Tasks T1-T12 unbuilt. Precondition gate (B6-B10) is now green. |
| **M8 release engineering + .NET deletion** | **NOT STARTED** | No installer / signing / updater; version `0.0.0`; `legacy/` .NET tree still present. See §Packaging. |
| **M1 go/no-go spikes (Windows)** | PARTIAL | Only Spike 4 (tri-target pixel, Mac) done. Spikes 1/2/3/5 (STA+IFolderView, SysListView32 layout, elevated-helper roundtrip, kill-injected `.lnk`) are Windows-bound and **never run** — the ADR-0019 "gate for everything after" was never actually gated on a real box. |

## Ship-blockers — the critical path from "compiles" to "a Windows user installs it and it works"

Ordered by dependency. Tagged **[MAC]** (closable + verifiable here) or **[WIN]** (needs the box).

1. **[MAC] `WindowsIconSourceExtractor::extract` is an `Err` stub** (`crates/dm-windows/src/source.rs:39`).
   **The single most under-rated gap.** On Windows, `build_icon_host` wires this extractor
   (`src-tauri/src/lib.rs:95`), so `icons.scan` cannot obtain any real icon pixels — the icon module
   cannot function on the ship target at all. Masked on Mac by `devhost_icons`. Writing the body
   (`IShellItemImageFactory::GetImage` → 256px, batch) is blind-writable + msvc-checkable **here**;
   only the runtime pixels are `[WIN]`. This should arguably rank above finishing the bridge polish.
2. **[WIN] WebView2 bridge-transport** — the `a339196` fix (route via `isTauri()`, not
   `window.chrome.webview`) is itself `[WV]`. If wrong, the app never boots on Windows. #1 thing to
   confirm on the box.
3. **[WIN] `secure_dir` SDDL owner** (`de1617e`, `O:BA`) — without it every apply/restore after the
   first permanently fails on stock Windows. `[WV]`.
4. **[MAC→WIN] Pre-first-apply snapshot** must fire + persist durably BEFORE the first apply, or the
   first apply silently destroys the user's original desktop with no way back. Logic is Mac-testable;
   the durable-write guarantee is `[WIN]`.
5. **[WIN] The four §8a durability/atomicity defects** (all P1, data-loss / CAS-poisoning, none
   Mac-fixable): symlink-following `.lnk` temp; non-write-through "durable publish"; overlay snapshot
   not durable before the HKLM write; overlay snapshot-once cross-process race.
6. **[MAC] `shell/layout.rs` positions** (returns empty Vec), **`ItemKind::System`** read/anchor/apply
   (3 sites return `Unsupported` — This-PC/Network/Control-Panel CLSID icons unstyleable), and
   **`icons.exportCompare`** (`ok:false` both sides — the before/after sheet compositor doesn't exist).
   The compare sheet is fully Mac-buildable; System CLSID + positions are blind-writable + `[WIN]`.
7. **[WIN] M1 spikes 1/2/3/5** — the ADR-0019 go/no-go gates, never run.

## Genuinely unimplemented (stubs / `Err` / `ok:false`) — no `todo!()` in the tree, the discipline is honest failure returns

| Site | What it should do | Where |
|---|---|---|
| `source.rs:39` | Windows icon-source extraction (ship-blocker §1) | dm-windows |
| `shell/layout.rs:32` | desktop icon positions (returns `Vec::new()`) | dm-windows |
| `state_reader.rs:120,138` + `apply/mod.rs:63` | `ItemKind::System` read/anchor/apply (return `Unsupported`) | dm-windows |
| `icon_host.rs:454` + `mock-desktop.ts:382` | `icons.exportCompare` compare sheet | src-tauri + web |
| `crates/dm-resident/src/lib.rs` | the ENTIRE M7 crate (reconciler, queue, tray SM, consent, reset, switch, activity monitor) — 0% | dm-resident |

## `[WINDOWS-VERIFY]` surface (blind-written, msvc-clean, never run)

Grouped; each hides a real Win32/COM/WIC/registry seam. Full "run this / observe that" recipes owed
in the (unwritten) Windows handoff doc; the running checklists live in
`docs/plans/2026-07-10-m34-windows-blind.md` (items 1-20) and
`docs/plans/2026-07-11-windows-hardening-wave2.md §[WINDOWS-VERIFY] frozen ledger` (8 items).

- **Icon writers** — `apply/{shortcut,folder,url_shortcut,file_wrapper,recyclebin}.rs`.
- **Shell reads** — `shell/{layout,shell_link,known_folders,attrs}.rs`, `scan.rs`.
- **Adapters** — `state_reader.rs`, `wallpaper.rs`, `topology.rs`, `overlay.rs`, `durable.rs`,
  `source.rs`, `refresh.rs`, `com/{sta_actor,apartment}.rs`, `watcher.rs` (3 runtime items only).
- **Elevated helper** — `dm-elevated/src/{overlay,secure_dir}.rs` (also blake3 blocks even the msvc
  cross-check on Mac).
- **dm-operations seams** — `txn/asset_store.rs`, `txn/driver.rs`, `fs_atomic.rs`, `wallpaper/decode.rs`.
- **src-tauri composition** — `lib.rs` (whole Windows icon composition blind-wired, active-profile
  count hardcoded to 1), `wallpaper_host.rs`, `icon_host.rs` (custom-protocol WebView2 form,
  `SPI_GETWORKAREA` grid, real IFolderView2 positions).

## Packaging / release gaps (M8 — all undone)

- **Version `0.0.0`** in `src-tauri/tauri.conf.json` + root `package.json`. Owner names the first release number.
- **No installer config** — no NSIS/WiX block, no `webviewInstallMode` (WebView2 bootstrapper), no per-machine/per-user choice.
- **No code signing** — no Authenticode / RFC-3161 timestamping.
- **`dm-elevated.exe` not packaged** — `src-tauri` has no `externalBin`/sidecar/resources entry; the
  overlay path resolves the helper at runtime from `current_exe().parent()/dm-elevated.exe`, so
  packaging MUST build + place it (with its `requireAdministrator` manifest) next to the main binary.
- **M7 build deps missing** — `tauri` features has no `"tray-icon"`; no `tauri-plugin-notification`,
  no autostart plugin (spec 07 §16 needs all three). No tray bitmap assets.
- **No CI** — `.github/workflows/` does not exist; all gates (generated.ts drift, byte-parity cert) are local-only.
- **`.NET` legacy tree not deleted**; frozen TS pixel compositor not physically deleted (tree-shaken only).
- **First-run/onboarding** — welcome-gate built on web; M7 first-format consent strip + resident onboarding unbuilt.
- Owner-gated release blockers: first version number, making the private repo public, the signing cert, the extracted `public/real-icons/` pack sign-off.

## Icon-bridge status: ✅ CONVERGED — codex R12 = Approve (2026-07-13)

The B1-B5 bridge went through **12 adversarial codex rounds** (R1 13 · R2 8 · R3 5 · R4 7 · R5 7 ·
R6 6 · R7 6 · R8 4 · R9 2 · R10 2 · R11 2 — ~50 findings, every one 二次核实'd then fixed with a
regression test; trajectory R5 4🔴 → R6 1🔴 → R7–R11 0🔴 → **R12 Approve**). Final verdict: the
Mac-verifiable Rust transaction kernel, CAS/recovery, host fencing, error contract, and TS
single-flight bridge are "高质量、fail-closed、可恢复的收敛状态". Gates at convergence: cargo
workspace 524 · dm-operations 167 · deskmakeover-desktop 26 · tsc · bun 516 · vite · bindings.
Structural highlights: single-flight replaced the generation guard (R4); `desktop_mutated` /
`repair_pending` / `intent_persisted` / `requires_rescan` outcome contract (R5–R9); the scan-revision
FENCE + explicit `scan_valid` (R9–R11) closing the poison/manual-restore ABA.

**Owner-informed residuals (codex-enumerated, all non-blockers for the Mac convergence):**
durable poison tombstone (one-round app-poison heal needs provenance; today = drop+conflict+rescan) ·
frontend auto-rescan after a fence (structure safe; UX is toast-only) · structured skip reasons
(`conflicts` folds 5 causes; toast copy 语义过宽) · ①→②→③ finalize + reset crash-windows
(unjournaled, self-healing, documented) · zero-byte-undeletable-log fault-injection test gap ·
re-apply rollback lands on true original (ADR-0019 semantics) · arrow marker is best-effort
user-level (real registry reconcile = [WV]) · the full [WV] battery + `WindowsIconSourceExtractor`
stub + live positions + System CLSID (= the Windows ship-blockers tracked above). The R5 table below
records the state-space gaps the R4 guard-removal exposed (historical):

| R5 # | Finding | Fix |
|---|---|---|
| 🔴 #1 | driver rollback/abandon moves the desktop but host still shows "桌面没有改动" (only keep-restore `reverted` was checked, not the driver's own `rolled_back`). | `desktop_mutated` flag on the outcome; host degraded toast when set. |
| 🔴 #2 | a poisoned lingering row re-styled DIRECTLY bypasses the heal arm → permanent CAS conflict; an all-conflicts batch still writes ②③ + reports ok:true. | `prepare_item` heals (current==original≠last_applied) then fresh-applies; ②③ gated on `!committed.is_empty()`; `Toast_ApplyNoEffect`. |
| 🔴 #3 | a clean recovery that ABORTED an interrupted txn moved the desktop, then a later bare `?` (package validate / store read) surfaces over it. | commit_apply/reset defer (resync) when `!aborted.is_empty()`, not only on degraded. |
| 🔴 #4 | both-terminals corruption Err fires INSIDE the drain loop — after an earlier incomplete txn was already aborted (desktop mutated). | structural preflight over all txns before any mutation. |
| 🟠 #5 | `Promise.all([scan,getPersisted])` fail-fast releases the single-flight lock while the still-running `scan` orphan-publishes + desyncs the revision. | `fetchScan` sequences getPersisted (publish-free) then scan. |
| 🟠 #6 | startup recovery swallows `degraded`; a fresh-incomplete txn styles the desktop with no ledger row → `applied:false` hides the only restore entry. | get_persisted forces `applied:true` off an in-flight journal (`active_txns`); startup logs degraded loudly. **[WV]: the cfg(windows) startup branch can't be msvc-checked from Mac (rusqlite baseline) — verify on the box.** |
| 🟠 #7 | 32MB LRU can evict the generation it is publishing (256px high-entropy × 200 icons > cap in one scan) → live 404. | `publish` pins the current generation; `trim` evicts only historical keys. |

_R1–R4 (33 findings) all fixed earlier; R4's Major 1+1b deleted the generation guard for strict single-flight, R4's B1–B5 hardened the degraded error-contract. Detail swept to `docs/journal/2026-07.md`._

## Open owner decisions (from STATE.md)

- **ICON-5 / ICON-9 / ICON-11** — HELD dead-code questions (rounding, mono branch, 5 `color.rs` fns).
- **Audit #7 + 3 marginal P3s** — owner-approved GO, not started (`diagnostics.getInfo`, ELEV-3, APPLY-3, CORE-1).
- **Preset collection v2**, **two-axis colour reshape**, **shortcut-mark default** (badge ON vs decreed None), **ADR-0016 badge lightness** — all owner-pending.
- **Release identity** — first version number, repo public, signing cert, `public/real-icons/` ship sign-off.

## Honest summary

The Mac-side product (UI, icon core, wallpaper wiring, icon bridge, storage foundation) is genuinely
built. **Essentially nothing on the Windows ship target has been validated at runtime.** Under
decision A, the near-term Mac work is: (1) converge the icon-bridge contract (R5 fixed, R6 verifying), (2) write the two
Mac-buildable stubs (`source.rs` extraction, `exportCompare`), (3) finish the Mac-buildable halves of
the `[WIN]` seams so the Windows pass is verification-only, (4) author the Windows handoff doc so the
box work is a checklist, then (5) M7 build + M8 packaging. Steps that are irreducibly Windows-runtime
(the §8a durability defects, the M1 spikes, the whole `[WINDOWS-VERIFY]` battery) remain the final
integration pass on a real, logged-in Windows box.
