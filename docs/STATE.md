---
updated: 2026-07-12 night (full-repo audit + fix run: 11 commits landed, all gates green — docs/reviews/2026-07-12-audit-fix-run.md · M7 常驻设计FINALIZED + 4 owner dispositions APPROVED — ADR-0022, docs/reviews/2026-07-12-m7-resident-panel.md · roadmap: M6-WIRE Wave A(壁纸)DONE → Wave B foundation B6-B9+fs_atomic DONE + **B1-B5 icon bridge DONE (2026-07-12, schema 7, D1-thin — Rust scan/package+apply/restore/②③-persist returns thin data; frontend assembles IconsStateDto) + 3-ROUND codex adversarial loop CONVERGING: R1(13)+R2(8)+R3(5) findings all fixed w/ regression tests — R3-Block1 session-token single-flight, Block2 op_gate serializes overlay calls, Block3 busy()-gates draft/rescan/crossing, Block4 degraded error-contract (never bare Err over a mutated desktop), Major5 two-gen dmicon cache; codex R4 came back Request-Changes (5 Block + 2 Major, several in the R3 error-contract fixes) — ALL FIXED: B1 reverted-not-nothing-changed, B3 applied-fail-closed, B4 restoreOverlay-preread, B2 CAS-poison-heal, B5 recovery-degraded, Major1+1b single-flight REPLACES the generation guard (nextGen/isCurrentGen deleted), Major2 byte-bounded-LRU cache; codex R5 came back Request-Changes (4🔴+3🟠, several in the R3/R4 fixes) — ALL FIXED (0cecacb/1fdd611/4c18bc7): #1 desktop_mutated flag (rollback≠"nothing changed"), #2 prepare_item heals poisoned re-apply + zero-effect no-②③ + Toast_ApplyNoEffect, #3 clean-recovery-abort defers, #4 both-terminals structural preflight before any mutation, #5 fetchScan sequential (no orphan publish), #6 get_persisted active_txns→applied fail-closed + startup logs degraded, #7 LRU pins current generation. codex R6 (4🔴+3🟠 residuals of the R5 fixes: committed-unreconciled repair signal, revert-only ②③, stale-scan heal, checkpoint/deferred/comment 🟡s) ALL FIXED (901b83f/df0b61a); codex R7 (no 🔴, 3🟠+🟡: heal CAS-bypass regression, complete-Apply ②③ gate w/ repair-guard, hand-edit-revert no-effect, empty-journal defer, auto-format-off swallow) ALL FIXED (bcc09c2/ccde4d3); codex R8 (heal ABA-safe) + R9 (scan-revision fence + intent_persisted) + R10 (healed survives batch failure + fence=no-valid-scan) + R11 (superseded-scan zero-side-effect + explicit scan_valid) ALL FIXED — **codex R12 = APPROVE (2026-07-13): the icon bridge is CONVERGED.** 8 adversarial rounds R5→R12, ~50 findings fixed, trajectory 4🔴→1🔴→0🔴×5→Approve. Verdict: "Mac 可验证的 Rust 事务内核、CAS/recovery、host fencing、错误契约和 TS single-flight bridge 已达到高质量、fail-closed、可恢复的收敛状态"。Owner-informed residuals (non-blockers, see ship-readiness §Icon-bridge): durable poison tombstone · frontend rescan-after-fence UX · structured skip reasons (conflicts toast 语义过宽) · ①②③ finalize + reset crash-windows (unjournaled, self-healing) · zero-byte-log fault-injection test gap · [WV] battery. Mac-green: cargo workspace 524 (dm-operations 167 · deskmakeover-desktop 26 · dm-windows 53) + tsc + bun 516 + vite + bindings drift-guard; msvc compile-check baseline-blocked by blake3 asm (zero dep edits, portable Rust))** + **B10 desktop watcher DONE (+ codex-review hardened: non-recursive, 4s, join-on-drop, notify-8.2 Windows-overflow KNOWN-LIMITATION documented) (2026-07-12, 37f4b13 — real notify+notify-debouncer-full, cross-platform so debounce/event-map core Mac-live-verified via FSEvents; msvc-clean; 3 runtime semantics [WV] = self-write suppression / restart catch-up / overflow→rescan, documented for Windows box in m34-windows-blind item 9)**, Wave B (B1-B10) now COMPLETE on Mac side = M7 precondition gate GREEN; icon source extraction over dmicon:// built on Mac (devhost synth) + [WV] on Windows (WindowsIconSourceExtractor stub); LIVE supervised icon-apply on Mac-Tauri is the owner gate (never auto-run) · #7 diagnostics+3 marginal P3 owner-go, not started · ICON-5/9/11 held, owner has not ruled. Prior (swept to journal): M6 kernel-speed Phase 0-4a + single-truth WASM flip EXECUTED, wave-2 hardening CLOSED, arrow-restore UX DONE, M6-WIRE Wave A wallpaper wiring DONE)
version: Unreleased (Directory.Build.props + Web package.json both 0.0.0; the owner names the first release number; the About-line + in-app changelog narrative is RESTORED per ADR-0013 amendment)
branch: main — synced with origin/master (repo exists on GitHub but is PRIVATE; making it public is the owner's call)
---

# State

Completed work is swept to `docs/journal/2026-07.md` (append-only). This file is a
pointer: what is TRUE now, what is in flight, what comes next.

> 🚨 **REPLATFORM DECIDED (2026-07-10, owner + 4-seat adversarial panel incl.
> Codex).** The product moves to **Tauri 2 + Rust**; .NET exits. One Rust icon
> core (WASM preview + native apply/background) is the single pixel truth in
> v1.0; the TS compositor is FROZEN as the parity oracle until certification;
> background resident auto-format ships in v1 (spec 07); the global transparent
> arrow overlay is the default and the 60s penance gate retired. Read:
> **ADR-0019 / ADR-0020 / ADR-0021**, plan
> `docs/plans/2026-07-10-tauri-migration.md` (M0–M8), panel record
> `docs/reviews/2026-07-10-tauri-rust-migration-panel.md`. §F8 below is VOID.

> ✅ **Doc-sync sweep COMPLETE (2026-07-10).** The Codex-audit drift was reconciled:
> specs 00/01/05 rewritten, 02/03/04/06 bodies synced, ADR amendments recorded,
> changelogs → Unreleased, onboarding docs corrected. Specs are trustworthy again;
> §Known doc drift below is kept as the RECORD of what was fixed (+ the few
> deliberately deferred low-stakes items in §Decisions).

## Governing docs (current truth)

- **`docs/ship-readiness.md`** — the authoritative "what is left before a Windows user can install
  this and it works" inventory (milestone table · ship-blockers [MAC]/[WIN] · [WV] surface · stubs ·
  packaging gaps · icon-bridge R4 status · owner decisions). Living tracker; the detail behind this
  pointer. **Owner decision 2026-07-12: polish everything Mac-closable to near-perfection first,
  Windows is final integration + [WV] runtime pass only.**
- **ADR-0019/0020/0021 + `docs/plans/2026-07-10-tauri-migration.md`** — the
  Tauri 2 + Rust replatform, background-resident v1 (spec 07), arrow default.
- **ADR-0022** + `docs/specs/07-background-resident.md` (updated) — M7 常驻自动 format 的外观模型
  /重置/信任模型/常驻前置定稿; panel `docs/reviews/2026-07-12-m7-resident-panel.md`; build plan
  `docs/plans/2026-07-12-m7-resident.md` (blocked on Wave B, see §Live now).
- `docs/plans/2026-07-12-m6-wire-host.md` — M6-WIRE host wiring (Wave A wallpaper DONE, Wave B
  icons NOT started = the M7 blocker); `docs/reviews/2026-07-12-audit-fix-run.md` — tonight's
  full-repo audit + fix run record.
- **ADR-0023** + `docs/specs/08-calm-windows.md` + plan
  `docs/plans/2026-07-13-calm-windows-module.md` — the **清爽 module** (calm-Windows,
  4th rail tile; owner dispositions 2026-07-13, panel
  `docs/reviews/2026-07-13-calm-windows-panel.md`): honest three-state grammar, guided≠toggle,
  composed admission rule (ad ID + Device Usage out of default), HealthCheck=re-propose,
  capability-gated release (write slice rides v1 iff the Windows cert lab turns green;
  else v1 ships the guided-only face). ADR-0004 §6 timing superseded (amendment in place).
- **ADR-0013** + amendments — v3 "Premium Flat": light-first OKLCH, follows system;
  bundled Inter + HarmonyOS Sans SC; version narrative RESTORED (About version line +
  in-app changelog, auto-opens once per UPDATE, never on first install).
- **spec 02 v3** (+ §Addenda) — visual language; ONE 16px glyph keyline; slash-circle
  无 dialect; selection grammars; dash-free copy. (Has drift — see §Known doc drift.)
- **plan `docs/plans/2026-07-08-v3-premium-flat.md`** — F1-F8 build plan.
- **`docs/references/webview2-pitfalls.md`** — hardening checklist (web-side items
  DONE, host-side items = F8).
- Runbook: `docs/development.md` (browser/mock loop on any OS; native host = F8, NOT
  yet wired — see §Bridge state).

## Bridge state (the P0 reality)

- Web bridge = **schema 6** (`src/bridge/types.ts`, `BRIDGE_SCHEMA_VERSION = 6`). The C# host
  is retired (`legacy/`, ADR-0019) — no schema-1 split to track any more.
- Under Tauri the contract is GENERATED from `dm-contracts` (tauri-specta).
  **`wallpaper.*` verbs now route through real Rust on Mac-Tauri** (M6-WIRE Wave A,
  `docs/plans/2026-07-12-m6-wire-host.md`). **`icons.*` verbs still fall through to the browser
  mock on every platform, including production** — Wave B (icons wiring) has not started; this
  is also the M7 resident blocker (see §Live now).
- Spec 05 (Tauri bridge) reflects schema 6.

## Recently shipped (web side, Mac mock loop → swept to `docs/journal/2026-07.md`)

- **v3 Premium Flat build** (F1 fonts/tokens · canvas-first layout + RIGHT 280px
  inspector, 248px compact · WebView2 hardening + diagnostics/CrashGate · welcome gate ·
  dash purge). Sweep 2026-07-08.
- **Zone editor rebuild** (spec 04, ADR-0014) rounds 1-3 — pixi v8 compositor live+bake,
  five materials + four title styles, import/export, codex+motion review disposed. Sweep 2026-07-08.
- **Icons v2 migration** (spec 06, ADR-0015) — WEB SIDE COMPLETE. CPU TS compositor
  renders every preview + the 256 bake master; bridge schema 3; desktop mirror + taskbar;
  60s arrow gate; Figma corner-smoothing shape engine + curated 11-shape catalog; Gloss
  filter; 极致单色 duotone; dual-tab colour + plateColor; silhouette-aware marks. Sweep 2026-07-09.
- **2026-07-09→10 corrections (post-marathon):** icon-SIZE control REMOVED (panel + canvas
  menu, commit `d708f87`); per-bucket `kindPolicy` (apps/folders/files/system) surfaced as a
  persistent 2×2 labeled-chip section; preview fit toggle 满宽 ⇄ 满高·靠左; canvas-confetti
  celebration (飘丝带 from both screen corners, first-apply-per-launch, shared DRY across
  icons + wallpaper); the first-screen wand+bloom veil/reveal was TRIED then ROLLED BACK
  (broke icons); wallpaper seam/blur polish; zone-list active wash slide FIXED (`b881568`).

## Live now — web, through commit `b881568`

**359 bun tests + `tsc -b` green**; browser visual-acceptance evidence
`docs/plans/evidence/2026-07-icons-v2/` (01-75). The contract truth is
`src/bridge/types.ts` (bridge **schema 4**). Mock desktop = a full
fake desktop (`bridge/mock-desktop.ts` + the REAL pack `public/real-icons/`, the
COMMITTED asset SSoT — ADR-0015 D9 amendment, owner override 2026-07-11; no
synthetic fallback; vite closeBundle strips it from every build). The synthetic
source pack is DELETED — the corpus is all-real (M5.12).

**M0b DONE (recaptured all-real at M5.12)** — parity oracle corpus committed under
`testdata/icons/` (1,611 PNGs): Tier A full desktop under the spectrum default,
Tier B 29-source style matrix (47 cells each), Tier C per-look hue-spread sessions,
per-source stage dumps. Harness `scripts/capture-oracle.ts` (`--capture` /
`--verify [--sample N]`, deterministic); test-suite smoke
`tests/oracle-corpus.test.ts` (no CI pipeline exists yet — all gates are local).
This is the TS side of the M5 tri-target differential.

**M1 Spike 4 DONE (tri-target pixel slice — the one M1 spike that runs on Mac)** —
Circle + white plate + subject blit + dock shadow over 120 sources × {256,512} = 240
cells: **native↔wasm 240/240 byte-identical, TS↔Rust 0 diff bytes of 157.3M**
(byte-equal; gate was SSIM≥0.995; re-run at HEAD over the real pack:
248/248 native↔wasm, TS↔Rust 0/162,529,280). `crates/dm-icon-core` slice modules + plain-ABI
`dm-icon-wasm` + `xtask spike4-*`; one command `bun tests/icon-parity/spike4/run.ts`.
Determinism intel (M5 checklist seed): JSC pow vs libm pow = 1 ulp apart on 34/256
decode-LUT entries → TS↔Rust byte parity is EMPIRICAL (full-corpus differential remains
the certification gate); wasm↔native parity is structural. Details in the migration
plan §M1. Remaining M1 spikes (1/2/3/5) are Windows-bound.

**Replatform progress (Mac-first, 2026-07-10→11 — details live in the migration
plan's DONE blocks, this is the pointer):**
- **Layout restructured (ADR-0019 Amendment 1, owner order):** community-standard
  Tauri — web app at the repo ROOT (`src/`, `public/`, root `package.json`),
  `src-tauri/` + `crates/` at root, the ENTIRE frozen .NET tree quarantined under
  `legacy/`, `apps/` deleted, `.gitignore` .NET globs scoped (Rust `src/bin` no
  longer swallowed). Six-commit chunk `1aadc6e..fd2ff77`.
- **M2 DONE:** Tauri 2 window hosts the app on Mac; tauri-specta generated
  bindings (`src/bridge/generated.ts`); rusqlite settings store; 359 bun tests.
- **M3/M4 blind-write DONE:** dm-domain/dm-operations/dm-windows/dm-elevated —
  durable WAL transaction + incremental CAS ledger fully unit-tested on Mac
  (non-icon crates 59→125 tests after the coverage audit, incl. kill-point
  battery); COM adapters msvc-check clean in isolation; runtime verification =
  [WINDOWS-VERIFY] checklist in `docs/plans/2026-07-10-m34-windows-blind.md`,
  batched with M1 spikes.
- **M5 + M5.11 + M5.12 DONE, CERTIFIED over the all-real corpus:** full TS pixel
  pipeline in `dm-icon-core` + `dm-icon-codec` ICO writer. Real-icon SSoT
  `public/real-icons/` committed (ADR-0015 D9 amendment, owner override — never
  ships: vite closeBundle strips it; taskbar/wallpaper fixtures repointed).
  One-command cert `bun tests/icon-parity/m5/run.ts`: **1487/1487 corpus cells
  byte-identical (0/389,808,128 diff bytes)** over 124 real sources, profiles
  124/124, masks 48/48, hue-spread 7/7; setHash `8a6c19ee69235d95`; pack 100%
  PNG, parity gate decodes in-process on any platform. bun-only sweep done
  (`node:zlib` kept deliberately — Bun's native zlib; `Bun.*` variants are
  raw-deflate and cannot read/write PNG streams).
- **M6 single-truth flip — ✅ EXECUTED (2026-07-11 night, swept to journal).** wave-2 hardening
  CLOSED, then the M6 preview cutover reached WASM-vs-TS perf parity via the kernel-speed line
  (Phase 0 cert-hardening → Phase 1 mask cache → Phase 2 source-fact cache → Phase 3 native rayon
  → Phase 4a content-addressed output cache; `docs/plans/2026-07-11-m6-kernel-speed.md`); the
  owner then approved the flip ("不保留 ts，冻结 ts，做完没问题就移除") — WASM is now the ONLY
  preview+bake+background pixel path (frozen TS tree-shaken out of the bundle, physical deletion
  held). Arrow-restore UX shipped the same window (`docs/reviews/2026-07-11-arrow-restore-panel.md`).
  Full detail: `docs/journal/2026-07.md`.
- **M6-WIRE Wave A (wallpaper) — ✅ DONE (2026-07-12, `docs/plans/2026-07-12-m6-wire-host.md`
  A1-A7).** Per the owner's D1 ruling (Rust wallpaper = thin platform I/O only; rendering/
  reconcile/state-assembly stay frontend), wallpaper verbs now route through real Rust on
  Mac-Tauri — bridge **schema 6** (see §Bridge state). **Wave B (icons wiring) has NOT started —
  this is the hard M7 blocker.**
- **Tonight's full-repo audit + fix run — ✅ 11 commits landed, ALL GATES GREEN (2026-07-12,
  `7f7e5c2..HEAD`):** `cargo test` workspace · `cargo check --target x86_64-pc-windows-msvc` ·
  `tsc -b` · `bun test` 516. 2 P1 (WebView2 bridge transport hang on real Windows Tauri;
  `secure_dir` SDDL owner missing → permanent apply failure after the FIRST apply) + P2 hardening
  + dead-code removal + a self-caught test regression. Full findings ledger + verification notes +
  held/deferred items + the blake3/msvc note + the codex-background-death root cause:
  `docs/reviews/2026-07-12-audit-fix-run.md`. Archived: `docs/journal/2026-07.md`.
