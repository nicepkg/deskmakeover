# Rust Full-Audit — Consolidated Ledger & Dispositions (2026-07-13)

> **What this is.** Disposition ledger for the 12-slice carpet codex review of the entire Rust
> workspace + Rust↔frontend bridge (calm/清爽 module excluded — concurrent session). Raw verbatim
> findings: sibling `2026-07-13-rust-full-audit-raw.md` (~293 findings). This file is the analysis:
> cross-cutting families, per-item disposition, the fix queue, and the owner-decision list.
>
> **Discipline.** Every finding I *fix* gets 二次核实 (read the source, confirm real) immediately
> before the edit — codex findings are claims, not truth. Items marked WIN/OWNER/PERF/RETAIN are
> recorded, not fixed; their categorization is cross-checked against `ship-readiness.md` (§8a
> durability defects, §[WV] surface) which already tracks most Windows-runtime items.

## Progress log

- **2026-07-13 — B1 §14 fail-closed (audit F2) — DONE + codex-reviewed.** `ScopeRoots` enum
  (`Unprivileged` | `Resolved(ResolvedRoots)` | `Unresolved`) replaces the fail-open two-slice API;
  the Windows host passes `Unresolved` (fail closed until known-folder resolution [WV]). Commits
  `f21a4d1` + the review-fix follow-up. **codex B1 re-review: 🔴 + 🟠 found and FIXED** — the public
  `Resolved{..}` variant was still constructible fail-open (now a private `ResolvedRoots` inner struct
  + `resolved()` rejects roots that normalize to nothing); the reconciler flooded the pending queue on
  `Unresolved` (now defers the whole cycle via `is_resolved()`). codex also **confirmed a BROADER gap
  the fix did not close → tracked as F2b below.**

### F2b — the scope gate does NOT dominate EVERY mutation (reset + recovery) — codex B1-🔴
`reset_to_original` (`icons/mod.rs:524`→`applier.restore` ~600) and crash `recovery`
(`txn/recovery.rs:277`, called at startup `lib.rs:258` + `version_switch:78` + `reconciler:143`)
restore/remove ledger+journal targets with NO privileged-scope check. So even with B1, a privileged
item that entered the ledger (old fail-open era, or the legitimate elevated path) can be touched by
reset/recovery. **Disposition: OWNER + follow-up batch.** Nuance: reset is user-initiated (supervised,
not silent automation) and recovery only completes txns already begun through the (now-gated) path, and
a non-elevated restore of a real privileged target simply FAILS rather than corrupting — so this is
lower-reachability than the raw 🔴 implies. But it IS a real "gate doesn't dominate" gap. Ace
recommends: gate reset+recovery to SKIP (leave untouched + surface) privileged/`Unresolved`-scope
targets rather than blind-restore. Needs the "what should reset do with a privileged ledger row"
decision — see Owner decisions.

## Disposition codes

| Code | Meaning |
|---|---|
| **FIX** | Mac-closable real bug/robustness gap → fix now (source-analyzable + msvc-checkable even for dm-windows). |
| **GUARD** | Mac-writable *fail-closed guard* so the blind Windows layer can't silently ship a red-line bypass. Fix now. |
| **WIN** | Irreducibly Windows-runtime ([WV] / §8a). Record + ensure tracked; not Mac-fixable. |
| **OWNER** | Needs a Boss/spec decision (design change, not a mechanical bug). |
| **PERF** | Perf/architecture backlog (🔵). Record; opportunistic. |
| **RETAIN** | Parity-retained (frozen-oracle mirror, ADR-0019) or otherwise by-design. Keep. |
| **MOOT** | Not a real defect on 二次核实 / already resolved. |

## Severity tally (raw, pre-verification)

| Pass | scope | 🔴 | 🟠 | 🟡 | 🔵 | ⚪ |
|---|---|---|---|---|---|---|
| A1 | dm-domain | 3 | 21 | 6 | 1 | 0 |
| A2 | dm-contracts | 0 | 0 | 2 | 1 | 2 |
| B | icon-core pipeline | 1 | 6 | 12 | 18 | 6 |
| C | icon-core algorithm | 0 | 4 | 7 | 12 | 10 |
| D | codec + wasm | 0 | 4 | 6 | 0 | 1 |
| E | txn | 6 | 5 | 6 | 4 | 0 |
| F | icons/ledger/wallpaper/scope | 6 | 12 | 6 | 4 | 2 |
| G | resident | 1 | 8 | 3 | 3 | 1 |
| H1 | windows core | 3 | 12 | 8 | 3 | 0 |
| H2 | windows apply+runtime | ~13 | ~15 | 4 | 0 | 0 |
| I | elevated | 10 | 1 | 3 | 0 | 0 |
| J | tauri + bridge | 4 | 12 | 7 | 3 | 2 |

