# DeskMakeover — Ship-Readiness Inventory

> **What this is.** The authoritative, honest "what is left before a Windows user can install this
> and it works" list. A standing companion to `STATE.md` (which is the ~150-line pointer); this doc
> holds the detail. Update it in place as items close — it is a living tracker, not a dated snapshot.
>
> **Last reconciled:** 2026-07-13 (icon-bridge convergence + extractor + ledger-aware source +
> exportCompare + live positions + M7 resident decision core + version switch — all Mac-green;
> plus the owner's "这些都做" pass over §Open owner decisions: tray any-state-disable, ICON-5/9/11,
> audit #7 + ELEV-3/APPLY-3/CORE-1, shortcut-mark None, preset-v2 guards, two-axis dispositions,
> badge-lightness declined — all Mac-green + committed, Release identity still owner-only).

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
| **M7 resident auto-format** | **DECISION CORE DONE + HARDENED (Mac, 2026-07-13)** — platform bodies + tray [WV] | `dm-resident` built + 24 tests: reconciler (T6), tray SM (T7), pending queue (T5), consent ladder, stability probe, privileged red-line (T12). Plus style_resolve + native_bake (T1 port), version_switch (T10, wired `icons.switchVersion`), reset toggle-coupling, resident precondition, T2 WindowsActivityMonitor (judge-2, msvc-clean). **Two codex adversarial rounds** (apply-path + policy) → 2🔴+9🟠+5🟡 ALL closed: propose→apply snapshot-CAS contract, §14 scope re-check at every write entry + shared path-ancestry, unconditional recovery, busy-abort, v1-always-proposes, atomic reset, nanosecond stability. **Remaining = [WV] platform bodies:** T8 tray+windowless residency wiring (unwritten), T11 tray bitmaps (uncreated), the watcher→reconciler→driver LOOP wiring, T2's judge-1 WinEventHook precision layer. |
| **M8 release engineering + .NET deletion** | **NOT STARTED** | No installer / signing / updater; version `0.0.0`; `legacy/` .NET tree still present. See §Packaging. |
| **M1 go/no-go spikes (Windows)** | PARTIAL | Only Spike 4 (tri-target pixel, Mac) done. Spikes 1/2/3/5 (STA+IFolderView, SysListView32 layout, elevated-helper roundtrip, kill-injected `.lnk`) are Windows-bound and **never run** — the ADR-0019 "gate for everything after" was never actually gated on a real box. |

## Ship-blockers — the critical path from "compiles" to "a Windows user installs it and it works"

Ordered by dependency. Tagged **[MAC]** (closable + verifiable here) or **[WIN]** (needs the box).