- **#7 + 3 P3s — ✅ DONE (owner 这些都做 2026-07-13, `c8ded4a`):** `diagnostics.getInfo` is a real
  Rust command (SystemInfoDto), ELEV-3 (CommandLineToArgvW quoting) / APPLY-3 (non-string DefaultIcon
  type fails closed) / CORE-1 (`WaitForSingleObject` result checked) all hardened. Dispositions +
  evidence: `docs/ship-readiness.md` §Open owner decisions.
- **ICON-5/9/11 — ✅ RESOLVED (owner 这些都做 2026-07-13, `5ac018a`):** ICON-5
  (`clamp_u8_round_half_even`) DELETED (Rust-only orphan, no oracle mirror, lying doc). ICON-9 (mono
  tail) + ICON-11 (5 `color.rs` fns) RETAINED + documented — each is a 1:1 port of a NAMED
  frozen-oracle export, dead in the oracle too; deleting the Rust halves would diverge the certified
  byte-parity port (ADR-0019). `SHELL-2 read_target()` remains confirmed NOT dead code.

## M7 常驻自动 format — design FINALIZED, build BLOCKED on Wave B

Owner-approved (2026-07-12) three-seat panel (chief-PM/chief-UX/chief-架构师, two rounds) + four
owner dispositions: `docs/reviews/2026-07-12-m7-resident-panel.md`. Governing docs: ADR
`docs/decisions/0022-m7-appearance-model-and-consent.md`, spec `docs/specs/07-background-
resident.md` (updated with the full model), build plan `docs/plans/2026-07-12-m7-resident.md`.
Core model: `version` = an appearance PRESET (3 orthogonal stores — ① active ledger ② saved-style
single-truth ③ look-history 10-cap), NOT a desktop snapshot; reset = trust-first (skip items the
user hand-edited since, never silently clobber; three-way coupled with clearing saved-style +
disabling auto-format + restoring the arrow overlay); auto-format trust model = batched propose +
timeout-auto-apply with native Toast (never silent by default); resident enable requires one
successful global Apply first.

