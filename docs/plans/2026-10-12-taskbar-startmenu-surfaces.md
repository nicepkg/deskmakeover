---
plan: taskbar-startmenu-surfaces
created: 2026-10-12
owner-approved: none — fork draft (ADR-0024 status: Proposed, NOT owner-disposed)
status: proposed
touches: dm-domain, dm-contracts, dm-windows, dm-operations, src-tauri, src
---

# Style taskbar pins and Start menu shortcuts (shell surfaces)

## Problem

DeskMakeover styles ONLY the desktop. `WindowsScanner::scan_blocking`
(`dm-windows/src/shell/scan.rs`) reads the two desktop roots + Recycle Bin +
CLSID virtuals and nothing else. The community ask (upstream issue #8, open
enhancement): icons restyled on the desktop still show the ORIGINAL icon on the
taskbar — Windows resolves a taskbar button's icon from the PINNED shortcut
(`%APPDATA%\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar\*.lnk`)
or from the running exe's resources, never from the desktop `.lnk` DeskMakeover
wrote. Start menu entries are a third independent copy of the same shortcut with
the same problem. There is no supported override API for running-window icons
(see §Non-goals); the levers that EXIST are the shortcut `IconLocation` writes we
already own.

Dev-box inventory (2026-10-12, the verification corpus): 3 taskbar pins (Brave,
File Explorer, Windows PowerShell) · 20 user Start menu `.lnk` across 6
subfolders · 72 all-users Start menu `.lnk` · 5 desktop items.

The write machinery does NOT need a new applier: a pinned/start `.lnk` is an
ordinary `ItemKind::Shortcut` and rides `apply/shortcut.rs` →
`shell_link::set_icon_location`, `RestoreAnchor::FileBytes`, the CAS ledger and
the existing elevated batch (a `ProgramData` ancestry already classifies
`PrivilegedScope::ProgramData`). The delta is SCAN + CONTRACT + PARTICIPATION +
MIRROR, plus the ADR-0024 amendment reversing the "taskbar is scenery" clauses.

## Design

### Surface model — one scan, three labeled surfaces

`IconItemDto` gains `surface: SurfaceDto` (`"desktop" | "taskbar" |
"startMenu"`, `#[serde(default)] = desktop` for downgrade tolerance) and
`parent: String` (Start menu subfolder relative path, `""` at root; `""` for
desktop/taskbar). Bridge schema **10 → 11**; regenerate bindings.

Identity stays `ItemId::from_source_path("filesystem", path)` — a pin and a
desktop shortcut of the same app are TWO items with independent fingerprints,
ledger rows and masters. The content-addressed store dedupes the pixels: same
look ⇒ same `.ico` hash ⇒ one asset, N ledger rows. Display-name duplication is
expected (both `Brave`), and is exactly why positions stop being name-matched
across surfaces (§Mirror).

Scan roots, all resolved through `known_folders.rs` (`SHGetKnownFolderPath` +
labeled suffix, same rule as `desktop_roots_labeled` — never a full hardcoded
path):

| Surface | Root | Walk |
|---|---|---|
| desktop | existing | flat (unchanged) |
| taskbar | `FOLDERID_RoamingAppData` + `\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar` | flat |
| startMenu | `FOLDERID_Programs` (user) + `FOLDERID_CommonPrograms` (all-users) | RECURSIVE, depth cap 8, skip reparse points |

The pin folder itself is Hidden+System; the EXISTING per-entry checks
(`is_ignored_entry`, `has_system_attribute`) run on entries INSIDE a root and
stay as-is — no bypass, entries are ordinary `.lnk`s. `desktop.ini` skipping
already applies. Common-Start-menu items classify `PrivilegedScope::ProgramData`
by ancestry → they ride `apply_privileged_batch` (kind `Shortcut`/`AppxShortcut`
covers every Start menu `.lnk`) → ONE UAC for the batch, disclosed by the
existing consent sheet. No new elevation code.

All surfaces ALWAYS scan (mirroring kindPolicy semantics): a disabled surface
renders its tiles as-original and ships no master (RESTORE-FIRST); enabling a
surface never requires a rescan.

### Participation axis — `surfaces`, orthogonal to `kindPolicy`

```ts
surfaces: { desktop: true /* always-on, not user-toggleable */,
            taskbar: boolean,          // default TRUE (owner decision 2026-10-12)
            startMenu: boolean }       // default TRUE (owner decision 2026-10-12)
```

