# Spec 07 — Background Resident Auto-Format (v1)

Living spec. Normative behaviour for the resident tray process that auto-formats new
desktop icons per the user's saved style. Decision record: ADR-0020 (v1 inclusion +
incremental-ledger restore), ADR-0021 (global arrow overlay default), ADR-0022 (M7
appearance model, trust-first reset, resident trust/consent model, resident precondition),
ADR-0019 (native Rust renderer, no WebView). Supersedes spec 06 §7. Panel record:
`docs/reviews/2026-07-12-m7-resident-panel.md`. Owner approved the M7 design brief PART
1–3 on 2026-07-12.

## Scope / Non-scope / Assumptions / Dependencies

- **Scope**: one per-user resident process; watch user + public desktop; detect
  new/changed items; auto-format user-desktop items per saved style, `kindPolicy`,
  `typeOverrides`, per-icon keeps; the three-store appearance-version data model (§8);
  switching between saved appearance versions by projecting onto the current desktop
  (§9); resetting all icons to their original, pre-DeskMakeover state (§10); conflict
  handling; activity detection so automation never fires under the user's cursor (§11);
  the tray surface state machine (§12); the reversible-touchpoint ladder (§13); the
  privileged/pending-elevation queue (§14); catch-up reconciliation.
- **Non-scope (v1)**: background wallpaper changes (wallpaper is edit-time only,
  spec 04); machine-level/public-desktop silent writes (queued instead, §14); any
  elevation from the background (§14); Explorer-module surfaces (drive/folder trees
  beyond the desktop); ML-based anything; per-icon overrides persisted *inside* an
  appearance version (§8 — only the three global knobs project; per-icon overrides
  stay ledger-local until a v2 `overrides` map is built).