**Wave B foundation BUILT + HARDENED (2026-07-12 night — pure-Rust storage/identity layer, Mac-green).**
Landed on `main` after `70fddf0`: **B6** saved-style column (② `SettingsStore.icon_style_json`,
`9f4a3c3`), **B7** `FsAssetStore` (the first real `dm_domain::AssetStore`, `97d4bf0`), **B8**
`LookHistoryStore` (③ `look-history.json`, `4af1e05`), **B9** `SourceIdentity` source fingerprint
(`517417d`), plus a shared `fs_atomic` crash-atomic-write extraction (`7feae9d`) + a validated shared
`dm_contracts::IconStyle`.
**Two adversarial codex rounds (`/multi-ai solo`, each "Request Changes") → 5 fix commits** hardened
it: symlink-temp truncation (create_new O_EXCL + fsync parent) across fs_atomic/durable.rs/overlay.rs;
completed the atomic-write DRY (journal + baked-wallpaper gained fsync); FsAssetStore trust
(content-verify on reuse, symlink-root refusal, case-insensitive `-empty`, length-first compare);
LookHistoryStore anti-clobber (strict mutation load) + pin-cap normalization; typed IconStyle
(rejects null/garbage/per-icon); SourceFingerprint newtype + AUMID; a real overlay-snapshot
durability barrier before the HKLM write (`6aec58d f2437f7 340231f 3e51d41 00a25b6 de3db16`).
`cargo test --workspace` **457 green**; dm-domain + dm-windows msvc-clean.
**3rd codex CERT pass: Mac foundation CERTIFIED (findings 1-4 RESOLVED); 4 remaining defects were all
Windows-platform durability/atomicity — owner ruled blind-fix-now, all DONE** (`b7a31db` dm-windows:
COM `.lnk` temp `claim_temp_for` O_EXCL + `MoveFileExW(WRITE_THROUGH)` publish, **msvc-verified**;
`a87dff8` dm-elevated: write-through overlay snapshot + non-replacing snapshot-once claim, **blind-write
[WINDOWS-VERIFY]** — blake3 blocks its msvc-check). Precise repros + the (deferred) IPersistStream /
recovery-re-verify / named-mutex follow-ups are in the handoff plan **§8a**. The B2 apply/GC
lifecycle-lock is recorded as a B2 acceptance criterion (its RUNTIME half — serializing apply+gc under
one mutex — lands with the host, B4).