1. ~~[MAC] `WindowsIconSourceExtractor::extract` is an `Err` stub~~ → ✅ **BODY BLIND-WRITTEN
   (2026-07-13), runtime = [WV]**. Full implementation, oracle `ShellIconCanvasSource.cs`:
   shortcut icon-resources via `PrivateExtractIconsW` (best ≤256 frame, `ExtractIconExW` classic
   fallback) ⇄ `IShellItemImageFactory::GetImage` (`ICONONLY|BIGGERSIZEOK`, premultiplied→straight
   un-premultiply) two-way chain; Recycle Bin full+empty pair from the per-user CLSID `DefaultIcon`
   registry values; HICON via `GetIconInfo` colour plane (straight alpha + AND-mask legacy
   fallback). Pure helpers (premul math / icon-location parse / %ENV% expand) Mac-unit-tested;
   msvc-clean, zero warnings. **[WV]: real pixels on the box** — shell image fidelity, Appx logo
   quality via the shell path, empty-bin pairing, alpha edge cases.
   **codex extractor review (2026-07-13): 2🔴+3🟠+2🟡 — ALL CLOSED.** Then a SECOND codex review of
   the ledger-aware + scanner-injection work (2026-07-13, "icons2"): Request-Changes with 4🔴+5🟠+3🟡,
   ALL CLOSED same-day (commit `893914d` + `69ed939`): 🔴1 committed-but-unledgered txn → the scan
   overlays the JOURNAL on the ledger (Prepared anchor + Applied fingerprint), incomplete txn →
   provenance-unknown degrade; 🔴2 epoch fence (a mutation during the slow extraction rejects the
   publish); 🔴3 terminal anchor (an anchor-present item resolves a trusted original or Errs → per-item
   degrade, never reads the styled live surface; folder desktop.ini gains IconFile/IconIndex + UTF-16
   + relative-path; bin full/default tried independently); 🔴4 compare-sheet CORS (both frames are
   compositor data: URLs); 🟠5 one apply-authority bit `ScannedItem.source_ok` (DTO styleable +
   commit acceptance + restore planner share one definition; empty sentinel no longer carries
   authority); 🟠6 wrapper provenance via a durable `SetDescription` marker (not structural guessing)
   + reparse guard; 🟠7 before/after fidelity; 🟠8 export size caps + PNG-only; 🟠9 atomic export;
   🟠12 unpredictable atomic scratch files; 🟡10 degraded tile clears; 🟡11 mock DOM-gate. All the
   original OPEN items (ledger-aware extraction, Recycle Bin injection, per-item degradation, mono
   HICON, long-path fallback) are DONE.
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
6. ~~[MAC] `shell/layout.rs` positions~~ → ✅ **BLIND-WRITTEN (2026-07-13)**: technique A
   (`IFolderView2::GetItemPosition` walk) behind a `DesktopGeometryReader` port; host matches live
   slots by name, degrades to the synthetic grid; msvc-clean, runtime `[WV]`.
   ~~`icons.exportCompare`~~ → ✅ **DONE (2026-07-13)**: webview composes the branded before/after
   sheet (both frames compositor-rendered → same-origin data: URLs, no CORS taint), Rust validates +
   atomically saves to Pictures. **`ItemKind::System`** read/anchor/apply (3 sites still return
   `Unsupported` — This-PC/Network/Control-Panel CLSID icons unstyleable) remains blind-writable + `[WIN]`.
7. **[WIN] M1 spikes 1/2/3/5** — the ADR-0019 go/no-go gates, never run.

## Genuinely unimplemented (stubs / `Err` / `ok:false`) — no `todo!()` in the tree, the discipline is honest failure returns

| Site | What it should do | Where |
|---|---|---|
| ~~`source.rs`~~ | ✅ DONE — full extractor + ledger-aware + journal overlay, runtime `[WV]` | dm-windows |
| ~~`shell/layout.rs`~~ | ✅ DONE — technique A positions + geometry behind `DesktopGeometryReader`, runtime `[WV]` | dm-windows |
| `state_reader.rs:120,138` + `apply/mod.rs:63` | `ItemKind::System` read/anchor/apply (return `Unsupported`) — STILL a stub | dm-windows |
| ~~`icon_host.rs` exportCompare~~ | ✅ DONE — webview composes, Rust validates + saves | src-tauri + web |
| ~~`crates/dm-resident`~~ | ✅ DECISION CORE DONE + codex-hardened (24 tests) — reconciler/queue/tray-SM/consent/stability/version-switch. Remaining = T8 tray/residency wiring + the reconcile-loop driver, `[WV]` | dm-resident |
| ~~`WindowsActivityMonitor` (T2)~~ | ✅ judge-2 synchronous poll written + msvc-clean; judge-1 WinEventHook precision layer `[WV]` | dm-windows |
| `src-tauri` tray (T8/T11) | tray-icon feature, §12 menu, windowless close handler, autostart, tray bitmaps — NOT WIRED | src-tauri |

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

## Open owner decisions — RESOLVED 2026-07-13 (owner order "这些都做")

The owner dispositioned the whole list "都做". Each is resolved below with its evidence; a few
resolve to "already true / retain / decline" on evidence, which is called out honestly rather than
built to look busy. Only **Release identity** stays owner-only (it cannot be resolved by an agent).

- **Tray toggle-off from any state** (M7 codex m7b-🟠2) — ✅ **DONE (`ae48aba`).** `ToggleOff` is now
  legal from ANY state → OFF (spec §12 + new §12.1): Working→Off relies on per-batch `TxnDriver`
  atomicity (never a torn partial); Error→Off retains the fault record via the unconditional
  `recover_from_journal` at the next enable. +1 state-machine test.