Persisted through the SAME channel as `kindPolicy` (`icons.setLook` →
`IconPersistedDto` → module state), and — same rule as spec 06 §6 — **never in
`ConfigDto`** (surfaces are participation, not look; presets/history must stay
clean). Normalizers whitelist the key on BOTH bridge sides (G11: a closed
whitelist — `src/lib/kind-policy.ts` + the Rust normalizer in
`dm-operations/settings_store.rs`), or the key is silently dropped.

Cascade for a tile's effective config (outermost first):

```
styleable:false  >  per-icon override  >  kindPolicy bucket  >  surface gate
```

The surface gate is the outermost PARTICIPATION gate: a disabled surface's
items never enter `bakeSet`. Bake set (frontend `stores/icons.ts apply()`):

```
items.filter(i => surfacesOn(i.surface)
               && kindParticipates(bucket(i))
               && !effectiveTileConfig(i).showOriginal)
```

…with `restoreIds` for kept-original overrides unchanged.

### Mirror — real pins in the strip, a Start panel view

- **Taskbar strip**: the decorative `TASKBAR_PINNED` fixture entries
  (`taskbar-icons.tsx`, 7 hardcoded glyphs) are REPLACED by the real scanned
  pins (styled previews through the normal Tile renderer — app chrome, NOT the
  OS-mirror palette; tray/clock/Start-flag scenery stays untouched). Pins with
  `styleable:false` render full-fidelity original (honest, never dimmed) with
  no context menu — existing §3.5 rule.
- **Start menu**: a new surface chip in the canvas toolbar (`桌面 | 任务栏 |
  开始菜单` → `Escritorio | Barra de tareas | Menú Inicio`) swaps the canvas to
  a Start-panel mock that lists the REAL Start menu items (grouped by `parent`)
  as ordinary tiles. The switch is OUR chrome (canvas toolbar), so the
  decorative Start flag itself stays non-interactive — D7's "no interactivity"
  is preserved for scenery; only tiles gain behavior.
- **Tile behavior is uniform across surfaces**: hover try-on, press-to-peek,
  owned-verb context menu (保留原样 / 跟随全局 / 单独配色), exception badges,
  undo/redo steps. Position slots: `live_slots` name-matching in
  `icon_host/scan.rs` applies **only to `surface == desktop`** (a desktop `Brave`
  and a pinned `Brave` would otherwise share one slot — G2); taskbar/startMenu
  tiles lay out by strip order / `parent` grouping in the component.

### Apply, refresh, restore — unchanged machinery

- Chunked bake (`applyBakedBegin/Chunk/Commit`), session caps, per-item txns,
  CAS anchors at scan fingerprint, journal + ledger + recovery: **zero changes**.
- Refresh after a mutating commit keeps `restart_shell()` +
  `purge_icon_caches()` (the proven path; `SHChangeNotify` alone does not repaint
  the taskbar) — the existing test
  `every_desktop_mutating_op_restarts_the_shell_but_an_idempotent_reapply_does_not`
  keeps its intent. ConsentSheet copy gains the per-surface counts (i18n
  en + zh-hans in lockstep): "also styles N taskbar pins and M Start menu
  shortcuts — your shortcuts' icons only, never the apps' files".
- `icons.restore()` walks every ledger row regardless of surface — one-click
  full restore covers the new surfaces with no code.

### Non-goals (v1)

- **Exe resource rewriting — never.** Editing `app.exe` icons breaks
  Authenticode signatures and is out of the reversibility envelope (a restored
  `.lnk` is byte-replay; a rewritten exe is not). Running-window buttons whose
  Windows renders from the exe keep the original icon while the app runs —
  documented honestly `[WINDOWS-VERIFY]` (the issue-#8 field report shows pins
  DO repaint for many apps after shell restart).
- **UWP package assets** stay immutable (shortcut-level styling only —
  existing spec 06 §6 rule).
- **Resident/background auto-format** (spec 07) keeps desktop-only; surfaces
  join it in a follow-up behind the same consent ladder.
- **Per-surface looks** (different style per surface) — v2 parking lot.
- Windows' own taskbar chrome (tray icons, running indicators) — scenery forever.

## Ports / DTOs (dm-domain, dm-contracts)