Raw 🔴 count is inflated by the CAS/fingerprint family (F1) and the elevated-boundary family (F6),
which are largely one root cause each + known [WIN] items — see families below.

---

## Cross-cutting families (where the real signal is)

### F1 — CAS / fingerprint atomicity is check-then-mutate, not compare-and-swap
Members: A1 `ports.rs:42` (root: ports take no expected fp), E `driver.rs:125`, E `recovery.rs:265,250`,
G `reconciler:385`, A1 `ports.rs:19`, F/J host empty-scope tie-in.
- **Root TOCTOU (apply/restore overwrite a newer user edit)** → **WIN.** True atomic CAS needs a
  platform-side conditional write under one lock/handle; already tracked as ship-readiness §8a
  "CAS-poisoning" / "overlay snapshot-once cross-process race". Not Mac-fixable. Reconcile into §8a.
- Recovery restoring an untouched user edit (E `recovery:265`) / ignoring `ItemRolledBack`
  (E `recovery:250`) → **FIX (partial, Mac-testable):** the *journal-the-intended-fingerprint-before-
  mutate* + *skip durably-rolled-back items on replay* logic is pure Rust + fake-testable. The final
  live-state re-verify is [WIN]. Split: fix the replay state machine now, tag the live re-read [WV].

### F2 — §14 privileged-root red-line is FAIL-OPEN, and the host passes empty roots ⚠️ TOP PRIORITY
Members: F `scope.rs:89` (empty roots → `None` → item passes), F `scope.rs:22` (lexical normalize can't
establish ancestry), F `mod.rs:578` (reset has no red-line), J `icon_host.rs:171` (Windows host builds
both root lists empty), G reconciler tie-in.
- **GUARD (Mac-writable, fix now):** make empty privileged-roots an ERROR on the scope gate (fail
  CLOSED) so a version-switch / reset / resident write can never classify a `C:\Users\Public\Desktop`
  item as per-user when roots are unresolved. This was *documented* in the M7 re-review but the guard
  was never enforced and the host still constructs empty lists — codex re-found it. This is the single
  most important fix: it is the safety net the "dumb Windows AI" will not add itself.
- The junction/reparse ancestry resolution (open-handle final path) is **WIN** (needs Windows APIs),
  but the fail-closed-on-empty + repeat-check-at-write-boundary structure is Mac-writable now.

### F3 — fail-open metadata probes (`exists()`/`is_dir()` swallow IO errors) 🎯 mechanical
Members: H2 `apply/folder.rs:44`, `apply/file_wrapper.rs:43`, `apply/shortcut.rs:16`, `durable.rs:127`;
H1 tie-ins; F `wallpaper/decode.rs:18`.
- **FIX (whole family, low risk):** replace `Path::exists()` with `try_exists()` and treat only genuine
  `NotFound` as benign; propagate permission/sharing/device errors instead of reporting a phantom
  "absent → success". Mechanical, msvc-checkable, exactly the class the blind Windows AI misses.

### F4 — unbounded allocation from the untrusted webview boundary 🎯 DoS
Members: F `icons/mod.rs:75` (count), F `wallpaper/mod.rs:62` + `decode.rs:21` (base64), J
`icon_host.rs:431,449` (count/chunks), J `wallpaper_host.rs:96` (base64), D `lib.rs:248` (render ABI no
out-capacity), A1 `restore.rs:23` (anchor no size bound).
- **FIX:** cap `count` against the live scan max + a small absolute ceiling before `Vec::with_capacity`;
  bound encoded+decoded base64 bytes and PNG dimensions before decode; enforce per-chunk + cumulative
  session byte caps; validate the WASM render out-buffer capacity. The webview is the Tauri trust
  boundary — these are real. All Mac-writable + unit-testable.

### F5 — registry restore is lossy / fail-open (recyclebin + overlay)
Members: H2 `recyclebin.rs:56` (restoring an originally-absent key `delete_subkey_all` nukes external
state), `:57` (delete failures discarded → false success), `:132` (lossy UTF-16 string decode); I
`overlay.rs:73,96,95`; A1 `restore.rs:61`.
- **FIX (Mac-writable half):** delete only *owned* values and remove the key only if it is app-created
  + empty; propagate every non-NotFound delete error; capture/restore the raw registry blob + type code
  (extends the APPLY-3 fail-closed fix already landed). 
- **WIN half:** the cross-process apply/restore mutex (I `overlay:86`) — [WIN], §8a.