**B1+B2 DONE (2026-07-12, D1-consistent THIN boundary — `961cc7f` contracts, `1ef2d23` operations).**
Owner ruling applied: Rust = scan/apply-package/restore/②③-persist; the frontend assembles
IconsStateDto (presets/palette/grid). Locked design decisions (carry forward to B3-B5):
- **B1** `dm-contracts/icons.rs` — the thin bridge shrinks like wallpaper's (schema 6→**7**, TS side lands
  in B5): `scan→IconScanDto{revision,items}` (raw items, NO state), `getState→getPersisted→IconPersistedDto`
  (② saved-style + ③ history + applied + arrow + profiles), mutating verbs → thin `IconOpResultDto{ok,toast,persisted}`.
- **`icons.setLook` LEAVES the bridge** (frontend draft) — a config/override/kindPolicy/typeOverrides draft
  is not intent (spec 07 §8.2: ② written only on a completed global Apply), so it lives in the frontend store
  like wallpaper's setLook. **Per-icon overrides are frontend draft state** (a `tint` bakes its master
  frontend-side), BUT a 「保留原样」 of an already-styled icon must REVERT it, so `commit` carries
  `restoreIds` and Rust CAS-reverts the tracked subset (codex Block 4 — not sending a master ≠ restoring).
- **IconStyle rides as an opaque validated JSON STRING** (`styleJson`/`savedStyleJson`) — like the baked
  wallpaper PNG rides as base64; Rust validates the envelope, never types the recipe internals, keeping the
  generated bindings free of the recipe shape.
- **sourceIndex convention: 0 = primary, 1 = paired empty** (Recycle Bin), matching `sourceUrls[0]=primary`.
- **B2** `dm-operations/src/icons/` — `IconApplySession` (chunk buffer) + `IconOps` (bundles the immutable
  ports + settings ②; methods take the mutable stores): `commit_apply` (reconciles journal→ledger before
  prepare = the #5 gap; packages masters via dm-icon-codec; drives TxnDriver.apply; persists ②③; GCs with
  live = ledger ∪ in-flight journal), `reset_to_original` (CAS-gated trust-first per spec 07 §10 ★; clears ②),
  `read_state`. `txn::fakes` is now pub(crate) for test reuse. **Documented follow-up:** reset's crash-window
  (crash between applier.restore and ledger.remove leaves a lingering row; desktop never wrong, self-healing)
  is NOT journaled yet.