- **Assumptions**: the manual apply/restore path (same crates) has passed its own
  gates; the saved style (② in §8) is the single style truth (no separate "background
  style"); resident automation cannot be enabled before the user has completed one
  successful global Apply (§8, §14 precondition) — this makes "a saved style exists"
  an invariant, not an edge case to special-case around.
- **Dependencies**: `dm-resident` (reconciler/jobs/queue/consent ladder/stability probe —
  the decision core is BUILT + hardened on Mac, `crates/dm-resident`; the [WV] platform
  bodies — tray+windowless residency wiring, tray bitmaps, the watcher→reconciler→driver
  loop, the WinEventHook precision layer — are still unwritten, status in
  `docs/ship-readiness.md`), `dm-windows` (watcher, scan, writers, STA actor,
  activity-detection WinEventHook), `dm-icon-core` native (headless render, `--features
  fast` — see §15), `dm-operations` (durable ledger + the saved-style column +
  `LookHistoryStore`, §8), tray/single-instance/autostart plugins (ADR-0019), `notify` +
  `notify-debouncer-full` (§3, §16), `tauri` built with the `tray-icon` feature (§16 — the
  tray body lands with the platform wiring).

## 1. Process model

- One long-lived, non-elevated Rust process per user session. `--background` startup
  creates NO WebView; the tray icon is the only surface until the user opens the
  window. Closing the window destroys the WebView (verified child-process exit) and
  returns to windowless residency.
- Single instance per Windows user/session, reusing `tauri-plugin-single-instance`
  (already a dependency) rather than a second process — the main app degrades to
  windowless residency when the window closes instead of a separate resident binary
  existing. A separate machine-scope lock guards global-overlay/public-desktop
  operations.
- **Autostart**: registers ONLY after the user explicitly enables resident automation
  (the first-run consent in §2, or the settings toggle later); disabling automation
  unregisters it in the same action. Test: toggling automation off leaves zero
  autostart registry/Task-Scheduler residue.
- **Crash recovery**: reuses `run_startup_recovery` (`src-tauri/src/lib.rs:171`) →
  `dm_operations::recover_from_journal` against the shared `txn.log`/`ledger.json` —
  the resident process and the foreground app recover from the SAME durable journal,
  so a crash mid-background-apply is indistinguishable from a crash mid-foreground-apply
  to the recovery path.
- Tray menu (fixed order — the normative menu lives in §12, restated here for
  continuity): 状态行(不可点) · ☑自动整理新图标 · 立即整理桌面 · 查看最近整理记录(N) ·
  撤销最近一次整理(窄,仅在有可撤销项时可点) · 打开 DeskMakeover · 设置 · 退出.
  **"恢复系统默认(全部)" is never a tray item** — it lives only in Settings ›
  Advanced behind a confirmation (§10, §13 level 4). Every label states what it does;
  no mystery verbs.

## 2. Trust model and the consent ladder

Carries forward the spec 06 §7 trust contract and formalizes it per ADR-0022.

1. Feature default OFF. The offer to enable it appears as a **non-modal confirmation
   strip** the FIRST time the user finishes a manual format (not an independent
   launch-time dialog): 「已美化 N 个图标。以后新出现的图标要不要自动保持这个风格？
   [开启自动整理][暂时不用]」, both buttons equal weight. This is the only place the
   feature is offered before the user has a style to keep.
2. **Resident enable precondition**: the automation toggle is disabled (greyed) until
   ② saved-style (§8) is non-empty, i.e. until the user has completed one successful
   global Apply. This is enforced in the UI AND the settings write path — a patch that
   would set `keepNewIconsStyled=true` while saved-style is empty is rejected, not
   silently accepted.
3. Choosing "开启自动整理" reveals a **three-fact disclosure**, inline, no extra click:
   「检测到新图标且你不在操作桌面时才触发 · 随时可在托盘撤销或恢复原图标 ·
   托盘右键随时可关闭」.
4. **New icons are never formatted purely silently in v1.** The default reaction to a
   detected new icon is a **batched proposal**: group pending new icons, surface one
   confirmation with a **timeout** (default 2h) after which the batch auto-applies if
   the user takes no action. This preserves the "zero-click eventually" value of
   automation while keeping a moment where the user is told a file is about to be
   rewritten — silent-forever would remove the very signal the trust model depends on
   (see next point).
5. **Why not pure silence**: the "applied and the user never objected" revealed-
   preference argument only holds if the user could *perceive* the automatic change.
   A purely silent background rewrite removes that perception, so it removes the
   premise the argument needs — "no complaint" would mean "not noticed," not "wanted."
   Pure per-icon silent mode is available ONLY as an opt-in **second-tier** setting,
   unlocked after the user has been through at least one confirm-or-timeout cycle; it
   is never the v1 default.
6. **Feedback channel = the OS-native notification** (`tauri-plugin-notification` /
   WinRT toast), never a self-drawn webview toast: it is Narrator-readable, respects
   Focus Assist/quiet hours, appears in the Windows Action Center, and carries an
   inline "撤销" action.
7. **Feedback tiering** (three tiers, never all-or-nothing):
   - **Environment layer** (always on, every run): a 2-second tray pulse + a brief
     fade-in highlight on each newly-formatted desktop icon.
   - **Trust-building layer** (the first 3 automatic batches only): the native toast
     above, with an inline undo.
   - After 3 consecutive batches the user did NOT undo, the toast tier is dropped and
     only the environment layer remains — this is a per-user counter, reset if the
     user ever undoes a batch.
   - **Any anomaly** (a conflict, a partial batch failure, an item that needed
     elevation) ALWAYS surfaces the toast tier regardless of the counter — anomalies
     never silently degrade.
8. **"Intent freshness" health check** (downgrades a would-be-silent batch back to a
   proposal, never suppresses it): before applying a batch under an already-earned
   silent tier, re-check two staleness signals; either one downgrades the batch to a
   proposal for that run only (the trust counter is not reset by a downgrade):
   - **Stale intent**: more than 60 days since the last global Apply (saved-style's
     write timestamp).
   - **Partial reversion**: since the last global Apply, the user has individually
     reverted one or more previously-styled icons back to the system default — read
     as a signal they are stepping away from the style, not as noise.
9. Every automatic change lands in the SAME ledger/history the manual flow uses — one
   undo surface (§5, ADR-0020 §2).
10. Turning automation off never retro-reverts already-applied icons, and the settings
    UI states this plainly next to the toggle.
11. **The background NEVER pops UAC.** Public-desktop/machine items queue as visible
    pending work; one batched UAC completes them when the user opens the window (§14).

## 3. Change detection (events are hints; reconciliation is truth)

- Sources: `notify` 7.x + `notify-debouncer-full` (NOT `-mini` — `-mini` only does a
  time-window dedupe; `-full` tracks file IDs and folds a temp-write→rename pair into
  one logical event, which installers rely on) on the user desktop AND public desktop
  (paths via `SHGetKnownFolderPath` — never hardcoded, re-resolved after resume,
  policy change, OneDrive Known Folder Move); Shell change notifications for virtual
  items; full reconcile on: startup, wake-from-sleep, watcher overflow/error, Explorer
  restart, app update, abnormal-exit recovery.
- Debounce: user-configurable 2–10s (default 4s) coalescing installer bursts.
- Stability probe before processing ANY new item (generalized beyond `.lnk` — file
  opens, size+mtime stable across two probes, non-exclusive lock) — covers ordinary
  files/folders as well as shortcuts, not only `.lnk` targets; a `.lnk` additionally
  requires `IShellLink` to parse and both target + `IconLocation` to be populated.
- Handle Created/Changed/Renamed/Deleted; installers commonly write-temp → rename.
- Reconciliation NEVER runs while the desktop is busy under the user's mouse (§11) —
  a pending reconcile wave is deferred, not dropped: "events are hints, reconciliation
  is truth," so a deferred wave still eventually reconciles the true on-disk state,
  never loses an event.

## 4. Identity and self-write suppression

- Source fingerprint covers MORE than the `.lnk` bytes: link fields, target path +
  target file version/mtime, IconLocation + its file state, AUMID/package family +
  version + chosen resource variant. A target app update with an unchanged `.lnk` IS
  a change.
- File identity (volume + file id) distinguishes rename from replace.
- Self-write guard: operation id + before-hash + expected-after-hash + time window;
  our own applies never re-enter the queue (no format loops).

## 5. Apply and the incremental ledger (ADR-0020 §2)

- Render: native `dm-icon-core` via the same `RenderSession` model as the foreground;
  profile cache keyed `source_hash + analysis_schema_version` is persisted on disk, so
  a background single-icon format skips re-analysis.
- Hue: new icons allocate against **pinned existing seeds**; existing icons never
  reflow. Global rebalance only via explicit foreground re-apply.
- Generated ICOs are content-addressed (`<source-hash>-<style-hash>.ico`); write new
  file first, then swap IconLocation; GC only ledger-unreferenced files.
- Ledger entry per item (`dm_operations::ledger::LedgerEntry`): original fingerprint +
  restore anchor (`original_anchor`), last-applied fingerprint (the CAS anchor,
  `last_applied_fingerprint`), owned fields, generated asset, a per-item sort key
  `version`, transaction state (prepared → asset-written → applied → verified →
  committed, `TxnState`). Persist + flush before every external mutation
  (`JsonLedgerStore` — atomic temp-file+rename write, fail-closed on a corrupt file:
  `OperationError::CorruptLedger`, NEVER an empty list).
- **This ledger is store ① of the three-store appearance model (§8) and is the ONLY
  store an incremental background apply writes to.** An incremental auto-format run
  does not touch ② saved-style or ③ look-history and therefore never creates a new
  appearance-version entry — it is not "the same operation as a global Apply, just
  smaller," it is a structurally different, narrower write.
- Each background run appends incremental ledger entries via `TxnDriver::apply`
  (`crates/dm-operations/src/txn/driver.rs`) — the SAME entry point the manual
  foreground flow uses. One undo surface, no parallel bookkeeping.
- Restore/re-apply is per-item compare-and-swap (`TxnDriver::prepare_item`): current
  state ≠ our last-applied fingerprint → visible conflict (user/installer wins); no
  silent overwrite; no restore-the-whole-desktop-first behaviour. This CAS check is
  also what makes appearance-version switching (§9) and reset (§10) data-safe: an
  item the user hand-edited since is structurally never overwritten by either
  operation.
- Ordinary-file wrapping (structural: companion `.lnk` + hidden original) defaults to
  the proposal queue even in silent mode; a setting may promote it.

## 6. Exclusions

Automation (incremental auto-format, version switching, AND reset) never touches:

- **Anything under `Public Desktop` / `ProgramData`** — administrator/installer-
  deployed items, not the user's own desktop; this also structurally sidesteps every
  item that would need elevation (§14).
- **System-drawn functional badges**: Mark-of-the-Web (download-source shield),
  cloud-sync status overlays (OneDrive/Dropbox), enterprise-compliance badges. The
  icon formatter touches only the primary icon visual, never a badge DeskMakeover did
  not itself draw — touching one of these turns "beautify" into "hide a security
  signal." Engineering must verify the formatter's write path cannot clobber these
  before shipping (open item — see plan `2026-07-12-m7-resident.md` §4 Tests).
- **Unstable temp/partial files**: `.crdownload`/`.tmp`/mid-install artifacts —
  caught by the generalized stability probe (§3), not a special-cased extension list.
- **Per-icon `styleable:false` items, `kindPolicy=false` buckets, per-icon
  「保留原样」, and items with an unresolved conflict flag** — the existing
  manual-apply exclusion surface, unchanged for the background path.
- **`ItemKind::System`** (此电脑/网络/用户文件/控制面板 — the per-user CLSID
  `DefaultIcon` family) is styleable via manual apply (spec 06 §6) but is
  structurally never a watcher hit: it is not a filesystem entry the desktop
  directory watch can observe, so it never enters the auto-format trigger surface.
  This is a consequence of what the watcher can see, not a policy carve-out — do not
  implement a special case to "exclude System"; there is nothing to exclude.

## 7. Verification (release gates for the resident path)

- Burst test: temp-write→rename installer storms; exactly one format per final item.
- Overflow test: forced watcher overflow → full rescan converges, nothing missed.
- Self-write test: N applies produce zero re-queued events.
- Kill-point battery: process death injected around every ledger transition; restart
  recovers to a consistent state; restore stays exact.
- Conflict test: externally modify a styled item; automation flags, does not touch.
- Environment matrix: OneDrive-redirected desktop, sleep/resume, Explorer restart,
  standard user, second user on the same machine (public desktop untouched).
- Idle budget: resident process ≈0% CPU idle, no WebView children, bounded RSS;
  warm single-icon background format under 250ms (Codex budget), compiled with
  `dm-icon-core --features fast` in the resident/release profile (cold analysis
  7.6–18.4 ms/icon vs warm 0.67–2.5 ms/icon — see §15; the budget is unreachable
  without `fast`).
- **New M7 gates**:
  - Reset trust-first test: seed a ledger with N items, externally modify M of them,
    run reset → exactly N−M items restore, M are left untouched and reported
    "已跳过 M 项(你自己改过)", ② saved-style is empty afterward, the auto-format
    toggle is false afterward, and (if the overlay was ever enabled)
    `OverlayControl::restore` was called exactly once.
  - Version-switch projection test: switch to a version whose remembered style
    differs from the current desktop's item set (icons added AND removed since);
    assert new icons are picked up, vanished icons produce no orphaned requests, and
    CAS-conflicted icons are skipped exactly as in a normal apply.
  - Freshness-downgrade test: a saved-style older than 60 days, or a desktop with a
    partial reversion since the last Apply, downgrades a would-be-silent batch to a
    proposal.
  - Precondition test: attempting to enable resident automation while ② saved-style
    is empty is rejected at both the UI and the settings-patch layer.
  - Tray state-machine test: every declared transition in §12's table is reachable
    and no undeclared transition exists (exhaustive over the 5-state machine).
  - Activity-suppression test: a synthetic drag/capture event on the desktop
    `ListView` handle suppresses a pending batch for at least 1.5s past the event's
    end, and the suppressed batch is NOT dropped (still applies once the desktop
    goes idle).
  - 10-cap dedup test: pushing an identical `IconStyleDto` as the current
    look-history head only bumps its timestamp, never grows the list; pinned entries
    never get evicted by the 11th distinct push.
  - Privileged red-line test (§14): construct a scenario requiring elevation (a
    public-desktop item, or the overlay) inside the resident reconciler and assert
    `dm-elevated`/`OverlayControl::apply` is never called from that code path — only
    enqueued to the pending-privileged queue.

## 8. Appearance model: the three-store architecture

### 8.1 Core reframe

`version` names an **appearance preset** (外观/外观方案), never a "desktop snapshot."
Users do not expect switching a look to resurrect icons they deleted — the "my icon
set changed" confusion only exists under a snapshot mental model, and the
appearance-preset model does not produce it, so there is nothing to work around.

- **UI terminology law (binding on every user-facing string)**: never use
  版本/快照/回退/时光机/恢复到某刻. Always 外观/外观方案/应用/恢复系统原始外观.
  Internal code may keep calling the field/struct `version`.
- A saved appearance's thumbnail MUST be a **style sample** (3–4 representative icons
  rendered under that style + a wallpaper-tone swatch), and MUST NEVER be a
  historical desktop screenshot — a screenshot re-imports the snapshot mental model
  this reframe exists to remove.