- **ICON-5 / ICON-9 / ICON-11** — ✅ **RESOLVED (`5ac018a`).** ICON-5 (`clamp_u8_round_half_even`)
  DELETED — a Rust-only orphan (no named-oracle mirror) modelling a store the oracle never triggers,
  with a lying doc. ICON-9 (mono per-pixel tail) + ICON-11 (5 `color.rs` fns) **RETAINED, documented**:
  each is a 1:1 port of a NAMED frozen-oracle export (`color.ts`), dead in the oracle too; deleting the
  Rust halves would diverge the certified byte-parity port (ADR-0019) for zero pixel gain.
- **Audit #7 + 3 marginal P3s** — ✅ **DONE (`c8ded4a`).** `diagnostics.getInfo` is a real Rust command
  (no longer the `(mock)` stub that shipped on Tauri); ELEV-3 (CommandLineToArgvW quoting, new
  `cmdline.rs`, Mac-tested), CORE-1 (`WaitForSingleObject` result checked), APPLY-3 (non-string
  `DefaultIcon` type fails closed) all hardened; msvc-clean + bindings drift-guard green.
- **Shortcut-mark default (badge ON vs decreed None)** — ✅ **RESOLVED = None (`d1f507d`).** The owner
  decree 2026-07-07 + the durable rule 「presets never carry a shortcut mark」 win over the panel's
  badge-ON. Code already ships None everywhere (every base config `shortcutShape:null`;
  `shortcutShape` is not a `TYPE_PATCH_KEY` so a type override cannot re-add it). Now locked with a test.
- **Preset collection v2** — ✅ **RESOLVED (`d1f507d`).** Factory lineup confirmed = the seven presets,
  spectrum default (test-locked). Owed guards closed: fixed-plate silhouette-shadow regression
  (STATE §-1a ①), retired-#65470D guard. Genuinely MOOT items disposed, not faked: the colour-migrate
  test (the schema 3→4 migration already happened; no live mapping to test) and the folder-drift probe
  (folders derive by design now — see two-axis) are obsolete; alpha-plate is out of scope ("plumbing
  not algorithm"). spec 02/06 amended off the legacy `colorMode` model.
- **Two-axis colour reshape** — ✅ **RESOLVED.** Folders **derive their plate by design** (各自色):
  the earlier 统一金 pick is superseded by the v2 acceptance; `#65470D` 深金板 is retired (survives only
  as a user swatch). The "multicolour folders" the designer saw are the INTENDED derive lane, not a bug.
  灰暗文件板 = **option A 暖纸色板 already shipped** (File plate `#E9E2D4`). Owed tests/spec amendments
  closed with preset-v2 above.
- **ADR-0016 badge lightness** — ⛔ **DECLINED for v1 (frozen-oracle constraint).** The ring-seam
  lightness polish would land in `icon-compositor/marks.ts`, which is FROZEN as the parity oracle
  (ADR-0019: "no fixes except oracle corrections") with no reserved slot; a cosmetic tweak there would
  break byte parity. Revisit only after the compositor unfreezes at certification. Not a defect — a
  deliberate deferral.
- **Release identity** — 🔒 **OWNER-ONLY (cannot be agent-resolved).** First version number, making the
  repo public, the Authenticode signing cert, and the `public/real-icons/` ship sign-off are all your
  calls / actions. **Ace's strong recommendation: stay `Unreleased` (or a `0.x` pre-release) until the
  Windows runtime pass — not one line of the Windows platform layer has ever run on Windows, so a `1.0`
  would be dishonest.** Making the repo public + obtaining the cert are things only you can do.

## Honest summary

The Mac-side product (UI, icon core, wallpaper wiring, icon bridge, storage foundation) is genuinely
built. **Essentially nothing on the Windows ship target has been validated at runtime.** Under
decision A, the near-term Mac work is: (1) converge the icon-bridge contract (R5 fixed, R6 verifying), (2) write the two
Mac-buildable stubs (`source.rs` extraction, `exportCompare`), (3) finish the Mac-buildable halves of
the `[WIN]` seams so the Windows pass is verification-only, (4) author the Windows handoff doc so the
box work is a checklist, then (5) M7 build + M8 packaging. Steps that are irreducibly Windows-runtime
(the §8a durability defects, the M1 spikes, the whole `[WINDOWS-VERIFY]` battery) remain the final
integration pass on a real, logged-in Windows box.
