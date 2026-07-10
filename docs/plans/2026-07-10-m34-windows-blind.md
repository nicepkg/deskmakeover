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
| `crates/dm-windows/src/watcher.rs` | spec 07 §3 | `ReadDirectoryChangesW` hint skeleton (debounce/stability probe are M7) — `[WINDOWS-VERIFY]` |

## Cross-crate needs (defined, not implemented here — for the icon-core agent)
- `dm-icon-codec` must expose content-addressed ICO assembly: `write_ico(frames) -> Vec<u8>` and
  a content hash so the ledger can reference `<source-hash>-<style-hash>.ico`. The txn driver
  treats the generated asset as an **opaque `AssetRef { hash, path }`** — it never inspects ICO
  bytes. Until that API lands, tests use a synthetic `AssetRef`.

## Non-goals (this plan)
Restore-first whole-desktop reapply (replaced by per-item CAS) · resident reconciler/consent
ladder (M7) · pixel/ICO internals (icon-core) · `src/`, `src-tauri/`, `dm-icon-core` edits ·
Windows runtime verification (owner's box).