### 8.2 The three stores (orthogonal — this is what "version" used to conflate)

| Store | Owns | Written when | 10-cap? |
|---|---|---|---|
| **① Active ledger** (`JsonLedgerStore`, `ledger.json`, exists today — `crates/dm-operations/src/ledger/`) | The only reversible truth. One row per icon (key = `ItemId`): `original_anchor` (the permanent true original), `last_applied_fingerprint` (CAS anchor), `asset`, `owned`, `pinned_seed`, `version` (a per-item sort key — unrelated to store ③'s `version`). | Every global Apply AND every incremental auto-format — through the SAME `TxnDriver::apply` (ADR-0020 §2: one undo surface). | **Never.** Dropping a row permanently loses that icon's `original_anchor` — there is no other copy of "what Windows originally showed." |
| **② saved-style** (NEW — a single row, `SettingsStore` gains an `icon_style_json` column, `crates/dm-operations/src/settings_store.rs`) | The single truth for "the current global style." One `IconStyleDto { config, kindPolicy, typeOverrides }` — the three global knobs, **no per-icon overrides** (a per-icon override could not apply to an icon that did not exist yet when it was recorded). | **Only when the user completes a global Apply** (the烘焙→写盘 path). A `setLook` draft debounce (400ms drag preview) is NOT intent; a single-icon edit is NOT intent; neither writes here. | Single row — no history. |
| **③ look-history** (NEW, `LookHistoryStore`, its own file `look-history.json`, corruption-**tolerant** — the opposite fail-safety direction from ①, which is why it MUST be a physically separate file) | Up to 10 "appearance recipes" the user can switch back to. `Vec<LookVersion { id, created_at, label, icon_style }>`. **Never stores an icon list.** | One entry pushed per global Apply (subject to the dedup rule in §17). | **Yes, cap 10 for this store only.** A corrupt/missing file reads as an empty history (never blocks apply/restore), the mirror-opposite of ①'s fail-closed rule. |

One-sentence boundary: **① is how to restore. ② is what style applies right now.
③ is which styles were used before.** All three carry `IconStyleDto`-shaped values
(`config`/`kindPolicy`/`typeOverrides`, matching `src/bridge/types.ts`
`ConfigDto`/`KindPolicy`/`TypeOverrides`) but have different lifecycles and different
fail-safety directions.

### 8.3 Confusion resolution (why this model, item by item)

- **"What was 'last style' again?"** → read ②. Only a global Apply writes it, so
  "last style" unambiguously means "last global Apply," never "last single-icon
  tweak."
- **"Does an incremental auto-format create a new version?"** → No. It writes ONLY
  store ① (one ledger row). It never touches ②/③, so it structurally cannot create a
  version-history entry.
- **"Does an appearance version remember which icons existed?"** → No, by design
  (§8.2 ③) — an icon list recorded at save time goes stale the moment an icon is
  added or removed, and collides with the projection algorithm (§9). The icon list
  for "switch to version Y" is always the LIVE scan at switch time.
- **"What does switching back to an old version do?"** → Sets ③'s recipe as ②'s
  current saved-style, then **projects** it onto the current live scan (§9).
  Switching a version and running an incremental auto-format are literally the same
  primitive underneath: `resolve → bake → TxnDriver.apply`.
- **"What about 'apply the system default' (no style)"?** → ② becomes `null`; the
  automation layer treats a null saved-style as "nothing to project," which is
  exactly the "stay off unless the user picks a style" behaviour — no special-case
  code path, it falls out of the model.

### 8.4 Action → state-coupling table (binding — the single source for what each user action touches)

| User action | Existing icons | ② saved-style (source) | Auto-format toggle |
|---|---|---|---|
| Global batch-apply appearance X | All adopt X | **← X** | unchanged |
| Incremental auto-format (same appearance X) | Only newly-added icons adopt X | unchanged | unchanged |
| Single-icon manual edit | Only that icon changes | **unchanged** (never polluted by a per-icon edit) | unchanged |
| Switch to a saved appearance Y | All (live scan) adopt Y | **← Y** | unchanged |
| **Reset to original appearance** | All restore to system default per §10 | **cleared** | **turned off** |
| No global Apply has ever run | — | empty | **dormant — never fires** (a single-icon edit never substitutes for "a style exists") |

## 9. Switching appearance versions (projection algorithm)

Switching to a saved version is NOT "replay what was recorded" — it is "make the
CURRENT desktop match the recorded style," because store ③ deliberately records no
icon list (§8.2).

```
switch_to_version(v):
  style   ← LookHistoryStore.get(v).icon_style          # ③ read
  Store2.saved_style ← style                              # ② becomes the new current global style
  current ← DesktopScanner.scan()                         # live desktop (shell/scan.rs — a lightweight read_dir walk)
  requests ← []
  for item in current:
    if item matches an exclusion (§6): continue
    cfg      ← resolve(style.config, style.kindPolicy, style.typeOverrides, item.kindBucket)
    ico      ← bake_native(cfg, item.source)               # dm-icon-core RenderSession, native
    expected ← ledger.get(item.id)?.last_applied_fingerprint ?? scan_fingerprint(item)
    requests.push(ApplyRequest { target: item.target, expected_fingerprint: expected, asset: ico, ... })
  TxnDriver.apply(txn, requests, journal, ledger)           # one transaction, same entry point as apply/auto-format
```

- An icon that existed under the old version but is gone now: it is not in
  `current`, so it is never requested — no orphan handling needed, the projection is
  defined over the live set.
- An icon that did NOT exist under the old version: it IS in `current`, so it gets
  `resolve()`'d and included automatically — new icons are never left out of a
  version switch.
- CAS is a free safety net here for the same reason it is for auto-format (§5): an
  icon whose current fingerprint ≠ its `last_applied_fingerprint` (i.e. the user
  hand-edited it since) fails the CAS check inside `TxnDriver::prepare_item` and is
  skipped, never overwritten.
- Per-icon overrides are v1-out-of-scope for version switching (§8.1 Non-scope). If a
  v2 `overrides: Map<ItemId, Patch>` is added to `IconStyleDto`, the projection MUST
  silently drop any key not present in `current` rather than erroring.
- Version switch and incremental auto-format are **the same primitive**
  (`resolve → bake → TxnDriver.apply`); the only difference is which items go into
  `requests` (all of `current` vs only the newly-detected subset).

## 10. Reset to original appearance ("as if never modified")

New operation `reset_to_original`, CAS-gated and journaled, reusing the driver's
existing journal + LIFO rollback machinery (`crates/dm-operations/src/txn/driver.rs`).

```
for e in ledger.all():
  match reader.read_fingerprint(e.target):
    Err(NotFound)                       → ledger.remove(e.id)                      # user deleted the icon: clear the row, GC its ICO
    Ok(cur) if cur != e.last_applied_fp → leave untouched; count toward "已跳过 N 项(你自己改过)"   # ★ see below
    Ok(cur) if cur == e.last_applied_fp → applier.restore(e.target, e.original_anchor); ledger.remove(e.id)
journal.append(TxnCommitted)
assetStore.gc(&[])                       # ledger is empty → every generated .ico is unreferenced → deleted (app-data content-addressed files only)
if the global arrow overlay was ever enabled:
  dm-elevated::OverlayControl::restore()  # one batched UAC (ADR-0021)
```

- **★ Owner-approved core decision (trust-first, not literal clobber).** For an icon
  the user has since hand-edited, a byte-literal revert would destroy the user's own
  edit and break the product's headline promise (README "Restore must stay visible
  and reliable," spec 07 §5 "user/installer wins, no silent overwrite,"
  `driver.rs:239`'s CAS check). **Reset restores only icons still exactly in "the
  state we last left them"; icons the user has changed since are skipped and the
  skip is reported, never silently reverted and never silently left ambiguous.** For
  the common case (an icon still exactly as DeskMakeover left it) "as if never
  modified" holds literally; for the conflicted case it degrades to "skip + tell the
  user," never to data loss.
- **★ Reset coupling (the easy thing to miss).** A reset is not "just" a ledger walk
  — to actually read as "as if never modified" it MUST, in the same operation: clear
  ② saved-style, **and** turn OFF the auto-format toggle, **and** restore the
  machine-level arrow overlay (one UAC, ADR-0021) if it was ever turned on. Skipping
  any one of the three leaves a state that looks reset but silently reapplies old
  style to the next new icon, or leaves the classic-arrow-hiding overlay live with no
  styled icons underneath it to explain it.
- **Edge cases already correctly handled by existing invariants** (call out so
  nobody "fixes" them):
  - Delete-then-recreate-same-name: a new file at the same path gets a fresh
    identity → its fingerprint ≠ `last_applied_fingerprint` → hits the conflict
    branch → correctly NOT restored (restoring here would overwrite the user's
    brand-new file with old `.lnk` bytes — genuine data loss).
  - "A ledger row with no restore material" cannot exist structurally: `driver.rs`
    `prepare_item` already refuses to create a row when `anchor.has_material()` is
    false (`RestoreAnchor::CaptureFailed` is the only material-less variant) — reset
    never needs to special-case it.

## 11. Activity detection (silent-desktop heuristics)

- **Data safety is already guaranteed** by the CAS check in `TxnDriver::prepare_item`
  (§5) — "never overwrite a user's own change" is structural, not a function of
  timing. Activity detection is a **UX-layer** concern only: don't let an icon
  visibly change under the user's cursor while they're mid-drag.
- **Judge 1 (primary, precise)**: `SetWinEventHook` on `EVENT_SYSTEM_DRAGDROPSTART`/
  `END` + `CAPTURESTART`/`END`, **scoped to the desktop `ListView` handle** (`Progman`
  → `SHELLDLL_DefView` (or `WorkerW`) → `SysListView32` — the same handle-resolution
  chain already documented for desktop layout reads,
  `crates/dm-windows/src/shell/layout.rs` Technique B). Scoping to this handle means
  dragging something in a browser or another app never falsely marks the desktop
  busy. A hit sets an atomic "busy until T+1.5s" flag.
- **Judge 2 (v1 fallback, coarse)**: `GetForegroundWindow`'s class is a
  desktop/Explorer class (`Progman`/`WorkerW`/`CabinetWClass`) AND
  `GetLastInputInfo` < 2s.
- **Check cadence**: re-checked between EVERY icon in a batch (reusing the perf
  doc's generation-token discipline,
  `docs/plans/2026-07-11-m6-performance-architecture.md` §4), not once at batch
  start — a user who starts interacting mid-batch stops it mid-batch.
- **On busy**: the pending batch returns to the pending queue for the next debounce
  cycle. No event is ever dropped — "events are hints, reconciliation is truth" (§3)
  applies here too.

## 12. Tray surface: state machine and menu

Five states, shape + colour double-coded (a 16px tray glyph cannot rely on hue alone):

| State | Trigger | Visual | Tooltip |
|---|---|---|---|
| OFF | user disabled automation | outline, grey | 自动整理已关闭 |
| WATCHING | enabled, desktop idle (§11) | solid, brand colour | 正在为你保持桌面风格 |
| PAUSED | drag/capture detected, debouncing | solid + small grey dot, bottom-right | 桌面使用中,已暂停 |
| WORKING | formatting N new icons (≤2s typical) | solid + 2-frame pulse | 正在整理 N 个新图标 |
| ERROR | a write or the undo safety-net failed to persist | solid + red exclamation | 遇到问题,点击查看 |

- Legal transitions: OFF→WATCHING (enable) · **ANY state→OFF (the user disable, owner
  decision 2026-07-13 — see §12.1)** · WATCHING↔PAUSED (activity start/end, §11) ·
  WATCHING/PAUSED→WORKING (a batch starts) · WORKING→WATCHING (batch commits clean) ·
  any state→ERROR (a durable write/undo-journal failure) · ERROR→WATCHING (the user
  acknowledges/retries). No other transition is legal — the release gate in §7 asserts
  this exhaustively.

### 12.1 Disable is honoured from every state (owner decision 2026-07-13)

The tray "☑自动整理新图标" toggle disables from **any** state, not only WATCHING. A user
who unchecks it while the desktop is paused, a batch is running, or an error is showing
expects automation to stop — a dead no-op there is a UX trap. The two states that needed
defined semantics before this could be legal:

- **WORKING → OFF (mid-batch).** The host loop checks the persisted `enabled` flag BEFORE it
  schedules the next reconcile or apply. A batch already handed to `apply_batch` is a SINGLE
  `TxnDriver` transaction that is DURABLE and crash-recoverable: it commits all N items, or on any
  failure rolls back / is recovered to all-original — the final converged state is always
  all-or-nothing, never a PERMANENT torn desktop. The driver writes items sequentially, so a disable
  CAN land during the ≤2 s write while the desktop is transiently mid-transition (some items redrawn,
  others not); that in-flight transaction still converges atomically (commit, rollback, or
  crash-recovery), so the worst a disable costs is one in-flight batch of latency and it can never
  leave a lasting partial. No batch-cancellation plumbing lives in the decision core; transactional
  durability + recovery is the guarantee, not instantaneous per-pixel visibility atomicity.
- **ERROR → OFF.** Disabling never discards a RECORDED fault: the pending-privileged queue and any
  unrecovered journal transaction are retained across the toggle, and re-enabling runs the
  unconditional `recover_from_journal` at the top of the cycle (§3) so a recorded fault re-surfaces
  (the tray returns to ERROR) and a resolved one lets the cycle proceed — retention is structural,
  not a discardable flag. ONE fault class has no journal record to retain: a failure that PREVENTED
  journaling itself (e.g. the journal location denies writes, so `TxnBegin` fails before any record
  or mutation). Re-enabling then reads an empty journal and returns to WATCHING, and that fault
  re-manifests at the NEXT apply (which hits the same denial → ERROR again), not at enable — an
  un-recordable fault is self-re-raising rather than retained.

OFF→OFF (disabling when already disabled) is the idempotent no-op.
- Menu (fixed order, the normative version of §1's placeholder): 状态行(不可点) /
  ☑自动整理新图标 / 立即整理桌面 / 查看最近整理记录(N) /
  撤销最近一次整理(窄,仅在存在可撤销项时可点) / 打开 DeskMakeover / 设置 / 退出.
  **"恢复系统默认(全部)" is never here** — only in Settings › Advanced (§13 level 4,
  §10).

## 13. Reversible touchpoints and reset copy

### 13.1 Four levels, narrow→broad (deliberately kept separate so the user's first instinct lands on the cheap one)

1. **Toast inline undo** (trust-building tier, §2 — single action, zero navigation).
2. **Tray "撤销最近一次整理"** (always available once automation has run once —
   still narrow, one batch). Implementation contract (2026-07-16,
   `Reconciler::restore_batch`): recover-first · busy-defers (§11) · per-item CAS
   on OUR last-applied fingerprint (a hand-edit since the batch is flagged, never
   clobbered) · restores from the ledger `original_anchor` · the ledger row is
   deliberately KEPT — the restored item then reads as the manual-restore tuple
   `reconcile` silently skips, so the resident never re-proposes an undone item
   (removing the row would make it a fresh newcomer → restyle-after-undo ABA).
3. **Switch to a different saved appearance** (§9 — the primary "buyer's remorse"
   path; horizontal re-dress, strictly lighter-weight than a reset).
4. **Settings › Advanced › 「恢复所有图标为系统原始外观」** (§10 — vertical exit,
   destructive, behind a confirmation dialog whose default-focused button is
   Cancel).

Design intent: a user's first "undo this" instinct should land on level 1–3; level 4
should read as a deliberate, rare, opt-in action — never the thing a slip of a click
triggers.

### 13.2 Reset confirmation copy (binding text, addresses both misreadings at once)

Label: 「恢复所有图标为系统原始外观」(禁孤立使用"重置" — always paired with
"系统原始外观" so it cannot be misread as "reset to last-applied style").

Dialog body:
> 这会移除 DeskMakeover 对全部 N 个图标做过的**所有修改——无论你手动设置的还是自动
> 整理的**，桌面回到安装 DeskMakeover 之前的样子。
> · 你保存的 M 个外观方案会**保留**，随时可重新应用。
> · 自动整理会**一并关闭**，直到你下次应用某外观。
> [恢复原始外观] [取消（默认焦点）]

The two bullets exist specifically to pre-empt the two most likely misreadings: that
a reset would also delete saved appearance history (③, it does not — only ② is
cleared), and that automation would silently resume on the next new icon (it will
not — it is turned off, §10).

## 14. Privileged operations queue (the background never elevates)

- **Already settled by ADR-0020 §4 / ADR-0021 §5, restated here as a hard
  engineering constraint, not an open design question**: the background process
  NEVER pops UAC. A public-desktop/machine-scope item is observed and enqueued as
  visible "待处理特权项 (N)" work; opening the main window offers one batched UAC
  that drains the queue.
- This is a Windows security-model fact, not a product choice to relax later:
  `requireAdministrator` only elevates via `runas`, and there is no unattended-silent
  elevation path — a design that tried to elevate silently from the background would
  simply not compile against the OS.
- **Every existing `WindowsIconApplier` writer is already user-level-only**
  (`Shortcut`/`AppxShortcut` `.lnk` via COM, `RegularFile` file wrapping,
  `UrlShortcut`/`Folder` filesystem, `RecycleBin` — HKCU only): the auto-format
  trigger surface needs zero elevation by construction. The ONLY elevation-requiring
  verb is the global transparent-arrow overlay (`dm-elevated OverlayControl`,
  machine-scope, one-time, user-explicit-click — orthogonal to auto-format).
- **Engineering requirement**: `dm-resident`'s reconciler applies a **hard
  kind/scope filter** BEFORE any write path — an item requiring elevation or living
  under public-desktop/ProgramData is routed straight to the pending-privileged
  queue and the elevated write path is never invoked for it. The pending-queue data
  structure and its UI (the tray "待处理特权项(N)" line, §12) do not exist yet and
  are a first-class M7 deliverable (plan `2026-07-12-m7-resident.md`).
- **Release gate**: a red-line test (§7) constructs a scenario requiring elevation
  inside the resident reconciler and asserts `OverlayControl`/`dm-elevated` is never
  invoked from that code path.

## 15. Performance

- **There is no "fall back to WASM if the system is slow" branch — this was a
  mistaken premise, corrected here as a fact, not an optimization choice.** The
  resident process is WebView-less pure background Rust (§1); there is no JS host
  for WASM to run inside, so background rendering can ONLY ever call the native
  `dm-icon-core` path. "Always native" is a structural consequence of the process
  model, not a tuning decision.
- **The real performance lever is the `dm-icon-core` `fast` Cargo feature**
  (`crates/dm-icon-core/Cargo.toml` — byte-identical kernel optimizations, already
  exists and is corpus-verified): cold per-icon analysis without it is **7.6–18.4
  ms/icon**; warm render with it is **0.67–2.5 ms/icon** (measured,
  `docs/plans/2026-07-11-m6-performance-architecture.md` §1/§4). `dm-resident`'s
  release profile MUST build `dm-icon-core` with `--features fast` — without it the
  §7 "<250 ms warm single-icon" budget is not reachable at realistic batch sizes.
- **Native parallelism precondition, already fixed** — `NATIVE_ARROW` was a
  `thread_local!` in `crates/dm-icon-core/src/marks/mod.rs`; the perf doc §4a
  flagged this as the hard prerequisite before ANY native `rayon` use (a worker
  thread that never set the thread-local would silently render the fallback arrow
  instead of the real one — output would depend on which thread ran the job). This
  is now a process-level `RwLock<Option<Raster>>` (`marks/mod.rs:203`), so
  `render_icons_par` is safe for M7 to call.

## 16. Dependency additions

| Crate/feature | Version | Purpose | Note |
|---|---|---|---|
| `notify` | 7.x | cross-platform file events | Mac-testable via real fsevents; Windows via `ReadDirectoryChangesW` |
| `notify-debouncer-full` (NOT `-mini`) | pin at implementation time from crates.io | debounce + rename pairing | `-full` tracks file IDs and folds temp-write→rename into one logical event (§3); `-mini` only time-windows, insufficient |
| `windows` + `Win32_UI_Accessibility` feature | already pinned in `dm-windows/Cargo.toml`, add the feature | `SetWinEventHook` for activity detection (§11) | |
| `tauri` with `features = ["tray-icon"]` | pinned 2.11.5 (existing) | tray icon + menu | `src-tauri/Cargo.toml` currently builds `tauri` with `features = []` — must change |

### Tray icon assets

Windows' notification area has **no `icon_as_template` auto-inversion** the way
macOS does. The tray needs explicit light/dark bitmap pairs at 16/20/24/32px
(multi-DPI), switched at runtime by watching the `AppsUseLightTheme` registry value /
`WM_SETTINGCHANGE`.

## 17. Housekeeping: 10-cap and local metrics

- **10-cap on ③ look-history only** (never on ① the active ledger — see §8.2's table
  for why the fail-safety direction is deliberately opposite):
  - **Dedup before cap**: pushing a new `IconStyleDto` that is field-for-field
    identical to the current head only bumps its timestamp — this is what makes
    "click Apply a few times in a row" not burn through the cap.
  - **Pin exemption (owner ruling 2026-07-12): pin and name are INDEPENDENT.**
    Pinning marks 1–2 entries as exempt from FIFO eviction, so a deliberately-kept
    favourite is never silently evicted by newer experiments. **Naming is a separate,
    unlimited label** — it aids recognition and does NOT affect eviction (so naming a
    look never fails on a cap, and a named-but-unpinned look can still age out). Mental
    model: 置顶=保留, 命名=整理. To keep a named look forever, pin it too.
- **Local metrics** (on-device only, privacy-sensitive — never leaves the machine
  without separate explicit opt-in): automation enable rate; undo rate within the
  first 5 automatic batches; idle CPU/RSS of the resident process; process
  crash-restart rate. These exist to answer "is this feature worth continued
  investment," not for telemetry shipping — treat any future export path as a
  separate decision, not implied by this spec.
