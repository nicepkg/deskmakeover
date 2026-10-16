# Plan — Explorer module: folder + drive icons (ADR-0004 D7)

**Date:** 2026-10-13 · **Branch:** `feat/taskbar-startmenu-surfaces` (continues) → lands as the
Explorer module · **Status:** ✅ landed — **broadened 2026-10-16** (all fixed drives + file-type
icons) per `2026-10-16-all-drives-filetypes.md`

## Goal

Extend the icon look into **File Explorer**: style the **folder** icons a user sees when
browsing (via `desktop.ini`) and the **drive** icons (via a per-user registry key — resolved to
`HKCU\…\Applications\Explorer.exe\Drives\<L>\DefaultIcon` after the on-box spike, not the machine
`DriveIcons` tree), on a
new `explorer` surface with a per-surface toggle and full restore. This is the "资源管理器
module" already scheduled in `specs/00-roadmap.md` and bounded by **ADR-0004 Decision 7**.

## Scope decisions (owner 2026-10-13)

- **In scope:** folder icons (`desktop.ini`) + drive icons (registry `DriveIcons`, **never**
  `autorun.inf`). Same shape×colour engine, per-surface toggle, full restore.
- **Out of scope (constitutional, ADR-0004 D7):** global file-type association icons
  (ProgId `DefaultIcon`). Permanently banned — association-hijack signature, un-restorable.
  **SUPERSEDED 2026-10-16**: the owner lifted this ban for every file type EXCEPT images (and
  never `%1` per-file types) — see `2026-10-16-all-drives-filetypes.md` and ADR-0004's amendment.