```rust
// dm-contracts/src/icons.rs
#[derive(Default)] #[serde(rename_all = "camelCase")]
pub enum SurfaceDto { #[default] Desktop, Taskbar, StartMenu }   // serde literals tested
// IconItemDto += surface: SurfaceDto (default), parent: String (default "")
// BRIDGE_SCHEMA_VERSION 10 → 11 (src/bridge/types.ts), bindings regenerated
```

No new ports: scan is a `WindowsScanner` internal; apply/restore/scope reuse
every existing port.

## Task list

1. **Docs**: ADR-0024 (amends ADR-0015 D7 · spec 06 §4 · spec 01 canvas clause)
   + this plan — the only commit that lands before code.
2. **dm-contracts**: `SurfaceDto` + `IconItemDto.{surface,parent}` + serde
   defaults + literal tests; schema 11. `bun run gen:bindings` +
   `cargo test -p deskmakeover-desktop --test bindings` green.
3. **dm-windows**: `known_folders.rs` roots (`taskbar_pin_root`,
   `start_menu_roots_labeled`) + `scan.rs` surface-labeled walk (flat pin,
   recursive start menu, depth cap, per-root label); `classify_entry` unchanged;
   unit tests on fs fixtures (pin folder, nested start folders, desktop.ini in
   roots, reparse skip).
4. **src-tauri `icon_host`**: surface-aware assembly — `live_slots` +
   `synthetic_layout` desktop-only; other surfaces emit `surface`/`parent`
   (x/y = 0, frontend-owned layout); persisted `surfaces` state through
   `getPersisted`/`setLook` with the Rust-side normalizer whitelist.
5. **dm-operations**: normalizer whitelist for `surfaces` (settings_store);
   confirm `scope.classify` ancestry hit for CommonPrograms paths (test:
   `C:\ProgramData\Microsoft\Windows\Start Menu\Programs\…` → `ProgramData`);
   NO other logic changes.
6. **Frontend store**: `surfaces` state + setters + normalizers (G11, both
   sides), bakeSet gate, undo steps for surface flips (one step each), draft
   snapshot/resume (surfaces ride the same draft as kindPolicy).
7. **Frontend UI**: participation rows (「参与美化的类型」 grows a SURFACES
   group with live counts), canvas surface chips, strip real-pin tiles, Start
   panel mock view, i18n en + zh-hans lockstep.
8. **Gates**: `bun test` · `bun run lint` · `tsc -b` · `cargo test --workspace`
   · bindings drift · banned-colors allowlist (expect NO change: styled tiles
   ride the shared Tile component, strip files keep only scenery literals —
   confirm, adjust the allowlist only if the test scans rendered subtree).
9. **[WINDOWS-VERIFY] live pass** (owner-supervised, per
   `docs/verification/`): scan shows 3+20(+72) · preview mirrors pins · apply →
   pins repaint after shell restart (running vs cold apps noted honestly) ·
   Start menu entries restyle (user + all-users UAC batch) · hand-edit a pin →
   conflict, untouched · restore → byte-identical originals (FileBytes) ·
   uninstall residue check.
10. **Upstream**: per the 2026-07-21 community-proposal panel record — spec-scoped
    PR splits (contracts+scan · store+UI · docs), each PR states touched spec
    sections + gates run + the 5 owner invariants; adversarial review on
    ledger/CAS/parity. Expect deferral (write-surface reliability is the owner's
    dominant ship risk) — the fork carries the feature until then.