### F6 — dm-elevated is a privilege boundary with holes (highest security severity)
Members: I `main.rs:9,21`, `guards.rs:55,40`, `args.rs:51,44`, `overlay.rs:73,96,95,86,92`,
`secure_dir.rs:103,159`; D `ico.rs:178`.
- **FIX now (Mac-writable):** strict per-verb arg grammar returning `Result` (reject unknown flags,
  duplicates, missing values, non-whitelisted `--style`; `args_os` not `args` to avoid the unpaired-
  surrogate panic); `--file` accepts only local files under an authenticated staging root, rejects
  UNC/device/reparse; full ICO/DIB structural validation (shared with codec D `ico.rs:178`);
  overlay raw-value snapshot + propagate delete errors + retain snapshot until postcondition verified.
- **OWNER / M8 / WIN:** signed helper installed + ACL'd under Program Files, `requireAdministrator`
  manifest, Authenticode verification before launch, and the confused-deputy fix (nonce-bearing
  request bound to the trusted host). These are packaging (M8) + an auth-model design decision.

### F7 — icon analysis assumes square sources
Members: B `source_facts.rs:76` (🔴), C `background.rs:151` (🟠), C `background.rs:32` (integer-div
parity), D `ladder.rs:38`.
- **FIX (verify reachability first):** background/ring analysis uses `width` for both axes → a non-square
  raster panics or misreads. If native ingress guarantees square (256²), severity drops to a guard;
  either way make it width/height-aware or reject non-square explicitly at the session boundary.
- C `background.rs:32` (`85 < 256/3` integer vs oracle float `85 < 85.333`) is a **real byte-parity
  divergence** → **FIX** by matching the oracle's float division (oracle is truth, ADR-0019).

### F8 — NATIVE_ARROW never initialized on the native render path
Members: B `batch:20`, `output_cache:249`; C `marks/mod.rs:203`; G reconciler tie-in.
- **WIN (part of unbuilt resident wiring):** the native resident/version-switch loop is itself unwired
  (G `lib.rs:25` — decision core has no production caller; = T8 [WV]). The arrow-init helper is
  Mac-writable and should land WITH the resident loop wiring on Windows. Record under T8.

### F9 — performance / architecture (🔵, ~40 findings)
Redundant recompute (segmentation run twice: C `segment:36`, `analysis:22`; profile+facts overlap),
clone-heavy hot paths (output_cache full-tile copy, arrow raster clone per icon, glass/mono/rim
scratch), LUT re-entry (B `color:49`), ledger full-rewrite per item (F `store:106`), god-modules
(dm-windows `source.rs` 757, `icons/mod.rs` 675, `txn/driver.rs` 520). Many tie to **"the `fast`
feature cache is a scalar passthrough in native production"** (B/C: spec 07 warm-render path not
wired). → **PERF backlog.** Opportunistic `Arc` swaps + the fast-path wiring are worth doing but are
optimization, not correctness; record + pick off after correctness batches. God-module splits →
refactor backlog (`/refactor-split`).

### F10 — dead / legacy code
- **RETAIN (parity, ADR-0019):** icon-core `color.rs` 6 exports, `mono` tail, `shapes` Seg::Line/Quad,
  `DerivedPlate`, `ShadowMode::Halo`, `dispersion`, `GlassMark`, smooth `q`/dead-writes,
  `profile.rs` cert-only fields. Each is a named frozen-oracle mirror. Already the ICON-9/11 precedent.
- **FIX (genuine dead, Mac-writable, low risk):** A2 `OverrideModeDto` (dead after setLook left the
  bridge), D `spike4_*` exports (gate behind a test feature), icon-core `render_slice_tile` Spike-4
  compat (retire once cert is sufficient — verify), filters `glass` re-export leak, `config.shortcut_shape`
  dead in the resolved ABI/key, and the stale docs/comments (icon-core `lib.rs:14`, J `lib.rs:86`,
  `types.ts:265`, A2 `lib.rs:4`, source_facts key doc).

### F11 — make-illegal-states-unrepresentable (dm-domain type modeling)
- **FIX (small, clearly-correct):** A1 `fingerprint.rs:48` `from_hex` Unicode-slice panic → parse bytes;
  A1 `item.rs:22` `ItemId::from_raw` validation; A1 `item.rs:31`/E `id.rs:25`/F `store.rs:40`/E overflow
  → `checked_add`; A1 `item.rs:153` `can_style` ignores `requires_explicit_consent`.