**B3-B5 DONE (2026-07-12 — `3a1e2ee` host+devhost, `a700bdf` web).** The icon bridge is wired end to end:
- **B3** `dm-domain` NEW `IconSourceExtractor` port + `src-tauri/devhost_icons.rs` (synthesizes distinct 256px
  stand-in sources + a shared virtual icon desktop backing reader/applier, so the real scan→package→apply→
  restore pipeline runs on Mac with zero setup) + `dm-windows/source.rs` `WindowsIconSourceExtractor` [WV] stub
  (real IShellItemImageFactory extraction = Windows batch; the type + composition are wired so the swap is one
  method body).
- **B4** `src-tauri/icon_host.rs` `IconHost` (mirrors WallpaperHost): ports + FsAssetStore + ②③ stores + the
  chunk-buffer session + last-scan cache ALL under one mutex (the B2 apply/GC lifecycle-lock's runtime half);
  serves sources over the `dmicon://<id>/<slot>?rev=N` custom protocol (mirrors dmwallpaper://). 8 `#[specta]`
  commands + AppState.icons + cfg-selected composition (dm-windows [WV] on Windows, devhost on Mac) + settings
  shared via Arc (one ② writer). generated.ts regenerated (schema 7).
- **B5** `src/lib/icons-assemble.ts` (NEW, the single frontend assembly path both bridge backends feed:
  presets/palette/swatches/grid/`activePresetIdOf`/`assembleIconsState`, moved out of `mock-desktop`) +
  `types.ts` schema 6→7 (IconScanDto/IconPersistedDto/IconOpResultDto/IconChunkItemDto; getState→getPersisted;
  **setLook LEFT the bridge** = frontend draft, resumed from ② on relaunch) + `stores/icons.ts` (scan fetches
  scan+getPersisted then assembles; apply commits a styleJson + reassembles from persisted; op results carry
  `persisted`) + `tauri.ts` (8 verbs → real commands) + thin `mock-desktop.ts`.
- **Gates green:** cargo test --workspace **492** · dm-domain+dm-windows msvc-clean · `tsc -b` · bun **516** ·
  vite build · generated.ts drift-guard · full `deskmakeover-desktop` app build.
- **[WV] follow-ups (Windows batch):** WindowsIconSourceExtractor body · the cfg(windows) icon-host composition
  (blind-wired, unverified — overlay-helper path, exec sharing, real IFolderView2 positions) · reset's
  crash-window journaling. **LIVE supervised icon-apply on Mac-Tauri is the owner gate (never auto-run).**

**Codex adversarial review — round 1 Request Changes → ALL FIXED (2026-07-12, `4c3a25c`+`2a12f53`).**
7 Block + 6 Major, each二次核实'd real then fixed: reset journal-revival (reconcile+strict-checkpoint before
delete), fresh-apply CAS uses the scan-TIME fingerprint (ScannedItem) + session revision/count validation,
a superseded apply rejects via the host look-epoch (no stale desktop write), 「保留原样」 reverts via
restoreIds, arrow overlay real-ICO-path + persisted marker, dmicon CSP allow-list, ②③ guarded on
apply.error, reset §10 (keepNewIconsStyled=false + skipped surfaced), ③ replay carries kindPolicy + isCurrent
via ②, rescan preserves overrides + setOverride marks dirty, dmicon content-addressed URLs, scan reports
observed GridMetricsDto (no fabricated dims), export honest, package.rs fail-closed (PNG/256²/size-cap).
Gates: workspace 497 · msvc-clean · tsc · bun 516 · drift-guard green. **Round-2 codex verification dispatched.**
Documented follow-ups: the ①→②→③ finalize crash-window (not yet journaled) + the reset crash-window.

**Still unbuilt in Wave B — only B10:** `crates/dm-windows/src/watcher.rs` (M7 SKELETON `Err` stub →
`notify`+`notify-debouncer-full`, spec 07 §3/§16). **Roadmap: B10 → M7.**

**In flight / next (web):**
-2. **清爽 module Wave 0 + polish + viz DONE (2026-07-13, ADR-0023 / spec 08 / plan
   2026-07-13-calm-windows-module.md):** web skeleton (catalog+state machine, CalmBackend
   port+mock, store, Direction-B page, rail 4th tile, i18n+copy gate) → codex R1 (2🔴+15🟠,
   ALL fixed) → owner complaint 「不知道关的是哪里」 → 3-seat polish panel → WHERE-axis
   redesign + codex R2 closes (lost-reply=unknown+reprobe, never unproven 已还原) →
   **W0.6-viz (owner escalation ×3):** per-row 104×64 schematics drawn from REAL Win11
   screenshots (panel ruling O4 `docs/reviews/2026-07-13-calm-schematic-panel.md`),
   NoiseGroup/ReflowGroup/ShrinkRect honest-motion (no hollow sockets), no done-ghost, no
   hero image, per-row 「恢复」, task-view glyph per owner pixels, shared `FullPage` shell
   (清爽/设置 title parity) → top-UX acceptance FAIL→`9e17d26`→re-verify PASS.
   **W0.6-viz-r2 (owner: text-wall + taskbar must re-centre):** type ladder 18>16>14>12>11,
   cluster axis x54.5 (designer re-acceptance PASS 1.01:1/1.00:1) + **codex R3→R7 loop ALL
   FIXED → R8 = APPROVE**: 'reopened' is a REAL CalmRowState (spec §6 drift notice
   「重新关闭」 scoped re-apply; RESTORABLE_LEDGER keeps Restore alive when every write
   drifts; ONE actionable set feeds notice/button/hero), quiet ghost frame dead,
   reduced-motion sweep, walk-token races, hero synced wears coral (§10 — the 双成功色
   owner call RESOLVED by spec), tray copy↔picture, celebration = ONE confetti per launch
   across ALL modules (spec 02 §Ceremony; per-module keys were drift — flagged to owner,
   one-line revert if per-module was intended), applyAll returns THIS call's summary.
   Spec 08 §2.1 carries the schematic contract + type ladder + taskbar axis. Gates: tsc ·
   bun 597 · browser E2E + `.tmp-calm5` acceptance set. Owner calls open: O1-O3.
   **W1 DONE (2026-07-14): Rust decision core + bridge schema 8, wired end to end on Mac.**
   New file family `crates/dm-{domain,operations}/src/system_tweaks/` + `dm-contracts/tweaks.rs`
   + `src-tauri/tweaks_host.rs` + `src/bridge/tauri-calm.ts`. TweakDriver (inspect/apply/restore/
   recover) rides its OWN WAL JournalStore (unforgeable WriterLease + generation guards; the icon
   txn spine is ItemId/fingerprint-keyed, incompatible). Fail-closed capability manifest (0 writes
   until a Windows VM cert run); catalog ids match calm catalog.ts verbatim (bridge zero-translation).
   **codex R1→R5 adversarial loop, ~15 real bugs fixed** (false-verified, clobbering rollback,
   generation-guard bypass, missing receipt, missing-key, effect-proof skip, no pre-write re-auth,
   policy-managed-via-Undo, restore-race block, inspect ownership misreport) — commits
   224e3e0…a99b916/30a32d0 — **codex R1→R7 loop, R7 = APPROVE (16 real bugs fixed, converged).**
   W1 scope (honest): value-level over pre-existing keys, DWORD-only, in-memory journal, devhost
   fakes on all platforms. Gates: cargo 244 · dm-domain msvc-clean · clippy · tsc · bun 598 ·
   check:bindings · files ≤500. **W1 CLOSED.**
   **W2 DONE (2026-07-14, `005ff6e`): the two Windows platform ports BUILT (Mac blind-write, msvc-clean).**
   `crates/dm-windows/src/system_tweaks/` — `WinregBackend` (`RegistryBackend`) +
   `WindowsSystemProfileProbe` (`SystemProfileProbe`). Pure host-tested core (`translate`/`profile_facts`,
   22 Mac tests) + thin `cfg(windows)` FFI shell (`backend`/`profile`, [WV]), so every DECISION is
   Mac-verified. Raw `windows-rs` registry FFI (not `winreg` — it errors on types > `REG_QWORD`, losing
   `Other(raw)`); `is_policy_managed` = `KEY_SET_VALUE` open-probe + catalog `policy_guards`; profile from
   registry `CurrentVersion` + `GetProductInfo` (no `Wdk`/new features). Scope: only the two ports — real
   verifier + durable journal are Wave 3, so `TweaksHost` stays devhost.
   **codex adversarial loop R1→R5, R5 = APPROVE (2026-07-14) — W2 CLOSED.** 8 real defects fixed across
   5 rounds: ports (false `packaged` fact, over-claimed `is_policy_managed` contract, derived/hardcoded
   env signals, malformed-UBR/lossy-UTF16 certification holes) + the W1 ENGINE (my honesty doc exposed
   that rollback/restore/recovery bypassed the authoritative catalog `policy_guards` — now the guard
   addresses are PERSISTED with each transaction/anchor so the reverse paths are drift-immune; and the
   apply read-back no longer hides a committed write behind a bare error). 7 regression tests lock the
   contracts (guard-before-restore/rollback/recovery, foreground+recovery catalog-drift immunity,
   migrated-recipe restore no-deadlock, committed-write-then-read-error → Reverted). Final gates:
   dm-operations **253** · dm-windows host **90** · msvc cross-check clean · msvc+host clippy clean · ≤500
   (all in a clean git worktree — main tree's dm-icon-core is mid-refactor by a concurrent session; my
   commits stayed literal-pathspec isolated). NEXT: Wave 3 cert lab = the ADR-0023 D2 gate (real box).
-1b. **PRESET COLLECTION v2 SHIPPED + ACCEPTED** (`b7dd226`+`f8eb20d`, all
   designer PASS): seven coordinate-bookmark presets (spectrum default ·
   stationery · glass · pebble · ink · white · ascast), six mark styles
   (Fold retired), #65470D dark-brown folder boards banned, featured-four
   fold + 「更多风格 +N」 counting chip, glass preset wears Shadow (Glass
   bead mark redesign = COMPONENT DEBT). Preset hover carries typeOverrides
   (`5d3b589` bug fix); type-row picker overflow fixed (`f03abe0`).
   Normative: docs/product/preset-collection-v2.md. OWNER-PENDING: which
   sets enter factory lineup + default confirmation (all seven live now,
   spectrum default). Owed unchanged: colour-migrate tests · banned-arc
   guard · fixed-plate shadow regression test · folder-drift probe (likely
   obsolete — folders now derive/manila by design) · spec02/06 amendments ·
   alpha-plate exploration (owner asked; verdict: plumbing not algorithm).
-1a. **TWO-AXIS RESHAPE DESIGNER-ACCEPTED (final PASS at `46a26bc`)** — subject×plate + 本色 fifth stop + panel two rows + accordion parity all certified (§6 full pass, four-way glyph distinguishability, violet-arc rulings executed). Paper file band PASS at `c080912` (owner A). NEW OWED: ① regression
   assertion - fixed-plate tiles MUST carry the silhouette shadow (T2 bug
   relapsed once, designer strong-rec); ② INVESTIGATE folder band colour
   drift - designer saw multicolour folders in paper-band-v1-zoom despite
   the #65470D factory pin (suspect: the widened user-plate lane's
   backdrop-swap path); ③ owner Q: folders 统一金 vs 各自色 (designer says
   multicolour aids within-folder findability; owner earlier picked 统一金).
   Owed T4 tail: colour-migrate mapping tests · banned-arc swatch guard in banned-colors.test · spec02/06 colour-axes amendments · OWNER-PENDING: 灰暗文件板 A(暖纸色板,推荐)/B(深板变暖)/C(不改) disposition. Was: T1/T2 `26aa3db` · T3 `e4bac52` (Subject+Plate rows live, dual-tab popover dead, accordion pins added; designer §6 acceptance IN FLIGHT). Owed T4: colour-migrate mapping tests · shape=None disabled-state screenshot · spec02/06 colour-axes amendments. Old note: **T1/T2** — model+engine+bridges live on
   subject×plate (smoke: 124 icons byte-identical look, presets match, 0
   errors). Spec FINAL (本色 = plate stop (null,'white'), faithful/minimal
   collapse fixed). NEXT: T3 panel two-row rebuild (主体行+底板行 + 本色
   chip + QuadPlateGlyph + kill dual-tab popover + accordion plate 本色 +
   i18n Subject_/Plate_ keys) + T4 (migration-table tests, screenshots,
   designer §6 four-way distinguishability acceptance) per
   docs/plans/2026-07-10-two-axis-colour.md.