11. **MSIX AppListEntry wrappers** (owner directive 2026-10-12 "solve Store-app
    icons"; recipe owner-verified on-box — see field findings below): the virtual
    Start entries become styleable through OWNED wrapper shortcuts, never package
    mutation:
    - **Scan synthesizes** one item per `FOLDERID_AppsFolder` entry with no
      file-backed `.lnk` in either Programs root: `kind = AppxShortcut` (free — the
      walker never emits it today), `surface = startMenu`, `parent = ""`,
      `path = <user Programs root>\<DisplayName>.lnk` (prospectual: the file does
      not exist yet; id = `ItemId::from_source_path` as usual). The AUMID is
      resolved LIVE — at scan for extraction, at write for creation — by display
      name → AppsFolder lookup. **No new `IconItemDto` field; schema stays 11.**
    - **Extraction — THE SOURCE GLYPH**: the wrapper's source must be the APP's own
      icon, never the wrapper's current `IconLocation` (a probe pointing at an
      arbitrary asset renders that arbitrary glyph — verified on-box: terminal glyph
      on WhatsApp). Rule: an item is `AppxShortcut` iff its shortcut args parse
      `shell:AppsFolder\<AUMID>` (classify.rs gains this parse; synthesized items
      take the AUMID from the AppsFolder display-name match) → the extractor builds
      the AppsFolder `IShellItem` (`SHCreateItemFromParsingName("shell:AppsFolder")`
      — `SHCreateItemFromRelativeName` takes that **IShellItem parent**, NOT a
      `FOLDERID`), resolves the child by AUMID (relative-name first, full
      `shell:AppsFolder\<AUMID>` parsing-name fallback) and renders via
      `IShellItemImageFactory::GetImage` through the shared `factory_icon_image`
      tail (same pixel contract as every shell-item read). File-backed appsFolder
      shortcuts extract by AUMID too — falling back to their `IconLocation` would
      perpetuate whatever it already points at.
    - **AUMID round-trip on write** ✅ *(implemented + pinned by test)*:
      `set_icon_location` (Load → SetIconLocation → Save) reads the AUMID up
      front (by-path read on the real `.lnk` — `SHGetPropertyStoreFromParsingName`
      + `GPS_READWRITE`, best-effort: an ordinary shortcut has none) and
      re-attaches it to the loaded ShellLink OBJECT (QI `IPropertyStore` →
      `SetValue(PKEY_AppUserModel_ID, VT_LPWSTR)` → `Commit`) BEFORE the single
      `Save`, so link + property block publish atomically — without the AUMID the
      running-window lookup falls back to the process exe and the pin goes green
      again. A by-path write is impossible on the save target: it is a
      `.tmp`-named claim-temp with no shell property handler
      (`SHGetPropertyStoreFromParsingName` fails on it — verified on-box, E_FAIL),
      and a two-step save-then-write could strand an un-merged wrapper. Test:
      create wrapper → rewrite icon → AUMID/args/marker unchanged
      (`shell_link::tests::appx_wrapper_round_trips…`), green on-box.
    - **State reader**: `AppxShortcut` with an absent wrapper skips
      `require_exists` (the System-CLSID precedent) → fresh + styleable; when the
      wrapper exists it reads like any `.lnk` (`IconRef`) so re-apply CAS keeps
      chaining. Fingerprint for "absent" = a dedicated sentinel (like the virtual
      recycle-bin state), stable both directions.
    - **Write**: `shell_link::create_appx_shortcut(path, aumid, icon)` — ONE
      `IShellLink` object: target = the HOST binary (resolved from
      `current_exe`), args `--activate-aumid <AUMID>` (the box-verified launcher
      recipe — see the click gate below), icon location, ownership marker
      `DeskMakeover:appx-wrapper:v1` (distinct from the file-wrapper marker —
      reunification never claims it), then `IPropertyStore` (QI from the SAME
      link object) → `SetValue(PKEY_AppUserModel_ID, VT_LPWSTR)` → `Commit`,
      then ONE `IPersistFile::Save` (claim-temp + finalize, crash-safe like every
      writer). The AUMID property is THE mechanism: Windows merges the wrapper
      with the virtual entry (2 rows → 1) and renders the wrapper's icon — the
      merge is independent of the launch mechanism. A present wrapper must
      RECLASSIFY from its args too: `parse_appx_aumid` accepts BOTH wrapper
      recipes (`shell:AppsFolder\…` and `--activate-aumid …`), otherwise a
      re-scan would read it as an ordinary shortcut and a re-style would
      compound (Style(Style(orig))).
    - **[WINDOWS-VERIFY] Launch path — VERIFIED ON BOX 2026-10-13, gate PASSED
      via the re-target**: the icon merge/render does NOT depend on the wrapper's
      target, but clicking DOES. The explorer route was re-tested with a REAL
      wrapper (ShellExecute = double-click equivalent) and FAILED again (WhatsApp
      never activated), confirming the 3 earlier CLI attempts; the pre-existing
      COM registration damage (wallpaper bug `0x80040154` family) stands, so
      `sfc /DISM` remains out of repo scope. The sanctioned fallback is therefore
      IMPLEMENTED as the shipped recipe: the wrapper targets
      `deskmakeover-desktop.exe --activate-aumid <AUMID>`, whose `main.rs`
      fast-path calls `IApplicationActivationManager::ActivateApplication`
      (CLSID `45BA127D-10A8-46EA-8AB7-56EA9078943C`, **IID
      `2E941141-7F97-4756-BA1D-9DECDE894A3D`** — the widely-circulated
      `2E97EF3E-…` is wrong → E_NOINTERFACE) and exits WITHOUT booting the GUI
      (exit codes: 0 activated · 2 missing argument · 3 activation failed).
      Gate evidence: direct CLI → exit 0 + `WhatsApp.Root` running; wrapper click
      → `WhatsApp.Root` running. The explorer.exe + `shell:AppsFolder\…` recipe
      stays the DOCUMENTED PRIMARY in ADR-0024 (Windows' own Store-app pins use
      it); this fork re-targets until the box is repaired. Host manifest is
      `asInvoker` → no UAC on wrapper click.
    - **Ledger / restore**: original = wrapper *Absent* → restore REMOVES the
      wrapper (the `file_wrapper` Absent→`remove_file` precedent) → the package
      icon returns. Zero package mutation; ADR-0024 Never-list intact.
    - **Elevation: never** — wrappers always go in the USER Programs root (the
      Start view merges both roots; verified per-user on-box), so the elevated
      helper gains no new op.
    - **Collisions**: a display name matching a walked `.lnk` stem → skip (the
      classic item covers it); two virtual apps sharing one display name → second
      skipped + scan warn (honest, logged).
    - **Tests**: synthesis (injected AppsFolder fixture), absent→styleable state
      reader, wrapper create + property-store + restore round-trip, ledger anchor
      round-trip, `commit_apply` path for `AppxShortcut` (unelevated), collision
      skips. Docs: ADR-0024 records the sanctioned wrapper recipe; the
      "silently-absent badge" follow-up is superseded by this task.
    - **Status 2026-10-13 — implemented, ALL gates green (incl. on-box ship
      gates)**: all Task 11 code landed (scan args-classification + pure
      synthesis hook; `shell/appsfolder.rs` enumerate/resolve/shell-item/glyph/
      `activate_app`; state-reader `AppxAbsent` fingerprint + `FileAbsent`
      anchor split; `apply_appx` create-or-rewrite + `remove`; extraction
      early-return via the shared `factory_icon_image`; AUMID-preserving writer
      + launcher recipe; `--activate-aumid` CLI fast-path in `main.rs`;
      fake-world virtual model). Gates on-box: `cargo test --workspace` 0
      failures (dm-windows 138, dm-domain 59, dm-operations 230), `bun test`
      670 / banned-colors 146 / `tsc -b` / lint / check:bindings green. The two
      ops orchestration tests live in the Mac-gated `icons/tests.rs`
      (`#[cfg(all(test, not(windows)))]`, repo convention): verified once by
      lifting that gate (272/272, both AppxShortcut flows pass), gate restored
      to a zero-diff state after. **On-box ship gates PASSED (2026-10-13)**:
      enumeration pin (the WhatsApp AUMID literal resolves through StaExecutor);
      scan shows 42 synthesized AppxShortcut entries with zero collisions
      against walked items (only pre-existing WinRAR file↔file duplicate labels
      remain, untouched by this task); full apply → 42 wrappers created, each
      verified (target = host, args = `--activate-aumid <AUMID>`, marker,
      `System.AppUserModel.ID` property, baked asset on disk); click-to-launch
      PASSED via the launcher recipe (wrapper click → `WhatsApp.Root` up,
      Responding); restore REMOVES all 42 wrappers (`RestoreAnchor::FileAbsent`)
      with user-root, ProgramData and desktop originals byte-restored — two
      full apply/restore cycles, both directions clean. Transient "duplicate
      display name" warns during a post-restore watcher rescan are honest
      first-wins dedupe racing the deletions (steady-state scans repeat none).
      After this pass the HKLM WebView2 `AdditionalBrowserArguments` dev value
      is deleted (value + key form) — the CDP bring-up existed only for this
      verification.
    - **Status 2026-10-16 — owner asked "quiero que TODOS los iconos de la
      taskbar se estilicen"; re-checked item by item, no decision changed**
      (ADR-0024 amendment appended): every PIN is already stylable (pins +
      Store pins via the Task 11 wrapper — the pinned row is built ONLY from
      `User Pinned\TaskBar`, there is no third category); for a *pinned* app
      the running button repainted to the styled pin icon after restart
      (on-box, 2026-10-13); what genuinely stays native is only (a) UNPINNED
      running apps' exe resources — no supported lever, Authenticode + the
      reversibility envelope, never-list — and (b) system-tray glyphs each app
      draws from its own resources. A desktop whose icons look native after
      this date means the system is in RESTORED state (the uninstall-residue
      test reverts everything): re-apply before judging coverage.

