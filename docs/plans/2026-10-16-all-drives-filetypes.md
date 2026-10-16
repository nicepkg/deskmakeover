# Plan — Explorer phase 2: all fixed drives + file-type icons (ADR-0004 D7 amendment)

**Date:** 2026-10-16 · **Branch:** `feat/taskbar-startmenu-surfaces` (continues) ·
**Status:** implemented (gates below)

## Owner rulings (this plan's scope — 2026-10-16, three answers)

1. **Folder styling: amplify to EVERY fixed drive** — "al abrir el explorador de archivos **todos**
   los iconos están estilizados", not just `C:\Users\<me>`.
2. **File-type icons: ALL file types EXCEPT images** — the owner explicitly overrules ADR-0004 D7's
   constitutional ban (answered mitigation-by-mitigation in the ADR amendment; images are the
   owner's own carve-out).
3. **Taskbar: "no me alcanza, quiero que todos los iconos se estilicen"** — investigated item by
   item; pins are already styled, running-window exe resources remain a platform floor
   (ADR-0024 amendment appended, no decision changed). The owner's system was in the RESTORED
   state at the time (the uninstall-residue test reverts everything), which reads as "nothing is
   styled" until the next apply.

## Goal

One `explorer` surface whose folder half covers **every fixed drive**, plus a new **file-type
half**: every extension the user actually has files of — except images — restyled through the
per-user `ProgId\DefaultIcon` value, with the same preview pipeline, policy kill switches and
restore posture as every other kind.

## Scope guards (part of the decision, not an afterthought)

- **Protected drive roots are never emitted, never descended**: `Program Files`,
  `Program Files (x86)`, `Windows`, `ProgramData`, `$Recycle.Bin`, `System Volume Information`,
  `Recovery`, `MSOCache`, `PerfLogs`, `Documents and Settings`. The current user cannot write
  `desktop.ini` there without elevation; emitting them would only manufacture per-item apply
  failures. **Elevation for these trees is NOT part of this plan** (a HOT-tier UAC decision:
  writing program-file trees via the elevated helper would widen its `confirm_target`
  confinement, which exists to stop desktop.ini-based CLSID hijacks). Pending, owner to decide.
- **`Users` is container-only**: descended (the profile lives under it), itself never styled
  (ACL-protected) — the walk's posture toward `Users` does not change.
- **Images never restyled** (`PerceivedType = image` OR an explicit extension list), evaluated
  **per ProgId** — one `DefaultIcon` serves every extension of an association, so a ProgId
  reachable from even one image extension is untouchable.
- **Per-file types never restyled**: any `DefaultIcon` carrying `%1`/`%L`/`%l` resolves to *the
  file itself* (`exefile` and friends) — a type-level override would collapse every executable's
  individual icon into one glyph. This is the honest floor of "todos los iconos de los programas":
  per-program identity lives inside each `.exe` (the same platform fact as ADR-0024's
  running-window limit).

## Architecture

| Piece | Status |
|---|---|
| `known_folders::explorer_folder_roots()` — every fixed root, profile fallback | 🆕 |
| Walk: one shared BFS over all roots; protected/container gates; depth 10 / 5000 folders; extension collection | 🆕 edit `shell/scan.rs` |
| Content-addressed `dmicon://` source keys (drop the item-id/slot prefix → dedup by pixels) | 🆕 edit `icon_host/source_cache.rs` |
| Frontend bake memoization (one master per unique source+style+opts, bounded) | 🆕 edit `src/stores/icons.ts` |
| `ItemKind::FileType` + `FileTypeAnchor` + `kind_tag = 9` | 🆕 domain |
| `apply/filetype.rs` — single-value HKCU writer/reader/restore + ProgId validation | 🆕 (mirrors `system.rs`/`drive.rs`) |
| State reader arms (fingerprint / live locations / anchor) → `IconRef` | 🆕 edit |
| Extraction: anchor-driven `original_file_type`; live scan-time `DefaultIcon` resource; System32 fallback for bare names | 🆕 edit `source.rs` |
| Scan enumeration `explorer_file_type_items()` (two-pass: resolve → image-progid gate → value gates) | 🆕 edit `shell/scan.rs` |
| Bucket `File` (`bucket_of` + TS `kindBucket`) — second, independent kill switch | 🆕 edit |
| DTO/TS/bindings: `IconKindDto::FileType` + `IconKind` union | 🆕 edit + `gen:bindings` |
| Budget order: file types sorted LAST (their `.ext` names would otherwise sort first and eat the preview budget) | 🆕 edit |

### Discovery (why not the registry enumerated wholesale)

Extensions are **collected during the folder walk** (every file seen on a walked drive → lowercase
extension, deduped). A type with no file anywhere the user browses is never styled blind, and the
item count stays proportional to real content. Extensions then resolve to their ProgId through the
merged `HKCR\<ext>` view (HKCU `Software\Classes` participates — the same override layer the writer
uses, so the test fixture needs no elevation). **One item per ProgId**: extensions sharing an
association share ONE registry value and must be one item (two writers of one value would
CAS-conflict forever). `path` carries the ProgId; item id namespace `"filetype"`.

### Scaling (why 5000 folders don't blow the pipeline)

- **Scan**: sources are content-addressed WITHOUT the item id — 5000 folders showing the same
  stock glyph cost ONE `SCAN_SOURCE_BUDGET` entry (the cache doc always claimed "one entry per
  unique pixel set"; the id prefix made it false).
- **Bake**: the apply loop memoizes on `(sourceUrl, tileStyleKey, isShortcut, opts)` — equal
  pixels render + PNG-encode once, bounded at 600 entries (a fully-unique item set must not pin
  every master in memory).
- **Order**: the final name sort keys on `(isFileType, name)` so `.ext` labels never crowd
  desktop/taskbar/folder previews out of the budget.

### Two scale defects the first full-size apply found — and their fixes (on-box 2026-10-16)

Both were found by the same ~5000-item apply, each killing the app at ~3 GB:

1. **Compositor decoded sources per ITEM, not per CONTENT** (`src/icon-compositor/icon-renderer.ts`).
   The worker registered one 256² RGBA buffer per item id, so 5042 folders showing one stock glyph
   held 5042 copies (~1.3 GB) and the WebView renderer died. Fixed by keying the decode on the
   **content URL** (`ready`/`seeds` by URL; `itemSource` alias map item→URL; `skey` on the
   render/bake worker messages to look pixels up while the coalescing lane + tile cache stay
   per-item). N items sharing a URL now await ONE decode — regression test
   `tests/compositor-source-alias.test.ts`.
2. **The Rust commit materialized every master twice** (`crates/dm-operations`). `package_masters`
   baked + retained a ~300 KiB laddered ICO per item (`Vec<PackagedItem>` ≈ 1.5 GB) and
   `commit_apply`/`version_switch` then cloned each item's bytes into an up-front
   `Vec<ApplyRequest>` (+1.5 GB) → the observed ~3 GB. Fixed by content-deduping the bake cache and
   sharing `Arc<IcoAsset>` across identical masters, and by building each `ApplyRequest` **inside
   its own per-item transaction** so the bytes live for one item, not for the whole batch.
   Regression test `identical_masters_bake_once_and_share_one_allocation`.

Neither defect changes any pixel or ledger semantics — both are pure identity/ownership fixes on
pipelines that already meant "content-addressed".

## Explicit non-goals

- Elevation for protected trees (pending HOT-tier decision, see guards).
- Per-file / per-program icons (`.exe` resource rewriting — Authenticode + reversibility, Never-list).
- Taskbar running-window buttons (ADR-0024 context 4/decision 5 stand — amended, not changed).
- Images (owner carve-out).

## Status log

- [x] ADR-0004 amendment (D7 broadened + ban lifted with mitigations), ADR-0024 amendment (owner
      ask re-checked item by item), roadmap tick, both prior plans annotated.
- [x] Phase A — all-drives folders: `explorer_folder_roots()`, one shared BFS, protected/`Users`
      gates, depth 10 / 5000 caps, extension collection, content-addressed sources, bake memo.
- [x] Phase B — file types: domain kind + anchor, `apply/filetype.rs` (validation, read_current,
      apply, restore, ProgId-poison refusal), state reader + extraction arms, bucket, DTO/bindings,
      scan enumeration with ProgId-level image gate, `kindBucket` case, budget ordering.
- [x] Tests: walk protected/container gates + extension collection; file-type gates against a
      live HKCU fixture (image / `%1` / dedup / unassociated / `.exe`+`.png`); `explorer_folder_roots`
      shape + profile containment; `filetype.rs` round-trip + poisoned-ProgId refusal; domain anchor
      round-trips; bucket-of-every-kind + serde literals; TS `kindBucket` cases.
- [x] Gates + on-box verification — see the dedicated section below.
- [x] On-box (2026-10-16): the first full-size apply (~5000 items) surfaced the two scale defects
      documented above (compositor per-item decode; Rust per-item master retention + request
      cloning) — both fixed with regression tests; restore + re-apply cycles re-run green below.

### Final gates + on-box verification (2026-10-16)

- **Gates**: `bun test` 674 pass / 0 fail (2 new regression tests included) · `cargo test
  --workspace` 0 failures · `bun x tsc -b` clean · `bun run lint` 0 errors (pre-existing warnings
  only) · `bun run check:bindings` passed · banned-colors clean.
- **Restore cycle**: restore → native — folder `desktop.ini` gone across C:/D:/E:, 0 file-type
  registry values, drives un-styled. One residual row (`Desktop\Trello Installer.exe`) is left as
  found by the trust-first/CAS posture — documented behaviour, self-heals on a later reset.
- **Apply cycle** (full, `files` toggle ON): committed **5213 items in ONE pass, no crash** —
  folders 5000 (C: 424 / D: 462 / E: 4286), file types 40 (all 40 `HKCU\...\DefaultIcon` values
  present), drives 3 (`HKCU\Software\Classes\Applications\explorer.exe\drives\{C,D,E}`), shortcuts +
  Start menu + taskbar 155, Recycle Bin 1. Spot audit: 40/40 folder `desktop.ini` (incl. non-ASCII
  paths) and 40/40 file-type keys on disk. App ends "In sync with the desktop"; peak Rust working
  set **~300 MB** (was ~3 GB and a dead process before the fixes).
- **Known characteristic**: apply wall-time at ~5200 items is dominated by per-item ledger/journal
  persistence (~1.3–4 items/s, tens of minutes). Correct and bounded; a batched ledger flush is a
  possible follow-up, out of this plan's scope.

## Owner follow-up (2026-10-16b): images + `.3mf` stay original, folders lose the white card

Three owner rulings after seeing the applied desktop, implemented as a scope extension of this plan:

1. **Loose image files stay original.** The old behaviour wrapped a desktop `.png`/`.jpg` in a
   styled companion `.lnk` with the original hidden — which the owner saw as "styled images", both
   on the desktop and inside the Desktop folder in Explorer (the wrappers live there). Loose files
   now hit the same never-restyle list as the file-TYPE half: `NEVER_RESTYLE_EXTENSIONS` marks them
   `PreviewOnly` with `Icons_Reason_KeptOriginal`, so they render original AND ride the
   RESTORE-FIRST set — the next apply removes the existing wrappers and un-hides the originals. The
   desktop walk is the only surface that emits loose files (the Explorer walk emits folders only),
   so this covers the whole reported surface.
2. **`.3mf` joined the never-restyle list** (owner: "también los `.3mf`"), and the static image
   list gained the formats that carry no `PerceivedType` (`jxl`, `jp2`, `j2k`, `dds`, `exr`, `hdr`,
   `qoi`, `psb`, `svgz`). `PerceivedType = image` stays the systemic gate.
3. **Folders never wear the plate card.** The `Folder` shape is a plate-filled folder silhouette —
   under the Mono + white-fallback look it reads as a WHITE card ("la parte blanca no debería
   estar"). The Folder bucket's ladder now uses the passthrough `None` shape (the tinted glyph
   alone), the built-in preset ladders were updated, and the icon-recipe migration chain gained
   **v1 → v2**, rewriting a stored `Folder: { shape: 'Folder' }` override to `None` — so an
   already-applied recipe self-heals on the next apply (the saved-style ② load path runs the chain).

Not in scope (still open): the `File` bucket keeps its document-card shape, and 3D/CAD siblings
(`stl`, `obj`, `gcode`, …) are styleable unless the owner adds them to the list.

### Third scale defect: the apply session's byte budget (2026-10-16b)

Re-applying after the restore failed **silently in ~8 s** with
`apply session exceeded its 268435456-byte cumulative budget` — the session's cumulative-base64
cap (`MAX_SESSION_B64_BYTES = 256 MiB`) was sized for "a real desktop of a few hundred icons", and
an all-drives apply carries ~5200 masters (~50 KiB each ⇒ ~260 MiB) even though the DISTINCT data
is a few MiB (every folder shares the stock glyph). The frontend's apply catch turned it into a
generic "couldn't apply" toast, so the desktop silently stayed native.

Fixed in `IconApplySession`: identical payloads are stored ONCE and shared by `Arc`, and the
accounted bytes are the DISTINCT payload bytes — so the cap measures the memory the session
actually holds (audit F4 intact) instead of re-counting the same master per item.
`package_masters`' content cache + `Arc<IcoAsset>` already dedup the post-commit side.

On-box verification (2026-10-16b, after restore → apply): **5200 items committed in one pass**
(Folder 5000 · Shortcut 112 · AppxShortcut 42 · FileType 40 · Drive 3 · RegularFile **2** — the
images left the styleable set entirely), peak RSS **~205 MB** (vs ~3 GB before the earlier fixes),
images visible with no companion wrappers, folders rendering without the plate card, and the
persisted recipe carrying `Folder: { shape: "None" }`.

### Fourth defect (owner-visible): the shell restart accepted a taskbar-less Explorer

After that apply the owner reported **no taskbar and a black desktop**. Diagnosis: `explorer.exe`
was alive but had created neither `Shell_TrayWnd` nor the desktop layer, and `restart_shell`'s
success criterion was the **process** only (`explorer_running`) — an Explorer that came up without
initialising its shell passed the check, so the app reported success over a desktop with no shell.
The wallpaper itself was untouched (the registry + transcoded file were the owner's own, days old):
the black screen was the missing desktop layer.

Fixed in `crates/dm-windows/src/refresh.rs`: the restart now requires **both** the process and the
`Shell_TrayWnd` window (`shell_ready` — pure predicate, unit-tested), clears a shell-less Explorer
before relaunching instead of spawning on top of it, and its error names the taskbar explicitly.
On-box repair for the already-broken state: restart Explorer (kill + launch) → taskbar and desktop
layer back, wallpaper intact.