- **Folder coverage:** every folder under the **user profile** (`FOLDERID_Profile`), subject to
  the exclusions below (owner chose "todas bajo tu perfil").
  **BROADENED 2026-10-16** to every fixed-drive root by owner ruling ("al abrir el explorador
  **todos** los iconos"), with protected-root and `Users` container-only guards — same plan doc.

### Folder exclusions (safety + perf — owner informed)

Writing `desktop.ini` into every directory naively is catastrophic: the profile contains the
owner's repo (`target/`, `node_modules/` = tens of thousands of dirs). The explorer walk SKIPS:

- **Hidden + System attribute folders** — the whole `AppData` tree, `.git`-style hidden dirs, and
  anything app-managed. Matches what Explorer shows by default (owner never sees hidden dirs).
- **Reparse points / junctions / symlinks** — no writing `desktop.ini` *through* a link; also
  prevents cycles (OneDrive, redirected Documents). The apply side already refuses reparse
  points (`apply/folder.rs` APPLY-2); the scan mirrors it.
- **Build/VCS/cache trees** — `target`, `node_modules`, `.git`, `.venv`, `__pycache__`, `dist`,
  `build`, `.cache`, `Cargo.lock`-adjacent junk dirs. A denylist by directory name.
- **Depth + count cap** — a max walk depth and a max folder count so a freak tree can never
  stall the scan (mirrors the existing read caps).

## Architecture (what exists vs what's new)

| Piece | Status |
|---|---|
| `desktop.ini` folder apply/restore (`apply/folder.rs`, `FolderIconWriter`) | ✅ exists — reused |
| Folder state read (`state_reader.rs` `SurfaceState::Folder`) + original icon (`source.rs`) | ✅ exists — reused |
| `ItemSurface::Explorer` | 🆕 domain enum variant |
| `ItemKind::Drive` (+ bucket) | 🆕 domain |
| Profile root + visible-drive enumeration (`known_folders.rs`) | 🆕 |
| Explorer scan walk (profile folders + drives) | 🆕 `shell/scan.rs` |
| Drive icon extraction (current `DriveIcons`/default) | 🆕 `source.rs` |
| Drive state fingerprint (`state_reader.rs`) | 🆕 |
| Drive apply/restore via the per-user `Drives\<L>\DefaultIcon` key (HKCU, in-host) | 🆕 `apply/drive.rs` (no elevation — spike-verified) | 🆕 `apply/drive.rs` + elevated helper cmd |
| `explorer` surface toggle (frontend `Surfaces`, serialize, UI) | 🆕 |
| Tests + gates | 🆕 |

## Phases

### Phase A — Folders (user-scope, in-host; the visible value)

1. **Domain:** `ItemSurface::Explorer`; `kindBucket(Folder)` already `Folder`. No `Drive` yet.
2. **Enumerate:** `known_folders.rs::profile_root()`; explorer walk in `shell/scan.rs` —
   recursive, exclusions above, `surface=Explorer`, `kind=Folder`.
3. **Scan wiring:** emit explorer Folder items; classify; extraction reuses folder icon path.
4. **State/apply/restore:** reuse existing folder mechanics (surface only gates participation,
   not the write mechanism). Verify in-host desktop.ini writes for profile folders.
5. **Frontend:** add `explorer` to `Surface`/`Surfaces`, `surfaceParticipates`, serialize,
   `DEFAULT_SURFACES`, toggle UI.

### Phase B — Drives (HKCU per-user key, in-host — **resolved** vs the original HKLM sketch)

6. **Enumerate:** `known_folders.rs::visible_fixed_drives()` (probe A–Z with `GetDriveTypeW`,
   `DRIVE_FIXED` only); display name from the shell (`"SYSTEM (C:)"`), fallback `(C:)`.
7. **Domain:** `ItemKind::Drive` + bucket (**Folder** — an Explorer container the user browses),
   `is_styleable`, serde/TS literal `Drive`.
8. **Extract/state:** original from the CAPTURED anchor (per-user value → icon resource; machine
   `DriveIcons` value likewise; absent → static stock `shell32.dll,8`) — never a live read
   post-apply (the spike proved `SHGetFileInfo` honors our key → compounding). Fingerprint =
   the HKCU value.
9. **Apply/restore:** `apply/drive.rs` writes/removes
   `HKCU\Software\Classes\Applications\Explorer.exe\Drives\<L>\DefaultIcon` — **in-host, no
   elevation** (on-box spike: the `SHGetFileInfo("C:\")` icon changed and reverted cleanly; the
   HKLM `DriveIcons` tree is read as fallback only, never written). Restore removes only the
   `(Default)` value we created (recycle-bin posture), key-existed semantics preserved.
10. **Wires + toggles** for the `explorer` surface covering drives (surface toggle + Folder bucket
    policy are the only gates; apply routes in-host because `C:\` is outside the privileged scope).

### Gates & docs

- `bun test`, `cargo test --workspace`, `bun x tsc -b`, `bun run lint`, `bun run check:bindings`,
  banned-colors test — green before any commit.
- On-box verification: folders under profile restyled (visible + AppData skipped), a fixed drive
  restyled, two clean apply/restore cycles.
- ADR-0004 amendment (Explorer module landed) + roadmap tick.

## Status log

- [x] Investigation + scope locked (owner).
- [x] Phase A folders (domain `ItemSurface::Explorer` + `SurfacesJson.explorer` + `SurfaceDto` +
      frontend surface toggle + profile walk with exclusions + Desktop-overlap dedup). All gates
      green: `cargo test --workspace`, `bun test` 670, `tsc`, `lint`, `check:bindings`, plus a new
      `the_explorer_walk_emits_folders_recurses_and_honors_exclusions` scan test.
- [x] Phase B drives (domain `ItemKind::Drive` + `DriveAnchor`; `apply/drive.rs` per-user
       `HKCU\…\Explorer.exe\Drives\<L>\DefaultIcon` writer, in-host, no elevation; `source.rs`
       `original_drive` from the CAPTURED anchor → static `shell32.dll,8`; `visible_fixed_drives` +
       scan emits `Drive` items; bucket = Folder; DTO/TS/bindings `"Drive"`). New tests:
       `letter_from_path`, `visible_fixed_drives_…`, `the_scan_emits_one_drive_item_per_fixed_root`,
       `bucket_of_covers_every_kind`, plus the serde/TS literal + domain-kind coverage.
- [x] Gates + on-box + docs. All gates green (cargo workspace, bun test 670, tsc, lint,
       check:bindings, TEMP-DEBUG grep clean). On-box: **two clean apply/restore cycles** — drives
       write the 3 per-user keys on apply (C/D/E) and clear them on restore; profile `desktop.ini`
       415 → 0 → 418 → 0. Shell proof: `SHGetFileInfo("C:\")` returns our styled icon at 16px AND
       32px (the navigation-pane size). ADR-0004 amended + roadmap ticked.
- **Known note (not a Phase-B regression):** on this box the per-scan source-preview budget
       (`SCAN_SOURCE_BUDGET` = 24 MiB) is exhausted by the profile's ~560 styleable items, so the
       alphabetical tail (~22 items, e.g. `xtask`/`worlds`/`zh`) lands `Icons_Reason_TooMany`. This
       is the documented bounded-scan cap; the drives add only 3 items.
- **Navigation-pane caveat:** the drive icon renders in the *This PC* content view after apply (the
       owner confirmed the main view styled); the left tree can lag on a window opened before the
       post-apply Explorer restart, a known Windows icon-cache quirk — the shell itself already
       serves our icon at 16px (proven above).
- [x] **2026-10-16 broadening (owner ruling)** — profile-only scope → **every fixed drive root**
       (`explorer_folder_roots()` + one shared BFS so `C:\` can't starve `D:\`; protected roots
       never emitted/descended, `Users` container-only; depth 10 / 5000 folders; extensions
       collected during the walk feed the NEW file-type pass) + **file types except images**
       (`ItemKind::FileType`, `apply/filetype.rs`, two-pass ProgId-level image gate, `%1`
       per-file exclusion, File bucket kill switch). Budget survival: content-addressed
       `dmicon://` keys (drop the item-id prefix) + frontend bake memoization + file types sorted
       LAST so desktop/taskbar/folder previews keep priority. See the 2026-10-16 plan.
