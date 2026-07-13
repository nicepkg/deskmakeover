# Plan — M3/M4 Windows platform layer, blind-written on Mac

Decision basis: ADR-0019 (+ Amendment 1 Mac-first + COM STA actor discipline), ADR-0020
(resident incremental ledger), ADR-0021 (global arrow overlay), spec 07. Master sequencing:
`docs/plans/2026-07-10-tauri-migration.md` (M3 + M4). This plan is the bite-sized task list
for the **blind-write on Mac** half of M3/M4: Rust behind `cfg(windows)`, kept compiling for
`x86_64-pc-windows-msvc`, with all runtime verification deferred to the owner's Windows box.

## Exit bar (this plan)

- `cargo check --workspace` (Mac) green.
- `cargo test --workspace` (Mac) green — the transaction/ledger machinery is fully unit-tested
  here, including kill-point recovery simulation.
- `cargo check -p dm-windows -p dm-elevated --target x86_64-pc-windows-msvc` green — the
  blind-written COM code type-checks against `windows-rs` for the real target.
- Every `[WINDOWS-VERIFY]` item batches into the owner's Windows session (M1 spikes 1/2/5 +
  M2 "runs on Windows").

## Environmental blocker (surfaced to owner, not mine to fix here)

`cargo check --workspace --target x86_64-pc-windows-msvc` **fails at baseline** (before any of
this work): `rusqlite`'s `bundled` feature compiles `sqlite3.c` with Apple's `cc` for the msvc
target, which cannot find the Windows CRT headers (`stdlib.h`). This is a C cross-compile gap,
not a Rust problem, and it predates M3/M4. Two clean resolutions, both for the owner:
1. On the **Windows box**, the native msvc toolchain compiles `sqlite3.c` fine — the workspace
   gate passes there unchanged. (Mac-first: this is where Windows verification lives anyway.)
2. To make the *Mac* workspace gate green, install `cargo-xwin` (`cargo install cargo-xwin`,
   then `cargo xwin check --workspace`) — it supplies the msvc CRT/SDK headers for `cc-rs`.
   Requires network + ~1GB SDK download; not attempted here (sandbox network returned 403).

Mitigation adopted so this plan's own code is genuinely msvc-verified: **`dm-windows`,
`dm-elevated`, and the shared kernel `dm-domain` are kept free of any C-compiling dependency**
(no `rusqlite`), so `cargo check -p dm-windows -p dm-elevated --target msvc` compiles for real.
The transaction/ledger machinery lives in `dm-operations` (which already carries `rusqlite` for
`settings_store`) but is **pure portable Rust with zero `cfg(windows)` and zero C deps**, so its
msvc-cleanliness is structural (portable Rust that checks on Mac checks for msvc).

## Architecture (ports & adapters)

```
dm-domain   (shared kernel, no I/O, no C deps)      ── Mac + msvc clean
  item / fingerprint / restore-anchor / ports (traits) / errors
        ▲                              ▲
        │                              │
dm-operations (pure txn+ledger)   dm-windows (COM adapters, cfg(windows))
  journal · driver · recovery       apartment · STA actor · shell_link · scan ·
  ledger store · CAS                 apply/* · wallpaper · overlay client · watcher
        ▲                              ▲
        └──────── dm-resident / src-tauri (composition — NOT this plan) ───────┘

dm-elevated (requireAdministrator bin, cfg(windows))  ── overlay-29 verb pair only
```

Rationale: the pure transaction driver (dm-operations) executes real mutations only through
port traits defined in `dm-domain` (`IconApplier`, `DesktopScanner`, …). `dm-windows` implements
those ports with COM; Mac tests implement them with fakes over a virtual desktop. This keeps the
state machine 100% Mac-testable and keeps `dm-windows` off the `rusqlite` path.

Note on crate ownership: this plan touches `dm-domain` (the shared kernel) because `dm-windows`
must be `rusqlite`-free for the isolated msvc check, and `dm-domain` is the natural rusqlite-free
home for shared IDs + port traits ("IDs, plans, typed errors, no I/O"). Additions are isolated in
new module files (`item.rs`, `fingerprint.rs`, `restore.rs`, `ports.rs`, `error.rs`) so a merge
with the icon-core agent is a one-line `lib.rs` reconciliation. `dm-icon-core`, `src/`,
`src-tauri/` are untouched.

