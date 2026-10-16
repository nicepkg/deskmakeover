# ADR-0024 — Shell surfaces: taskbar pins and Start menu shortcuts are styleable

- **Status**: **Proposed** (fork draft, 2026-10-12) — NOT owner-disposed. Written per the
  2026-07-21 community-proposal panel record: fork contributions state touched spec
  sections up front; this ADR IS that statement for the scan/participation/UI delta.
- **Amends**: **ADR-0015 D7** ("The simulated taskbar is generic-but-photoreal scenery…
  never impersonates the user's real taskbar, never gains interactivity"), **spec 06 §4**
  ("The taskbar is scenery: zero interactivity… never impersonates the user's own
  taskbar"), **spec 06 §4/§3** where the decorative `TASKBAR_PINNED` fixture is normative,
  and the review acceptance line (2026-07-09-icon-frontend-panel.md Q7: "taskbar stays
  original-Windows scenery, styling never applies to it").
- **Does NOT touch**: ADR-0021 (arrow overlay — already global), ADR-0022 (consent ladder —
  this feature walks it unchanged), spec 07 (resident stays desktop-only until its own
  consent-gated follow-up).

## Context

1. **The user ask is real and public**: upstream issue #8 (open, enhancement) — restyled
   desktop icons still show original icons on the taskbar. Windows resolves a taskbar
   button's icon from the PINNED shortcut
   (`%APPDATA%\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar\*.lnk`) or the
   running exe's resources — never from the desktop `.lnk` DeskMakeover wrote. Start menu
   entries are a third independent copy of the same shortcut with the same problem.
2. **Scenery was a scope decision, not a platform limit**: D7 bounded the simulated
   taskbar so the mirror stays honest OS chrome. The platform limit it avoided —
   impersonation — is about *fake Windows chrome*, not about *our own tiles on it*. The
   write lever (`.lnk` `IconLocation`) is the exact lever the desktop path already owns,
   with the exact same snapshot/ledger/CAS/restore envelope.
3. **The machinery is already kind-agnostic**: a pinned/start `.lnk` is an ordinary
   `ItemKind::Shortcut` — `apply/shortcut.rs` → `shell_link::set_icon_location`,
   `RestoreAnchor::FileBytes` byte-literal replay, per-item txn CAS. `ScopeRoots` already
   classifies `ProgramData` ancestry as `PrivilegedScope::ProgramData`, so the all-users
   Start menu rides the existing elevated batch (one disclosed UAC). No new writer, no new
   privilege, no new recovery path.
4. **No supported lever exists for exe resources**: rewriting `app.exe` icons breaks
   Authenticode and leaves the reversibility envelope (a restored `.lnk` is byte-replay; a
   rewritten exe is not). Running-window buttons that Windows draws from the exe may keep
   the original while the app runs — an honest, documented limitation, verified on-box
   (`[WINDOWS-VERIFY]`, plan §9), not a broken promise.

## Decision

1. **Three surfaces, one scan**: `IconItemDto` gains `surface: "desktop" | "taskbar" |
   "startMenu"` (+ `parent` for Start-menu folder grouping), bridge schema 10 → 11.
   Roots resolve through `known_folders.rs` known-folder + labeled suffix (pin folder,
   `FOLDERID_Programs`, `FOLDERID_CommonPrograms`); Start menu walks recursively (depth
   cap, reparse skip). Desktop behavior is untouched.
2. **Participation, not look**: a persisted `surfaces: { taskbar, startMenu }` axis rides
   the module state beside `kindPolicy` (default **ON — participate**, owner decision
   2026-10-12 amending the original all-OFF opt-in: system-wide restyling is the app's
   promise, so the default matches kindPolicy's participate-by-default), whitelisted by the
   normalizers on BOTH bridge sides, and never in `ConfigDto` (presets/history stay clean,
   same rule as spec 06 §6 kindPolicy). Cascade: `styleable:false > per-icon override >
   kindPolicy > surface gate`. Opting a surface OUT is explicit and, as before,
   RESTORE-FIRST (a disabled surface's items ride restoreIds on the next apply).
3. **D7 amended, scenery preserved**: the strip's TRAY/clock/Start-flag chrome stays
   decorative and non-interactive; what changes is that the pinned row renders the USER'S
   REAL pins as ordinary tiles (hover try-on, owned-verb menu, exception badges — the §3
   contract), and a canvas-toolbar surface chip opens a Start-panel mock listing real
   Start-menu items. The mirror never impersonates a Windows surface the user does not
   have: it shows OUR tiles over the scenery backdrop, clearly our chrome.
4. **Bake/apply/restore/refresh unchanged**: same chunked bake, same per-item CAS txns,
   same `restart_shell()` + `purge_icon_caches()` refresh (proven taskbar repaint path),
   `icons.restore()` covers all surfaces by ledger walk. ConsentSheet discloses the
   per-surface counts + "shortcut icons only, never the apps' files" (i18n en + zh-hans
   lockstep).
5. **Never**: exe resource rewriting · UWP package-asset edits (spec 06 §6 stands) ·
   per-surface looks (v2 parking lot) · resident auto-format on new surfaces without its
   own consent gate · any new privileged surface beyond the already-classified
   `ProgramData` Start menu.
6. **MSIX entries via owned wrapper `.lnk`** (Task 11, owner directive 2026-10-12):
   Start entries that are AppListEntries with NO file-backed shortcut (42 of the 98
   AppsFolder entries on the dev box — display-name matched against the full recursive
   walk) become styleable through an OWNED wrapper shortcut in the USER Programs root.
   The recipe has two parts:
   - **Launch (the fork ships the box-verified re-target)**: target = the HOST binary
     with args `--activate-aumid <AUMID>` — a `main.rs` fast-path that calls
     `IApplicationActivationManager::ActivateApplication` (CLSID `45BA127D-…`, IID
     `2E941141-7F97-4756-BA1D-9DECDE894A3D`) and exits without booting the GUI.
     REASON: the documented primary recipe (Windows' own Store-app pins) — target
     `%SystemRoot%\explorer.exe` + args `shell:AppsFolder\<AUMID>` — was re-tested
     on-box with a real wrapper (ShellExecute) and FAILED a 4th time to activate the
     app, consistent with the 3 earlier CLI attempts and the pre-existing COM
     registration damage (wallpaper bug `0x80040154` family). The primary stays
     documented here for upstream; the re-target is what makes the gate honest
     ON-BOX (click-to-launch passed via the launcher route 2026-10-13). Host
     manifest is `asInvoker` → no UAC on wrapper click. `parse_appx_aumid`
     reclassifies present wrappers from EITHER recipe (a present wrapper read as an
     ordinary shortcut would compound re-styles).
   - **Identity/merge**: `System.AppUserModel.ID` = the package AUMID (the field
     Windows' merge logic reads: 2 Start rows → 1, the merged row renders the
     WRAPPER's icon), `IconLocation` = our asset, Description =
     `DeskMakeover:appx-wrapper:v1` (distinct from the file-wrapper marker —
     reunification never claims it). The AUMID is set on the ShellLink OBJECT
     (QI `IPropertyStore` → `SetValue` → `Commit`) and published by ONE
     `IPersistFile::Save`, so link + property block land atomically; the writer
     preserves the AUMID across every icon rewrite (a dropped AUMID silently
     un-merges the wrapper back into a duplicate Start row). The merge is
     independent of the launch mechanism.
   The wrapper NEVER touches package assets (the Never-list stands byte-for-byte;
   deletion restores the package icon — owner-verified reversible), always lands in
   the user Programs root (no new elevation, no new privileged surface), and its
   absent-original restore REMOVES the wrapper (`RestoreAnchor::FileAbsent`) —
   verified on-box across two full apply/restore cycles. Package glyph extraction is
   the AppsFolder item's `IShellItemImageFactory::GetImage` — never the wrapper's own
   IconLocation. An entry that renders but cannot launch is never shipped: the
   click-to-launch gate passed on-box via the re-target (2026-10-13).

## Consequences

- Spec 06 §4 + ADR-0015 D7 text updated with the code that enforces this ADR (same-commit
  rule); spec 06 §3 tile contract applies verbatim to the new surfaces, §6 taxonomy
  unchanged (pinned/start entries classify by target like any `.lnk`).
- The pinned row of the mirror is now partly user-data: a user with zero pins sees the
  decorative fixture (honest), a user with pins sees them styled live.
- Issue #8 can be answered with a shipped mechanism instead of a partial accident.
- Upstream merge risk is structural, not technical: per the 2026-07-21 record, expect
  deferral behind write-surface reliability; the fork carries the feature and the PRs
  split spec-scoped (contracts+scan · store+UI · docs) when the owner is ready.
- New failure surface is bounded: pin/start entries are ordinary `.lnk`s in
  ordinary-privilege (user) or already-existing-privileged (ProgramData) scopes; a
  locked pin fails its own per-item txn, never the batch (existing contract, G12).

## Amendment (2026-10-16 — owner ask "quiero que TODOS los iconos de la taskbar se estilicen")

Re-examined against the ask, in this order:

1. **What is stylable IS stylable**: every taskbar pin is a `.lnk` under
   `User Pinned\TaskBar` → `ItemKind::Shortcut` on the `taskbar` surface (this ADR's decision 1),
   plus Store pins via the Task 11 wrapper. There is no third category of pin: Windows builds the
   pinned row exclusively from that folder.
2. **Running-window buttons**: context 4 / decision 5 stand as written — no supported lever
   exists for exe resources (Authenticode + reversibility), and rewriting `.exe` icons stays on
   the Never-list. **On-box observation (2026-10-13, plan §9)**: for a *pinned* app the button
   renders the PIN's icon once the shell repaints, so a running pinned app DOES show the styled
   icon; only *unpinned* running apps (and system-tray glyphs, which each app draws itself from
   its own resources) keep the native icon. The honest coverage is therefore "every pin + every
   pinned-then-running button", never an unpinned running exe's own resource.
3. **Not a bug when it looks native**: the desktop/restart path is `restart_shell()` +
   `purge_icon_caches()` (decision 4) — a shell whose cache predates an apply shows the old
   glyphs until that refresh; pin-cache staleness across restarts was reproduced and fixed by the
   same repaint on-box.

No decision text changes; this records that the ask was checked item-by-item and that the exe
limit is a **platform floor**, not remaining scope.