-1. **TWO-AXIS COLOUR RESHAPE — OWNER-APPROVED (2026-07-10), spec being
   written to `docs/product/two-axis-colour-spec.md` by the chief-designer
   seat.** Panel 3/3 convergence: dissolve `colorMode` into 主体 (subject:
   原彩/黑白/单色) × 底板 (plate: 随图标-first/白/bounded swatches/wheel);
   满彩 demotes from mode to the default preset coordinate; presets become
   coordinate bookmarks; the word "mode" leaves the UI; plateColor's
   per-mode semantics collapse to one plate value; Original's
   plate-recolour gap dies structurally; net-new combos (BW×Auto etc.).
   Deterministic old→new mapping = zero feature loss; schema 3→4 BEFORE F8
   (translation layer VETOED by the UI seat). Naming law: 主体/底板, never
   前景色/背景色. Guardrail: plate axis leads with 随图标 (anti "all apps
   one colour"). Type rows keep the just-shipped chip grammar; per-type
   rule becomes "types may only step DOWN".
0. **ADR-0017 per-type distinction system — v1 BUILT (commit `7ba12cf`),
   designer acceptance of the factory ladder in flight.** Three-round
   adversarial panel (PM/UX/UI) + owner dispositions; docs: ADR-0017,
   spec 06 §6.5, plan 2026-07-10-type-distinction-system.md. Shipped: sparse
   `typeOverrides` + `resolveTypeConfig` chain (preview/styleKey/bake all
   consume resolved configs), kindShapes DELETED, `shortcutShape` (uniform
   shortcut shape, default off), ExecutableFile→App bucket (bare exe =
   program), AppxShortcut isShortcut bug fix, hue-spread pool filter
   (fixed-plate types exit), type ACCORDION panel (expand-to-edit
   shape/saliency/bounded-plate + canvas scope-highlight dimming), factory
   saliency ladder (App Apple+Field / Folder Bookmark / File Tile / System
   Circle+BlackWhite). 355 tests + tsc green. OWNER-PENDING: shortcut mark
   default (panel consensus = badge ON; owner decree 2026-07-07 = presets
   ship None — unresolved conflict, decide before F8). F8 additions: host
   `.exe` classification + Appx mark fix + typeOverrides in Contracts.cs.
1. **ADR-0016 icon colour-field default** — COMPLETE to the owner's FIVE-STEP
   LAW + the RIM BAND law (spec 02 §Default Composition, ADR-0016 Amendments
   2-3; `IconProfile` metadata layer in profile.ts). Derived plates take the
   artwork's outermost solid BAND (α≥245, ~minDim/16 deep, majority hue via
   dominantColor): 亮圈深底 / 暗圈浅底 / 黄圈黄底; law-② boards additionally
   pass a corner-symmetry gate (dog-eared pages rejected); deep boards keep
   fitted chroma ≥0.09 where the gamut allows (yellow-green zone pulls to
   amber h≈78 — 深金, never olive). Designer verdicts: v7 PASS · v8.1 PASS ·
   v17 PASS · **v19 FAIL → v20 PASS (4/4 items cleared, 11 own-boards
   pixel-identical, zero collateral)**. ⛔ Iron laws: subject pixels never
   recoloured; own backgrounds never altered. T7 glass rim SHIPPED. T9 codex
   review DONE (11/11). 346 bun tests + tsc green (rim regression trio:
   light-outline / accent-vs-majority / soft-shadow). OWNER-PENDING: info-class
   solid circle badges legally anchor their own colour (law ②) — designer
   suggests optional ring-seam lightness polish, owner call. Owed: D4 corpus
   ΔE probe (browser harness) · resx sweep of new PENDING-RESX. Plan:
   `docs/plans/2026-07-10-icon-colour-field.md`.