## [WINDOWS-VERIFY] field findings — live pass 2026-10-12 (owner box)

Verified ✅ (dev loop, `bun run tauri:dev` + `DESKMAKEOVER_ALLOW_UNSIGNED_HELPER=1`):

- Pipeline end-to-end with surfaces ON: scan 103 items / 104 jobs → bake → 6 chunks
  → CAS commit → `restart_shell` → **`commit ok=true`**, including the elevated
  ProgramData batch (user-scope items unelevated; ONE batched UAC approved).
- On-disk rewrites confirmed by hand: taskbar pins (`User Pinned\TaskBar\*.lnk`) and
  Start-menu shortcuts (user + common) carry `IconLocation` →
  `icon-assets/{hash}.ico`; re-applies stay idempotent (re-own via ledger).
- Initial hang root-caused: a fresh clone lacks the gitignored
  `public/dm_icon_wasm.wasm` (only `bun run wasm` writes it) → worker init rejects →
  the FIFO enqueue chain swallowed the rejection → zero `sourceReady` acks → `apply`
  waited forever on the last `bake:ok`.

Honest exceptions measured on-box:

- **MSIX package Start entries are AppListEntries with NO file-backed shortcut** →
  invisible to the file scan by design (Never-list: package-asset edits break the
  package signature). **7 of 97** Start entries here: *Click to Do · Configuración ·
  Copias de seguridad de Windows · Introducción · Reloj · Trello · WhatsApp*. The
  other 90 are `.lnk`-backed and restyle (owner spot-check: only Trello/WhatsApp
  reported unchanged).