- **OWNER (larger model changes, real correctness but wide blast radius):** A1 `restore.rs:39`
  WrapperAnchor contradictory state (Absent|Present enum), `restore.rs:51` RecycleBinAnchor,
  `restore.rs:61` RegistryValue raw-bytes+type, `asset.rs:29` Single|Paired, `item.rs:89` ItemState
  discriminated enum, `item.rs:125` Windows-path WTF-16, `item.rs:31` 64-bit stable-ID collision,
  `error.rs:10` PortError missing permission/cancel/busy variants. Recommend doing the enum-anchor
  changes (they close real restore-correctness holes) but they touch serde + every applier — Boss's
  call on scope/timing.

---

## Fix queue (ordered — each batch = 二次核实 → fix → workspace+msvc+tsc+bun gates → codex re-review → literal-pathspec commit)

1. **B1 — §14 fail-closed guard (F2).** scope gate errors on empty roots; reset gets the red-line;
   repeat-check at write boundary structure. *Highest safety value.*
2. **B2 — webview alloc caps (F4).** count/base64/chunk/PNG-dimension caps across icons + wallpaper
   hosts + ops + WASM render ABI + anchor size bound.
3. **B3 — fail-open metadata (F3).** `exists()`→`try_exists()` family across dm-windows apply/* +
   durable + decode; propagate non-NotFound.
4. **B4 — elevated boundary (F6 Mac-half).** strict arg grammar (`args_os`), `--file` UNC/device/reparse
   reject, ICO/DIB structural validation (shared w/ codec), overlay raw-value + error propagation.
5. **B5 — registry restore correctness (F5 Mac-half).** owned-value-only delete, propagate delete
   errors, raw-blob+type capture (extends APPLY-3).
6. **B6 — quick correctness (F11 small + F7 parity).** from_hex, ItemId validation, overflow checks,
   can_style consent, integer-div oracle-parity, non-square guard.
7. **B7 — dead-code + stale-doc cleanup (F10 genuine half).**
8. **B8 — recovery replay state machine (F1 Mac-half).** journal-intended-fp + skip rolled-back;
   live re-read tagged [WV].

PERF backlog (F9) + god-module splits: after correctness batches, opportunistic.

## Owner decisions (accumulated — the "过一下" list)

1. **CAS fingerprint coverage (H2 `state_reader:49/73/91`).** Fingerprint covers only icon fields, so a
   user edit to a shortcut's target/args (or a folder's `desktop.ini` non-icon settings) is invisible
   to CAS and gets clobbered on restore. **Ace recommends: restore only the OWNED icon fields** (we
   don't own the rest), or at minimum fingerprint the full file to fail closed. Design call.
2. **dm-domain model hardening scope (F11 OWNER half).** Do the enum-anchor rewrites now (close real
   restore holes, wide blast radius) or defer? Ace leans DO the anchor enums, defer PortError/path-WTF16.
3. **Elevated auth model (F6 OWNER half).** Confused-deputy: bind each helper invocation to a
   nonce-bearing request from the trusted host + Authenticode-verify + Program-Files install. This is
   M8 packaging + an auth-protocol design. Ace recommends scheduling it into M8, not v1-blind.
4. **System CLSID scope (H1 `scan:93`, H2 `apply/mod:62`, A1 `restore:83`).** This-PC/Network/Control-
   Panel icons are `Unsupported` (known stub). Implement per-user CLSID `DefaultIcon` discovery/apply
   now (blind + [WV]) or leave stubbed for v1? Ace recommends blind-writing it (Boss wants max done here).
5. **Non-square source policy (F7).** Guarantee-square-at-ingress vs make-analysis-shape-aware. Ace
   recommends an explicit square-normalize-or-reject at the session boundary + keep goldens square.
6. **reset/recovery privileged-scope gating (F2b, codex B1-🔴).** Should `reset_to_original` and crash
   `recovery` SKIP privileged / `Unresolved`-scope ledger+journal targets (leave them untouched +
   surface), or blind-restore them (today's behavior)? Ace recommends SKIP+surface (a privileged row
   is either old-fail-open debris or belongs to the elevated path, neither of which a non-elevated
   background restore should silently touch). This is the follow-up batch that finishes "the gate
   dominates every mutation."

## Not Mac-fixable — reconcile into ship-readiness §8a / §[WV]

Platform-conditional CAS write (F1 root), cross-process overlay mutex (F5/I), durable-replace
power-loss barrier (H2 `durable:127`), disposable-STA isolation worker for shell extraction (H1
`source:36`), `ReadDirectoryChangesW` overflow surfacing + watch re-arm (H2 `watcher:16,130`), the
watcher→reconciler→driver LOOP + tray/residency wiring (G `lib:25`, H2 `watcher:99` = T8/T11),
WinEventHook drag precision (H2 `activity:75`), signing/ACL/packaging (F6 = M8). Most already in §8a.