2. Zone rebuild polish tail: equal-gap ticks (deferred, NOT accepted), rename-input polish,
   SwiftShader/`MAX_TEXTURE_SIZE` startup probe with reduced-res fallback, TS bake fixtures.
3. Dark theme + zh locale full regression screenshots → `docs/plans/evidence/2026-07-v3/`.
4. F7: cross-vendor adversarial review (codex via /multi-ai) over the full diff.

## Known doc drift (Codex audit 2026-07-10 — RESOLVED; kept as the record)

Facts verified against HEAD. Every item below has been corrected in the named docs
(commits `6ec1ffc` / `f0542ff` / `9453656` / `80dbaf4`); this list stays as the map of
what changed and why.

- **Specs 01 & 05 — most misleading.** They describe C# producing pixels (SharedBuffer
  frame stream, WebView2-as-viewer, left 300px panel). CURRENT: icons rendered by CPU
  TypeScript + Worker; wallpaper by Pixi; C# keeps only window / source decode / ICO
  packaging / shell write / backup-restore. Pending full rewrite to schema 3 + current arch.
- **Spec 00** — the v1.0-icons-only / v1.1-rail release train is void; prototype-parity is
  no longer a release gate; icon-size was reverted; move to `Unreleased → first public → next`.
- **Spec 02** — 300px panel → RIGHT 280/248px; shape catalog is 11 (not 13); Apple corner
  math has 3 conflicting sources (see decision below); font budget line stale; first-scan
  original→beautified wow was rolled back; module switch is instant/keep-mounted.
- **Spec 03** — settings page drifted: grouped card (not per-group full-width), version +
  changelog RESTORED (not removed), trust facts are dotted text (not pills).
- **Spec 04** — opens citing round-2 but the body still says single-Frost/single-chip/no
  baked shadow; references `paper-presets.tsx` which does NOT exist; bake is main-thread
  Pixi `toBlob`, not an OffscreenCanvas worker.
- **Spec 06** — Pixi vs CPU-renderer self-contradiction (it is CPU TS + Worker; Pixi is
  wallpaper only); schema v2 → 3; icon-size + canvas-size menu removed; taskbar running
  pills removed; mock pack is PNG (not WebP); auto-beautify default conflicts with settings.
- **ADR-0014 line 22** — says `WallpaperBakeRenderer.cs` / `WallpaperComposer.cs` are
  DELETED; they still exist (F8 deletion). **ADR-0015 line 113** — says the IconStyler chain
  is deleted; `IconStyler.cs` + tests still exist (F8 deletion). Corrected to pending.
- **ADR status table** — many are partially superseded (0003/0005 default-mark governance
  reversed → default is None; 0008/0012 dark-default + no-version reversed; 0011 renderer
  ownership superseded by 0014/0015; 0014 material decision reversed to five materials).
  Full table lives in the Codex audit; amendments to be added, history NOT rewritten.

