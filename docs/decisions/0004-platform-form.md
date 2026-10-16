# 0004. Platform Form: One Verb, Many Battlefields

**Status:** accepted — amended by [ADR-0023](0023-calm-windows-module.md) (§3 HealthCheck scope
narrowed · §5 gained a guided exception · §6 system-purification timing superseded) and
[ADR-0019](0019-tauri-rust-replatform.md) (the module BEHAVIOUR contract — Probe/Preview/Snapshot/
Apply/Restore/HealthCheck — stands, but its C# implementation types `IBeautifyModule` /
`Operations.Engine` / the C# helper are superseded by Rust crates + `dm-elevated`). See the
amendment at the end of this file.
**Date:** 2026-07-05

## Context

The owner expanded the product vision beyond desktop icon styling: disable Windows
system ads, beautify File Explorer icons, and further experience refinements. A
third expert-panel round (product / UX / visual / Windows engineering) examined how
to grow into a platform without becoming the bundleware ("全家桶") this product
exists in opposition to. The owner resolved the three open conflicts.

## Decision

1. **Identity.** The name 桌面美颜 stays through v1.x. The unifying narrative is
   "让 Windows 回到它本该的样子" — a self-pleasing space ("悦己空间"), never a
   butler/管家. Ad removal is framed as removing visual noise (part of the
   makeover), not as a cleaner capability.
2. **IA: the synthesized one-switch model.** The main screen keeps exactly one
   hero switch. Its default package = cold-tier modules plus disclosed warm-tier
   ones. Once on, modules appear as a "已帮你做的事" checklist rendered in
   module-row grammar (icon + name + status line + per-row exclusion toggle) —
   exclusion, not selection. Hot-tier items live only in an 进阶 area with
   per-item informed consent. New modules are discovered after the first success,
   never as a first-screen menu. A thin icon sidebar may appear only at 4+
   modules. First-screen decisions ≤ 1; outcome-named groups ≤ 3.
3. **Module contract.** Every capability implements `IBeautifyModule`:
   Probe → Preview → CaptureSnapshot (namespaced into a shared snapshot) →
   journaled Apply → idempotent zero-residue Restore → **HealthCheck** (drift
   detection; Windows updates roll changes back, re-apply is a platform
   capability). Metadata: RequiresElevation, Scope (HKCU/HKLM), Reversibility,
   RiskTier, SupportedEditions. Operations.Engine becomes the module host;
   all privileged steps across modules batch into one helper call / one UAC.
   The helper stays a fixed whitelist of named, validated verbs — never a
   generic registry RPC.
4. **Risk tiers.** Cold (pure rendering, user-space) / Warm (toggles Microsoft
   itself exposes; HKCU, no elevation) / Hot (deep registry, resident self-heal).
   **The global one-click never touches the hot tier.**
5. **Module constitution (admission test, all mandatory):** one-click reversible
   with zero residue; local-only, no account, no telemetry; adds no step to the
   primary flow; additive/backstop tone (never "扫描到 X 个问题" anxiety
   language); no destructive or unverifiable pseudo-features (cleaner /
   accelerator / memory optimizer are constitutionally banned). Taste is defined
   by what we refuse.
6. **System-ads module ships in v1.0 as the HKCU one-shot version** (Start menu
   recommendations, lock-screen Spotlight tips, Explorer sync promotions,
   settings/notification suggestions, advertising ID, search highlights — all
   per-user, no elevation, home-edition compatible). Machine-level CloudContent
   policies (HKLM, ignored on Home) are a later advanced option. Resident
   self-heal watching stays opt-in and out of the minimal release. Restore
   distinguishes original value absent/0/1 and never overwrites third-party
   drift (mark, don't clobber).
7. **Explorer icons boundary.** Folder styles (`desktop.ini`) and drive icons
   (registry DriveIcons; never autorun.inf) are in scope for v1.x. **Global
   file-type association icons (ProgId DefaultIcon) are constitutionally banned**
   — the one feature that can break a machine (association hijack signature,
   un-restorable merge states).

## Consequences

- Roadmap re-sliced (specs/00-roadmap.md): v1.0 gains the ads module (HKCU
  one-shot); v1.2 gains folder/drive icon modules; file-type icons removed
  permanently; resident watcher remains v1.1+ opt-in.
- The engine refactor to `Modules.Contracts` + module host is scheduled before
  module #2 lands (v1.1 spine work), not during v0.9.
- The UI keeps the v0.9 single-module layout; the module-row grammar and
  checklist appear when module #2 ships.
- EV signing upgrade becomes more important as modules multiply; whitelist
  submissions (MSRC/360/火绒) become ongoing ops.

## Amendment (2026-07-13 — ADR-0023, calm-Windows panel)

- **§6's "ships in v1.0" timing is SUPERSEDED.** The 2026-07-11 research
  (`docs/references/windows-settings-rust/`) re-tiered §6's item list (lock-screen tips are
  ADVANCED with an empty allowlist; widgets surfaces are GUIDED-only; every direct write
  needs per-environment certification, manifest initially empty), falsifying the "clean HKCU
  one-shot" premise. Release timing is now **capability-gated** (ADR-0023 decision 6): the
  certified 2-4-item write slice rides the first release iff the Windows-VM lab turns green
  during the Windows integration phase; otherwise v1 ships the guided-only face and writes
  follow certification. The capability boundary of record is the research README + ADR-0023.
- **§5 admission test — guided exception (written):** "adds no step to the primary flow"
  binds the hero one-click, which covers automatic-certified switches ONLY; guided surfaces
  are optional user-chosen walkthroughs, never auto-included in the one-click, never counted
  in 「已帮你做的事」.
- **§3 HealthCheck semantics for the calm-settings module = re-detect + re-PROPOSE** (never
  silent auto-replay across a feature-update boundary; in-boundary drift may re-close with an
  honest notice). §3's generic "re-apply is a platform capability" wording stands for pixel
  modules (icons/wallpaper) only.
- The module is named **清爽 / 清爽系统** (ADR-0023 decision 7); 净化/清理/优化 are banned
  copy for it. §2's "已帮你做的事" checklist grammar is refined by ADR-0023 decision 3
  (three-state honest results; verified writes only).

## Amendment (2026-10-13 — D7 Explorer module landed)

- **§7's folder half ships:** every folder under `FOLDERID_Profile` on a new `explorer` surface
  (per-surface toggle, consent-gated like the others), styled via `desktop.ini` in-host (no
  elevation — Warm tier per §4). The walk skips hidden/System-attribute trees (AppData), reparse
  points, and build/VCS/deno trees, with depth + count caps, so a repo-heavy profile can never
  stall the scan or get residue. Restore removes only our `desktop.ini` (byte-identical return).
- **§7's drive half ships as the PER-USER key, not the HKLM one:** `HKCU\Software\Classes\
  Applications\Explorer.exe\Drives\<L>\DefaultIcon` — an on-box spike proved the shell honors this
  key (the `SHGetFileInfo("C:\")` icon changed and reverted), so drives are styled **in-host with
  no elevation**, staying inside §4's Warm tier; the machine `HKLM\…\Explorer\DriveIcons` tree is
  read ONLY as the original-icon fallback and is never written. `autorun.inf` stays untouched;
  fixed drives only (removable/optical/network letters are never keyed).
- **Original-icon recovery for drives never reads the live surface post-apply** (the same spike
  showed the live read WOULD return our own asset → compounding): a captured registry value
  resolves as an icon resource, and an absent value falls back to the static stock glyph
  `shell32.dll,8`. Restore writes HKCU only and removes just the `(Default)` value we created
  (recycle-bin posture: never `delete_subkey_all` over a key another program may share).
- **Bucketing:** `ItemKind::Drive` rides the **Folder** bucket (an Explorer container the user
  browses) — same policy switch, same `explorer`-surface consent; §7's ProgId `DefaultIcon` ban
  is unchanged and the banned association icons were never built.

## Amendment (2026-10-16 — owner ruling: ALL fixed drives · ALL file types except images)

Owner directive, fork scope: "al abrir el explorador de archivos **todos** los iconos están
estilizados". Two parts, one of which **lifts §7's constitutional ban**.

- **§7 folder half BROADENED to every fixed drive root** (`visible_fixed_drives()`, `DRIVE_FIXED`
  only), no longer `FOLDERID_Profile` alone. Mechanics unchanged: in-host `desktop.ini`, Warm
  tier (§4), no elevation. Two guard additions make whole-drive walks honest:
  - **Protected roots are never emitted, nor descended** — `Program Files`, `Program Files (x86)`,
    `ProgramData`, `Windows`, `$Recycle.Bin`, `System Volume Information`, `Recovery`, `MSOCache`,
    `PerfLogs`, `Documents and Settings`: the current user cannot write `desktop.ini` there without
    elevation, so emitting them would only produce doomed per-item apply failures. Styling them
    would mean opening a HOT-tier UAC path (writing program-file trees via the elevated helper);
    **that is NOT part of this amendment** and remains out of scope until explicitly decided.
  - **`Users` is container-only**: descended (the profile lives at `C:\Users\<me>`), never itself
    styled (ACL-protected), same posture as before.
  - Caps re-based on drive roots: depth 10 (the old profile-relative depth 8, re-measured from a
    drive root) and 5000 folders (drive-wide content), denylist + attribute gate unchanged.
  - **Source previews are now content-addressed WITHOUT the item id** (`dmicon://<hash>`), so the
    shared `SCAN_SOURCE_BUDGET` counts *unique pixels once* instead of once per folder — a
    5000-folder walk costs one folder-icon entry, not 5000. The cache doc already promised
    "one entry per unique pixel set"; the item-id prefix made that false. Frontend bakes are
    memoized on the same key for the same reason (one master PNG per unique source+style).
- **§7's file-type ProgId `DefaultIcon` ban is LIFTED for every file type EXCEPT images**
  (owner ruling 2026-10-16, informed — the ban's own rationale is answered below, not ignored).
  New `ItemKind::FileType`, same `explorer` surface toggle + consent count, bucketed **File**
  (so `kindPolicy.File` remains a second, independent kill switch). The type set is not the
  registry enumerated wholesale: it is **the extensions the explorer walk actually SAW on the
  walked drives**, resolved to their ProgId (one item per ProgId; extensions sharing a ProgId
  are one registry target and must be one item, never two writers of one value).

  Answering the original ban rationale, clause by clause:

  1. *"association hijack signature"* — hijack signatures live in the **command verbs**
     (`HKCU\Software\Classes\<ProgId>\shell\open\command`). This feature writes **exactly one
     value**: `HKCU\Software\Classes\<ProgId>\DefaultIcon`, the icon reference only. Verb keys,
     `shell\runas`, `UserChoice`, `OpenWithProgids` are never touched, so no launch path can
     change. Elevation: none (HKCU, §4 Warm tier); HKCR/HKLM never written.
  2. *"un-restorable merge states"* — the anchor is captured BEFORE the write
     (`RestoreAnchor::FileType`: per-user-vs-HKCR effective value + `key_existed`), restore
     removes only the `(Default)` value we created (recycle-bin posture, never
     `delete_subkey_all`), and the styled-residue provenance guard applies unchanged (a value
     pointing into our asset store is never re-captured as "the original"). `run_uninstall_restore`
     walks the same ledger path.
  3. **Exclusions are part of the decision, not an afterthought:**
     - **Images** (owner's explicit carve-out): `PerceivedType = image` **or** membership in an
       explicit list (`png jpg jpe jpeg gif bmp dib ico cur ani tif tiff wmf emf svg webp heic
       heif avif jfif …`) — native icons/previews stay untouched.
     - **Per-file types**: any `DefaultIcon` containing an unexpanded `%1`/`%L`/`%l`/`%v`
       resolves to *the file itself* (`exefile`, `dllfile`, `.scr`, …). A type-level override
       there would collapse EVERY executable's individual icon into ONE glyph — strictly worse
       than native — so those types are skipped. This is also the honest floor of "los iconos
       de todos los programas": per-program identity lives inside each `.exe` (same limit as
       ADR-0024 running-window buttons) and cannot be restyled per-file.
     - No `DefaultIcon` at all, an invalid ProgId shape (backslash/`\`-bearing or control chars
       — defence-in-depth on a key path we interpolate), or an unextractable resource → not
       emitted (nothing to style).
  4. **Refresh**: association icons need `SHChangeNotify(SHCNE_ASSOCCHANGED)`, the notify the
     apply path already fires.
- **§7 Consequences line ("file-type icons removed permanently") is SUPERSEDED** by this
  amendment; the roadmap line follows it.