- **[BREAKTHROUGH, owner-verified 2026-10-12] MSIX wrapper recipe works**: a probe
  `.lnk` in the user's Programs root with target `explorer.exe shell:AppsFolder\<AUMID>`
  + `System.AppUserModel.ID` = the package AUMID (via `SHGetPropertyStoreFromParsingName`
  with `GPS_READWRITE` → `PKEY_AppUserModel_ID` = `{9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3},5`)
  + `IconLocation` = styled asset makes Windows MERGE it with the virtual AppListEntry
  (Get-StartApps: 2 rows → **1 row** after the AUMID set) and the merged entry renders
  **the wrapper's styled icon** (owner's eyes on the real Start menu, not just the
  enumeration). Deleting the wrapper restores the package icon → reversible, no package
  mutation. → Elevated to product scope as Task 11.
- Running-app pins keep the exe icon while the app runs (issue #8 / ADR-0024
  caveat). **Cold-pin repaint VERIFIED** (owner): Brave's pin had a styled `.lnk`
  on disk but showed the original with the app closed after the first restart
  (stale pin cache), and repainted to the styled icon after the next shell
  restart — no AppUserModelID exception observed on-box.
- **[2026-10-12 night] Probe teardown + launch-route finding**: the AUMID pin
  probe made the running WhatsApp taskbar button render OUR asset (mechanism ✅ —
  but an arbitrary placeholder glyph, terminal-derived: probes bypass extraction,
  the correct glyph needs Task 11's `IShellItemImageFactory` source, see spec).
  Launching then broke: clicking the probe wrapper (target = the standard
  `explorer.exe shell:AppsFolder\…` recipe) spun a busy cursor and never opened
  the app — same failure reproduces for Calculator via that route on this box,
  while direct `IApplicationActivationManager::ActivateApplication` activates
  WhatsApp fine (S_OK, stable process, window up). Probes (wrapper + pin)
  deleted → Start back to 1 row, pins back to the original set; package
  activation healthy. Lesson baked into Task 11: **two live shortcuts carrying
  the same AUMID + a broken launch route = user-visible breakage — probes on the
  real shell stay short-lived, and the wrapper's click-to-launch is a ship
  gate** (spec bullet added).
- Stray `File Explorer (2).lnk` in the pin folder = a manual user copy (19:16),
  not ours — owner to delete or keep.

- **Conflict test PASS (2026-10-12, CDP-driven UI)**: hand-edited `Brave.lnk`
  (`imageres.dll,-109`) → apply → CAS conflict on that pin only, its bytes and
  mtime untouched while the other ~100 items committed. The hand-edit rides the
  restore-job path (`restores=1` → `restore_skipped` → counted as a conflict)
  and also survives a later Restore (trust-first): skipped as a hand-edit until
  the item's fingerprint again equals `original_fingerprint`.
- **Restore test PASS (2026-10-12)**: apply → snapshot ledger → restore →
  **94/94 verifiable FileBytes byte-identical** to `original_anchor.bytes`
  (0 mismatches; the other 3 are files the user deleted long ago — rows healed
  as NotFound, as designed), wrapper `.lnk` sidecars removed and the originals
  un-hidden back to `Archive`, ledger 101 → 1 rows, `txn.log` 284 KB → 0
  (checkpointed), zero `icon-assets` residue except the unmanaged
  `File Explorer (2).lnk` stray.
- **Vanish-residue heal verified**: the 3 wrapped loose files were found
  Hidden|System with their sidecars already gone — a state the apply/restore
  code order cannot produce (external deletion: old crash artifact or AV/user
  cleanup). Recovery path: un-hide → Restore → the "already pristine" arm
  drops the rows → next apply re-wraps cleanly; the re-wrapped→restored cycle
  then came back byte-clean.
- **jpg wrap race (open)**: wrapping `wp14409295-….jpg` failed 4/4 with
  `0x80070005` at the attrs step — Explorer's thumbnail extraction locks the
  target inside the sidecar→hide window, and the rollback (sidecar removal)
  hits the same lock → `rollback-incomplete` txn. The recovery abort hit the
  same denial twice before landing (`icons … finalize degraded` WARNs), which
  **defers a restore until the abort clears** — correct but loud. Arithmetic
  match: the owner's historical `99 committed + restores=3 (+1)` gap is this
  same item. Disposition: marked **keep original** via the per-item override so
  applies run clean (`ok=true`, CTA In-sync); proposed fix (out of v1): retry
  `attrs::set` with backoff on `0x80070005` in `file_wrapper::apply`.
- **3 steady apply conflicts (open observation)**: every CDP apply reported
  `3 conflicts` (the owner's historical runs show none). The UI/event exposes
  the count only, so the ids are unattributed; every directly-inspected item
  is styled and the applies end `ok=true`/In-sync. Needs a conflict-id log
  line or event payload before it can be chased.
- **Test harness note**: WebView2 Runtime 150+ drops `WEBVIEW2_*` env vars for
  elevated hosts (WebView2Feedback #5645) — CDP for the elevated dev app was
  brought up via the HKLM policy `AdditionalBrowserArguments`
  (`--remote-debugging-port=9222`), removed again after the pass. Permanent
  dev-loop suggestion: a non-elevated CDP mode or a scripted test bridge.

Remaining checklist: hand-edit a pin → CAS conflict untouched ✅ (2026-10-12) ·
restore → byte-identical FileBytes on pins/Start ✅ (2026-10-12: 94/94) ·
uninstall residue ✅ MEASURED (2026-10-15) — see the finding below · dev-loop hardening ✅ (worker posts
`ok:false`/`png:null` failure acks instead of swallowing init rejection;
TEMP-DEBUG markers removed; `**/target/**` vite watch ignore landed) ·
Task 11 on-box ship gates ✅ (2026-10-13: enumeration pin · 42 synthesized
AppxShortcut with zero walked collisions · 42 wrappers created — content +
AUMID property verified · click-to-launch PASSED via the implemented
`--activate-aumid` launcher recipe after the explorer route failed a 4th
confirmation · two full apply/restore cycles clean incl. ProgramData · then the
HKLM WebView2 `AdditionalBrowserArguments` dev value + key form deleted).

## Uninstall residue — measured 2026-10-15 (Task 9 check)

**Finding: uninstalling does NOT revert the applied styling.** The Tauri NSIS
uninstaller is configured with NO `installerHooks` (`tauri.conf.json` bundle →
`nsis.installMode: currentUser` only), and the app has no headless restore CLI
(`main.rs` handles only `--activate-aumid`). A per-user uninstall removes the
install dir, the Start-Menu shortcut and the `…\Uninstall\DeskMakeover` entry,
and leaves:

- **Applied styling** (all surfaces): profile-folder `desktop.ini`, rewritten
  `.lnk` `IconLocation`s (desktop / taskbar pins / Start menu), the per-user
  `HKCU\Software\Classes\Applications\Explorer.exe\Drives\<L>\DefaultIcon` keys
  (drive module), the machine `HKLM\…\Explorer\Shell Icons\29` arrow overlay,
  any applied wallpaper, and Task-11 appx wrapper `.lnk`s.
- **App data**: `%APPDATA%\com.xiaominglab.deskmakeover\` (settings.sqlite3,
  `ledger.json`, `icon-assets\`, `look-history.json`, markers) and
  `%LOCALAPPDATA%\com.xiaominglab.deskmakeover\` (WebView2 cache).
- **ProgramData**: `C:\ProgramData\DeskMakeover\transparent-overlay.ico`
  (the elevated overlay copy).

Evidence on the owner box (2026-10-15): the styled state was live
(`HKLM …Shell Icons\29 → C:\ProgramData\DeskMakeover\transparent-overlay.ico,0`,
the three `HKCU …Drives\<L>` keys present, 418 profile `desktop.ini`, app data
33 MB + 179 MB + 27 MB); the only `…\Uninstall` entry present was the OLD 0.1.1
install (whose install dir was already gone). No restore path is wired to any
uninstall.

**Verdict:** a **gap** against the constitutional "one-click reversible with
zero residue" promise — the SAME posture the module ships with today (the fork
adds surfaces but no uninstall hook). It is **not** a regression from the
taskbar/explorer work.

**Fix — IMPLEMENTED 2026-10-15 (headless `--uninstall-restore` + NSIS hook):**
1. `lib.rs::run_uninstall_restore()` reuses the `setup` composition roots
   headlessly: `%APPDATA%\com.xiaominglab.deskmakeover` → `run_startup_recovery` →
   `SettingsStore::open` → `build_icon_host` → `icons.restore()` +
   `icons.restore_overlay()`, then `build_wallpaper_host` → `restore()` per
   monitor, then `TweaksHost::restore()`. `main.rs` runs it on
   `--uninstall-restore` (before booting the GUI) and exits with its status code.
2. `bundle.windows.nsis.installerHooks` → `src-tauri/installer-hooks.nsh`, whose
   `NSIS_HOOK_PREUNINSTALL` runs `"$INSTDIR\deskmakeover-desktop.exe"
   --uninstall-restore` BEFORE files are removed, then drops `$APPDATA\<id>`,
   `$LOCALAPPDATA\<id>` and `$PROGRAMDATA\DeskMakeover`. The HKLM overlay restore
   raises ONE UAC (the uninstaller is per-user) — an explicit, honest trade.
3. **Tested on-box 2026-10-15** (`deskmakeover-desktop.exe --uninstall-restore`
   against a live applied state): exit 0; `HKLM …Shell Icons\29` removed (arrow
   native); the three `HKCU …Drives\<L>` values cleared; 417/423 profile
   `desktop.ini` removed. Gates green (cargo workspace · bun 670 · tsc · lint ·
   bindings). The full install→apply→uninstall→verify loop and the UAC-on-uninstall
   edge ride the owner-supervised live pass (needs a VM / clean install).

**Residual caveat (pre-existing, NOT from this hook):** the ledger-based restore
cannot reach ORPHAN `desktop.ini` that exist on disk with no ledger row. On the
owner box 6 such files remained under `…\Documents\chibi-poser\dist\…` (a
denylisted build tree — `node_modules` had ZERO, so the walk denylist works; the
`dist` copies reference our asset but the parent folder is not styled and they are
absent from `ledger.json`). They were likely styled by a pre-denylist explorer
apply whose rows were later dropped, or copied in by an external build. A
*scan-time rescue sweep* (remove `IconResource=…com.xiaominglab.deskmakeover…`
`desktop.ini` the ledger does not own) would close this — recorded as a follow-up.

## Follow-ups (documented, out of v1)

- Resident auto-format covers the new surfaces (spec 07 §8.2 consent ladder).
- Per-surface style variants (v2 parking lot).
- `keepNewIconsStyled`-equivalent for pins created AFTER apply (the resident
  reconciler's watcher root gains the pin folder).
- Running-app exe-icon honesty: a "this app shows its .exe icon while running"
  badge once [WINDOWS-VERIFY] measures which app classes repaint.
- Common Start Menu per-item UAC-free path if the helper grows per-surface
  batching (today: one batched UAC, already disclosed).
- MSIX package Start entries: ~~non-stylable badge in the mirror~~ **superseded by
  Task 11** (owned wrappers make them styleable; the badge idea applies only to
  entries Task 11 cannot match).