## Decisions (owner-resolved 2026-07-10 — drive the spec rewrites)

1. **`win-native-arrow.png`** — RESOLVED: leave it. The owner accepts the git-tracked
   extracted arrow as-is (do NOT redraw, do NOT re-flag as a release blocker). ADR-0015's
   no-ship-extracted-assets clause carries an owner exception for this one asset.
2. **Apple corner geometry** — the TS renderer's iOS-0.225 cubic (`shapes.ts`) is CANONICAL
   (it is the WYSIWYG bake truth). chip-preview must drop its Lamé n=5 and share the same
   cubic path; Spec 02 + old ADR updated to cubic. C# oracle already cubic. [code + doc]
3. **`ConfigDto.size` / `TrySetIconSize`** — `size` becomes a READ-ONLY observed field;
   guard the writer so history/version replay can never resize the real desktop. [C# guard = F8]
4. **Wallpaper gesture** — KEEP HEAD (blank-left-drag creates a zone; pan on middle/compare);
   add a reversal amendment to ADR-0013. [doc]
5. **New-icon auto-beautify** — default FALSE and HIDE the setting until a real watcher/
   consumer exists (no promising an absent capability); Spec 06 updated. [code + doc]
6. **Ordinary-file participation** — KEEP default-on (product is reversible + supervised;
   kindPolicy gives one-click per-bucket opt-out); update Spec 01 to match. [doc]
7. **Space key** — REVISED 2026-07-10 (owner challenge): the "focused button gets Space"
   a11y clause does NOT apply to this product. Space stays a GLOBAL compare gesture (only
   text inputs excluded). Reason: the inspector is button-dense and a just-clicked swatch/chip
   keeps focus — letting Space activate it would break the compare gesture exactly when it is
   used. Buttons remain keyboard-activatable via ENTER, so nothing is stranded. No code
   behaviour change; amend Spec 02's generic Space-activates-button clause to record this. [doc]
8. **Release identity** — standardize ALL changelogs to `Unreleased` until the owner names
   the first number (root CHANGELOG, Host changelog.json, mock). [doc]

**doc-sync part 2: COMPLETE 2026-07-10.** Code 2/5/7 `3a6ec48` (3 size-guard = F8) ·
ADR amendments `9453656` · changelogs → Unreleased `b1890fa` (Host json feature-copy
still needs owner curation at release) · Specs 00/01/05 rewritten `6ec1ffc` ·
Specs 02/03/04/06 bodies synced `f0542ff` · code-style.md two-stack rewrite.
Still deferred (low-stakes): per-ADR Superseded status banners (map above suffices),
historical banners on old plans/reviews/evidence, HarmonyOS font subsetting task,
webview2-pitfalls SharedBuffer-era scope note.

## F8 — VOID (superseded 2026-07-10 by the ADR-0019 replatform)

The C#-host reconciliation list that lived here is dead work: the host is never
wired; the Tauri/Rust migration plan (`docs/plans/2026-07-10-tauri-migration.md`)
replaces it. Items that CARRY FORWARD into the plan (not lost, re-homed):
wallpaper get-source/apply (M4) · parity fixtures → the TS-oracle corpus +
tri-target harness (M0/M5) · host error capture → Rust tracing/minidumps (M2) ·
fonts attribution line in About (M8) · packaging made real → NSIS + helper (M8) ·
**findability gate (ADR-0016 D4)** at release exit (M8) · i18n: the resx sweep is
CANCELLED — TS dictionaries become the source of truth (ADR-0019 defaults).

⚠️ **Owner-only gates unchanged**: supervised LIVE icon-bake + wallpaper-apply +
resident-mode audit (never auto-triggered) —
`docs/verification/owner-supervised-live-runs.md` (rewrite for the Tauri stack at M8).

## Owner rules (durable)

- Accent = warm coral `#FF6F5E` only; blue/violet permanently banned (grep+test gated).
  Reviewed exemptions in `tests/banned-colors.test.ts`: OS-authentic depictions (Windows
  arrow blue `#0067C0`, taskbar chips) AND the multicolour celebration confetti (one file).
- **Light-first, theme follows system** (ADR-0013 D3; supersedes old dark-default).
- **Version narrative RESTORED** (ADR-0013 amendment; supersedes ADR-0012).
- **No dashes in user-facing copy** (owner decree; reads as AI text).
- Every axis's 「无」 sits FIRST wearing slash-circle; dashed = auto, slash = none. ONE
  keyline for all axis glyphs (25px canvas = 20px ink).
- **Control scale unified app-wide**: segmented `sm` (22px/11px), chip buttons 11px on every
  page. Page-scale adjustments touch the TEXT layer only, never inflate controls.
- Presets never carry a shortcut mark; nothing arrow-shaped near preset thumbnails.
- **Arrow semantics (ADR-0021, 2026-07-10)**: the global transparent overlay is the
  DEFAULT; every shortcut is redrawn; 「保留原样」 = subject + baked classic arrow;
  the 60s penance gate RETIRED (its object no longer exists). The rest of the
  welcome-gate ritual is untouched owner brand ceremony — do not soften it.
- ⛔ **Icon subject pixels are never recoloured** (ADR-0016 D8, owner 2026-07-10):
  every icon keeps its own colours; looks differentiate via plates, silhouette
  shadows/halos, outlines, backgrounds — never by re-inking some subjects.
- **Visual work acceptance loop** (owner order 2026-07-10): a look/effect is done only
  when the designer-seat subagent passes a pixel-level acceptance on REAL renders;
  FAIL → iterate and resubmit.
- Extreme DRY; files ≤500 lines; WYSIWYG (preview == bake pixels); bake/apply owner-supervised.
- Specs are the intended source of truth — but see §Known doc drift; the old prototype HTML is historical only.

## Blockers

- None for web/core development (M0/M2/M5 run on Mac). M1 spikes + M3/M4/M8 need the Windows
  box (SSH/Tailscale, logged-in interactive session). **M7 is additionally blocked on M6-WIRE
  Wave B** (Mac-buildable, not a Windows-box blocker — see §Live now). Release gates: signing
  cert (owner), public repo visibility (owner), first version number (owner).