## Task list (exact paths · C# oracle source · Mac-testable vs Windows-verify)

### Phase A — plan (this file). Commit first.

### Phase B — transaction + ledger machinery (dm-operations + dm-domain kernel). PURE, Mac-tested.
The crown jewel. C# oracle is behavioral reference only; the C# `JournaledOperationRunner` is an
in-memory rollback stack (NOT crash-durable, per ADR-0019) — we add the durability it lacks.

| File | Harvested from (C#) | Mac-testable |
|------|---------------------|--------------|
| `crates/dm-domain/src/item.rs` | `Core/DesktopItem.cs`, `Core/IconSource.cs` | pure types |
| `crates/dm-domain/src/fingerprint.rs` | preflight `SequenceEqual` in `DesktopIconApplyOperations.cs` | sha256 over bytes/registry-set — unit test |
| `crates/dm-domain/src/restore.rs` | `RestoreMetadataCollector.cs` (per-kind capture) | anchor enum + serde round-trip |
| `crates/dm-domain/src/ports.rs` | writer interfaces (`IShortcutIconWriter`, …) | trait defs only |
| `crates/dm-domain/src/error.rs` | — | typed errors |
| `crates/dm-operations/src/txn/journal.rs` | (new durability; C# had none) | append-only WAL + fsync; `VecJournal`/`TruncatableJournal` fakes |
| `crates/dm-operations/src/txn/driver.rs` | `DesktopBakeService.ApplyAsync`, `JournaledOperationRunner.RunAsync` | prepare→asset→apply(CAS)→verify→commit; LIFO rollback — unit test |
| `crates/dm-operations/src/txn/recovery.rs` | `DesktopBakeService` reversibility invariants (anchor-before-mutation, corrupt-anchor-never-"nothing-applied") | replay + roll-forward/back decision — **kill-point battery** |
| `crates/dm-operations/src/ledger/entry.rs` | ADR-0020 §2 ledger entry; `LookHistoryStore.cs` | `TxnState` machine + entry — unit test |
| `crates/dm-operations/src/ledger/store.rs` | `LookHistoryStore.cs` (cap, corruption-tolerant), `SnapshotStore.cs` (atomic save) | `LedgerStore` trait + JSON impl + in-memory; corruption tolerance — unit test |

Harvested invariants → named Rust tests (each becomes `#[test]`):
- `preflight_conflict_when_content_changed_after_snapshot` ← `Shortcut_operation_aborts_when_target_changed_after_snapshot`.
- `rollback_is_lifo_and_replays_original_content` ← `Journaled_runner_rolls_back_completed_shortcut_operation`.
- `anchor_is_written_before_any_mutation` ← `DesktopBakeService` codex B4 ("declare the restore anchor BEFORE mutating").
- `corrupt_ledger_never_reads_as_nothing_applied` ← `LoadState` corrupt-anchor handling (codex B4) + `LookHistoryStore` corruption tolerance.
- `reapply_restores_to_true_original_first` ← `DesktopBakeService` re-apply-restore-first invariant.
- `killpoint_recovery_leaves_each_item_original_or_target` (parametric over every journal truncation) ← anchor-before-mutation guarantee.
- `cas_reapply_uses_last_applied_fingerprint` ← ADR-0020 §2 per-item compare-and-swap.
- `content_addressed_asset_write_new_then_swap` ← spec 07 §5 ("write new file first, then swap IconLocation").
- `background_hue_pins_existing_seeds` — ledger records pinned seeds (data-level; hue math is icon-core) — assert entry carries seed, no reflow.

### Phase C — dm-domain done above; dm-windows M3 vertical-slice surface. cfg(windows), msvc-check.
One disposable `.lnk`, end to end. All COM; `[WINDOWS-VERIFY]` runtime.

| File | Harvested from (C#) | Status |
|------|---------------------|--------|
| `crates/dm-windows/src/com/apartment.rs` | `Shell/StaThread.cs` | thin unsafe `CoInitializeEx(STA)` RAII — `[WINDOWS-VERIFY]` |
| `crates/dm-windows/src/com/sta_actor.rs` | ADR-0019 Amendment 1 COM discipline; `StaThread.cs` | dedicated STA thread + mpsc + oneshot; COM never crosses `.await`/threads — `[WINDOWS-VERIFY]` (channel plumbing Mac-testable via fake work) |
| `crates/dm-windows/src/shell/known_folders.rs` | `Shell/DesktopPaths.cs` | `SHGetKnownFolderPath` Desktop/PublicDesktop — `[WINDOWS-VERIFY]` |
| `crates/dm-windows/src/shell/shell_link.rs` | `Shell/ShellLinkComInterop.cs`, `ShellLinkShortcut{Reader,IconWriter}.cs` | `IShellLinkW`+`IPersistFile` read/write IconLocation, target, create-new — `[WINDOWS-VERIFY]` |
| `crates/dm-windows/src/shell/scan.rs` | `Shell/DesktopScanner.cs` (FileSystem source) | enumerate + classify → `DesktopItem`; system-attr skip; stable id (sha256[..8]) — pure classify unit-testable on Mac |
| `crates/dm-windows/src/shell/explorer_refresh.rs` | `Shell/ExplorerRefresh.cs` | `SHChangeNotify(SHCNE_ASSOCCHANGED)` — `[WINDOWS-VERIFY]` |
| `crates/dm-windows/src/apply/shortcut.rs` | `Shell/DesktopIconApplyOperations.cs` (ShortcutIconApplyOperation) | `IconApplier` for `.lnk`: SetIconLocation apply + restore-bytes — `[WINDOWS-VERIFY]` |
| `crates/dm-windows/src/apply/mod.rs` | `Shell/DesktopIconOperationFactory.cs` | port dispatch by `ItemKind` — `[WINDOWS-VERIFY]` |

### Phase D — dm-elevated skeleton + overlay client. cfg(windows), msvc-check.

| File | Harvested from (C#) | Status |
|------|---------------------|--------|
| `crates/dm-elevated/src/main.rs` + `Cargo.toml` + `manifest`/`build.rs` | `ElevatedHelper/Program.cs`, `OverlayCommands.cs` | fixed verb whitelist `apply-overlay`/`restore-overlay`; LPE guards (copy ICO into ProgramData, registry never points at caller path; ICONDIR magic check; size cap); one-time `__absent__` state snapshot — arg parsing + guards Mac-unit-testable; registry `[WINDOWS-VERIFY]` |
| `crates/dm-windows/src/overlay.rs` | `Orchestration/OverlayBadgeService` (client side) | ShellExecuteExW `runas` invoke of dm-elevated; UAC-cancel mapping — `[WINDOWS-VERIFY]` |

### Phase E — M4 breadth (as time allows before exit). cfg(windows), msvc-check.

| File | Harvested from (C#) | Status |
|------|---------------------|--------|
| `crates/dm-windows/src/apply/url_shortcut.rs` | `Shell/UrlShortcutIconWriter.cs` | `[InternetShortcut]` INI upsert + restore-bytes — INI upsert pure/Mac-testable |
| `crates/dm-windows/src/apply/folder.rs` | `Shell/FolderIconWriter.cs` | desktop.ini + ReadOnly/Hidden/System attrs + restore — desktop.ini content pure/Mac-testable |
| `crates/dm-windows/src/apply/file_wrapper.rs` | `Shell/RegularFileWrapperWriter.cs` | wrapper `.lnk` + hide original + unwrap — `[WINDOWS-VERIFY]` |
| `crates/dm-windows/src/apply/recyclebin.rs` | `Shell/RecycleBinIconWriter.cs` | per-user DefaultIcon REG_SZ/REG_EXPAND_SZ fidelity + restore — `[WINDOWS-VERIFY]` |
| `crates/dm-windows/src/shell/layout.rs` | `Shell/DesktopLayoutReader.cs`, `FolderViewInterop.cs` | `IFolderView2::GetItemPosition` (+ SysListView32 cross-process fallback) — `[WINDOWS-VERIFY]` |
| `crates/dm-windows/src/wallpaper.rs` | `Shell/DesktopWallpaperInterop.cs` | `IDesktopWallpaper` get-source/apply/restore, multi-monitor state — `[WINDOWS-VERIFY]` |
| `crates/dm-windows/src/watcher.rs` | spec 07 §3/§16 | **DONE (B10, 2026-07-12)** — real `notify`+`notify-debouncer-full` watcher; debounce + event-mapping core Mac-tested & live-verified; only the 3 runtime semantics in item 9 (self-write suppression / restart catch-up / overflow→rescan) are `[WINDOWS-VERIFY]` |

## Cross-crate needs (defined, not implemented here — for the icon-core agent)
- `dm-icon-codec` must expose content-addressed ICO assembly: `write_ico(frames) -> Vec<u8>` and
  a content hash so the ledger can reference `<source-hash>-<style-hash>.ico`. The txn driver
  treats the generated asset as an **opaque `AssetRef { hash, path }`** — it never inspects ICO
  bytes. Until that API lands, tests use a synthetic `AssetRef`.

## Non-goals (this plan)
Restore-first whole-desktop reapply (replaced by per-item CAS) · resident reconciler/consent
ladder (M7) · pixel/ICO internals (icon-core) · `src/`, `src-tauri/`, `dm-icon-core` edits ·
Windows runtime verification (owner's box).

## Delivered (2026-07-11)

Commits (all on `main`): plan `85c784e` · Phase B `ce2fa11` · Phase C `8b078c7` ·
Phase D `e7210ff` · Phase E `67462b7`.

**Gates.** `cargo check --workspace` (Mac) green; `cargo test` for the four owned crates green —
**55 tests** (dm-domain 12, dm-operations 24, dm-windows 12, dm-elevated 7), including the
kill-point recovery battery; `cargo check -p dm-domain -p dm-windows -p dm-elevated --target
x86_64-pc-windows-msvc` green. The full-workspace msvc check stays blocked only by the pre-existing
`libsqlite3-sys` C cross-compile (see the blocker section) — resolved on the Windows box or via
`cargo-xwin`.

**[WINDOWS-VERIFY] checklist for the owner's box** (batches with the M1 spikes 1/2/5):
1. STA actor: `StaExecutor` enters STA, runs a shell-COM job, `CoUninitialize` on the same thread.
2. Known folders: `SHGetKnownFolderPath` returns the user + public desktop roots.
3. Scan: enumerate + classify a real desktop; `IShellLinkW::GetIconLocation` reads shortcut icons.
4. `.lnk` apply/restore: `SetIconLocation`+`Save`; byte-replay restore; CAS conflict on external edit.
5. `.url` / folder / file-wrapper / Recycle-Bin apply + restore (attrs, `desktop.ini`, `DefaultIcon`
   REG_SZ/REG_EXPAND_SZ fidelity).
6. Kill-point battery on a real desktop (process death around each ledger transition → exact restore).
7. Elevated overlay: `dm-elevated apply-overlay/restore-overlay` writes/restores HKLM `Shell Icons\29`;
   `WindowsOverlayControl` `runas` roundtrip; UAC-cancel → `Declined`; requireAdministrator manifest
   embedded at packaging.
8. Wallpaper: `IDesktopWallpaper` capture/set/restore across monitors.
9. **`shell/layout.rs` — DESKTOP GEOMETRY + POSITIONS NOW BLIND-WRITTEN (2026-07-13), no longer an
   empty stub.** Technique A (oracle `FolderViewInterop.cs`) via windows-rs projections:
   `IShellWindows::FindWindowSW(SWC_DESKTOP)` → `IServiceProvider::QueryService(SID_STopLevelBrowser)`
   → `QueryActiveShellView` → `IFolderView2 { ItemCount, Item, GetItemPosition }` +
   `IShellFolder::GetDisplayNameOf`; geometry from `SM_C*SCREEN` + `SPI_GETWORKAREA`. Behind a new
   `DesktopGeometryReader` port; the host matches live slots to scan items BY NAME (oracle rule) and
   degrades per-item to the synthetic grid. Technique B (SysListView32 + ReadProcessMemory) stays
   the documented fallback if A proves unreliable. msvc-clean. **Please [WV]**: (a) `icons.scan`
   positions match the real desktop layout; (b) a headless/denied-QI session degrades to the grid
   without error; (c) side-docked taskbar → taskbar_height reads 0 and the grid tolerates it.
   `source.rs` `WindowsIconSourceExtractor` — **BODY BLIND-WRITTEN (2026-07-13), + codex-review
   hardened + ledger-aware (2026-07-13)** (oracle `legacy/.../ShellIconCanvasSource.cs`): shortcut
   icon-resources via `PrivateExtractIconsW`→`SHDefExtractIconW` (≥MAX_PATH long paths)→
   `ExtractIconExW`, everything else `IShellItemImageFactory::GetImage`
   (ICONONLY|BIGGERSIZEOK, premultiplied→straight), Recycle Bin full+empty pair from the per-user
   CLSID `DefaultIcon` values, HICON via `GetIconInfo` (straight alpha + AND-mask legacy fallback +
   monochrome hbmColor=NULL double-height AND/XOR split). **Ledger-aware re-scan (codex icons2-🔴1/🔴3):**
   the host passes the captured original anchor for an item whose LIVE surface == its last-applied
   fingerprint (our own styled output), so extraction derives the TRUE original from anchor material
   (original `.lnk`/`.url` bytes materialized to a temp sibling → IShellLink icon-location; original
   `desktop.ini` IconResource/IconFile+IconIndex, relative-resolved; captured bin registry values)
   instead of compounding `Style(Style(orig))`. The anchor path is TERMINAL — an unresolvable anchor
   ERRORS → per-item degrade, never reads the styled live surface. A committed-but-unledgered txn's
   journal overlays the ledger; an incomplete txn → provenance-unknown → degraded. msvc-clean; pure
   pixel/parse/ini helpers Mac-tested. **Please [WV] on the box**: (a) `icons.scan`
   returns real 256px pixels for every item kind (shortcut/folder/file/Appx/RecycleBin) and the
   webview renders them over `dmicon://`; (b) the bin advertises TWO sources (full+empty) when the
   per-user DefaultIcon values exist, and degrades to one otherwise; (c) alpha looks right (no
   dark premultiplied halos, no opaque squares from legacy icons); (d) a shortcut whose icon
   resource is unreadable (Electron/Store) still gets the shell image; (e) **APPLY → RE-SCAN → APPLY
   AGAIN does NOT darken/compound** (the ledger-aware original extraction is the whole point — a
   styled icon re-scanned must serve its ORIGINAL, verify the bytes don't double-style); (f) a temp
   `.lnk`/`.url` materialized under a non-ASCII `%TEMP%` username loads via IShellLink; (g) a
   monochrome (16-colour) legacy HICON renders with correct transparency.
   `scan.rs` **NOW INJECTS THE VIRTUAL RECYCLE BIN (2026-07-13, codex icons2-🔴2)** — a
   shell-namespace item the filesystem walk can't see (oracle `DesktopPreviewService.AddRecycleBin`):
   CLSID parsing name `::{645FF040-…}` + localized display name via `IShellItem::GetDisplayName`;
   plus **wrapper reunification** — a `.lnk` this app created (proven by a durable `SetDescription`
   ownership marker `DeskMakeover:file-wrapper:v1`, NOT structural guessing) re-presents its
   Hidden+System original as the RegularFile item, so a wrapped file keeps one identity across
   re-scans. **Please [WV]**: (a) the Recycle Bin appears as a styleable tile with full/empty art;
   (b) a wrapped loose file shows ONCE (as the original, not a duplicate .lnk) and its custom-icon
   shortcut siblings are untouched; (c) a user's own Hidden+System file beside a same-named .lnk is
   NOT mistaken for our wrapper.
   `watcher.rs` — **DONE (B10, 2026-07-12): now a real `notify` + `notify-debouncer-full`
   watcher, NOT a `ReadDirectoryChangesW` stub.** The primitive is cross-platform so the
   debounce + event-mapping core is unit-tested + live-verified on the Mac host (a real FSEvents
   watch reports a newly-created file through the debouncer; `cargo test -p dm-windows watcher`).
   Three runtime semantics still need a real Windows box — **please test + confirm these, they
   cannot be exercised on Mac**:
   - **(a) Self-write suppression** — run an icon apply (which writes `desktop.ini` / swaps ICOs)
     while the watcher is armed and confirm the resident does NOT treat the app's own writes as a
     new-icon event and self-format-loop. (The suppression window is the reconciler's job — M7 T4;
     the watcher only supplies raw hints — but the loop can only be observed on Windows.)
   - **(b) Explorer-restart / sleep-resume catch-up** — kill+restart `explorer.exe` (or
     sleep→resume) while items appear on the desktop, and confirm the resident does a full rescan on
     re-arm so nothing that landed while unwatched is missed (spec 07 §3 catch-up).
   - **(c) Buffer-overflow → full rescan — ⚠️ KNOWN BROKEN with notify 8.2, EXPECT THIS TO FAIL.**
     A codex source-read of `notify-8.2.0/src/windows.rs` found the Windows `ReadDirectoryChangesW`
     backend IGNORES the completion's `bytes_written`: a real buffer overflow (Windows signals it as
     a zero-byte completion, discarding the whole 16 KiB buffer) emits NO event, NO error, and NO
     `Flag::Rescan` — so `WatchEvent::Overflow` never fires and burst-loss recovery silently breaks.
     Mainline notify still ignores the parameter, so no version bump fixes it. **Please confirm the
     symptom** (dump hundreds of files in a burst, observe no `Overflow` + missed items), then pick a
     fix: (i) patch/fork notify's Windows backend to emit `EventKind::Other + Flag::Rescan` on a
     zero-byte completion / `ERROR_NOTIFY_ENUM_DIR`, OR (ii) — recommended, and already written into
     the M7 plan — have the M7 reconciler run a PERIODIC full reconcile as a backstop so a silently
     dropped overflow still heals. `to_watch_event` already maps `need_rescan() → Overflow` correctly
     (it fires on macOS/inotify); the gap is purely the Windows backend's delivery.
   Also note: the file-STABILITY gate (size+mtime settle / open-lock probe / `.lnk` readiness) is
   NOT in the watcher (a blocking probe would stall the debouncer worker) — it belongs in the M7
   reconciler; and the default debounce is now 4s (spec 07), watch is NON-recursive (matches the
   scanner's root-children-only scope), and `DesktopWatch::drop`/`shutdown()` joins the worker.
   Msvc cross-check is green (`cargo check -p dm-windows --target x86_64-pc-windows-msvc`), so the
   Windows backend (ReadDirectoryChangesW under the hood) compiles for the real target already.

**Blind-audit follow-ups (2026-07-11, independent Windows reviewer over `7dc82c1`).** Reading-
detectable hardening scheduled as blind fixes; each carries a live-box verification. Codenamed
by the finding they close:
10. **ProgramData LPE (P1):** from a *standard* account pre-create `C:\ProgramData\DeskMakeover`
    as a directory junction + plant a symlinked `{style}-overlay.ico` and a poisoned
    `overlay-state.txt`; run elevated apply+restore; confirm the helper refuses the reparse point
    and rejects attacker-seeded state (no arbitrary admin write, no HKLM poison).
11. **STA message pump (P2):** drive `IDesktopWallpaper` (and, once wired, IFolderView2) through
    `StaExecutor` while the thread idles on `recv`; confirm no hang; message loop added if any
    interface marshals.
12. **Ledger sharing violation (P2):** hold `ledger.json` open (indexer/AV/backup) during a commit;
    confirm the rename succeeds via `ReplaceFileW`/retry rather than failing the apply.
13. **Journal directory-entry durability (P2):** power-cut right after the first `ItemPrepared`
    append; confirm the journal file + its NTFS directory entry survive for recovery.
14. **Torn-tail recovery (P2):** truncate the journal mid-final-line; confirm recovery tolerates a
    single trailing partial line (post-fix) instead of hard-failing.
15. **`--file` TOCTOU/reparse (P2):** pass a symlinked/oversized `--file`; confirm the size cap
    holds through a single handle.
16. **Reparse-point styling (P3):** desktop folder = junction, file = symlink; confirm styling
    refuses or is fully reversible without mutating the target's attributes.
17. **Long paths (P3):** desktop item with a full path >260 chars — confirm `IPersistFile::Load`,
    `GetFileAttributesW`, and `desktop.ini` writes succeed (verbatim `\\?\`) or skip cleanly.
18. **Non-Unicode item name (P3):** desktop file with an unpaired-surrogate name — confirm it is
    skipped-and-warned, not silently mis-addressed.
19. **Wallpaper solid-colour/slideshow restore (P2):** start from a solid colour and from a
    slideshow, apply then restore; confirm the original returns, not a leftover DeskMakeover image.
20. **Recovery wiring:** confirm `dm_operations::recover()` runs at Tauri startup before any apply
    surface is exposed (today `src-tauri/src/lib.rs` only opens the settings store).

## M7 resident auto-format — decision core DONE on Mac (2026-07-13), platform bodies + tray [WV]

The `dm-resident` decision engine (spec 07) is fully built + Mac-tested (563→570 workspace tests):
`style_resolve` (the frontend resolve ladder ported to Rust), `native_bake` (webview-less
RenderSession bake), the reconciler (classify → queue privileged → gate unstable → flag conflicts →
propose/silently-apply, per-icon activity re-check), `consent` (3-batch silent tier + 60d/partial-
reversion freshness downgrade), `pending_privileged` (§14 queue), `tray_state` (5-state machine +
exhaustive gate), `stability` (two-cycle settle probe), plus `version_switch` (spec §9 projection,
wired as `icons.switchVersion`), the reset toggle-coupling, and the resident precondition guard.
The §14 red line is STRUCTURAL — `dm-resident` has no `dm-elevated`/`OverlayControl` dependency, so
background elevation cannot compile. Incremental applies write ONLY store ① via the shared
`TxnDriver::apply`. **Please [WV] on the box — these are platform bodies the decision core drives
but that cannot run on Mac:**

- **T2 `WindowsActivityMonitor` (NOT YET WRITTEN):** `SetWinEventHook` on DRAGDROP/CAPTURE scoped to
  the desktop `SysListView32` handle + `GetForegroundWindow`/`GetLastInputInfo` fallback. The
  reconciler consumes an `ActivityMonitor` port; the Windows impl is a remaining blind-write. Verify
  a real desktop drag suppresses a pending batch ≥1.5s past release.
- **T8 tray + windowless residency (NOT YET WIRED):** `tauri` `features=["tray-icon"]`, the §12 tray
  menu, `on_window_event(CloseRequested)` → destroy WebView + stay resident, autostart registration
  gated on the toggle. Verify: closing the window keeps the process alive; the tray renders the
  5-state glyph; toggling automation off leaves zero autostart residue.
- **T11 tray bitmaps + theme switch (NOT YET CREATED):** light/dark 16/20/24/32px pairs switched on
  `AppsUseLightTheme`/`WM_SETTINGCHANGE` (Windows has no `icon_as_template`). Verify the glyph swaps
  on a light/dark toggle without a restart.
- **The reconcile LOOP driver (NOT YET WIRED):** a background thread consuming `watch_desktops`
  events + a periodic full reconcile (the notify-8.2 overflow backstop, item 9c). The reconciler
  BODY is done + tested over fakes; the real watcher→reconciler→driver loop only runs on Windows.
  Verify the burst test (temp-write→rename storm → exactly one format per final item) and that a
  mid-batch desktop drag stops the batch and the remainder applies once idle.
- **`icons.switchVersion` on the box:** switch between two saved appearances; new icons since the
  saved version get picked up, vanished ones don't orphan, a hand-edited icon is CAS-skipped.
- **Reset coupling on the box:** a full reset turns the auto-format toggle OFF and lifts the arrow
  overlay (one UAC) as well as clearing ②.

**Cross-crate need for the icon-core agent** `[icon-core-need]`: content-addressed ICO assembly
(`dm-icon-codec::write_ico(frames) -> bytes` + content hash) for the ledger `AssetRef`; the
transparent/refined overlay ICOs the overlay client passes to `dm-elevated`; and the paired
`<asset>-empty.ico` for the Recycle Bin. The txn driver treats every asset as an opaque `AssetRef`.
