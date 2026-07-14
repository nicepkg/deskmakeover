# Spec 09 — Preset Packages (.dmpreset) & the User Preset Library

Normative spec for preset import/export and the user preset library. Owner intent
(2026-07-15): a shareable archive format, community-ecosystem groundwork (future
wallpaper presets, theme packs), offline file sharing, version compatibility, and
a single source of truth between the internally maintained preset structure and
the exported structure. Binding decision record:
`docs/reviews/2026-07-15-icon-preset-io-file-shape-arrow.md`.

## Scope / Non-scope / Assumptions / Dependencies

- **Scope**: the `.dmpreset` container format; manifest schema; two-level
  versioning + the migration chain; the user preset library entity (storage,
  bridge commands, UI surface); import/export semantics; the security pipeline.
- **Non-scope (v1)**: wallpaper preset payloads (format reserves the type; no
  writer/reader ships), package signing / author verification (fields reserved),
  any online marketplace or URL sharing, font assets inside packs (excluded).
- **Assumptions**: Tauri 2 host (Rust owns fs/zip/validation); bridge DTOs
  generated from `dm-contracts`; the icons panel presents the library via the
  风格库 popover (spec 06 §3.14).
- **Dependencies**: `zip` crate (new); `tauri-plugin-dialog` (new, narrow grants:
  open + save only); `lib/icon-look.ts` + `lib/preset-migrations.ts` (new
  frontend single-truth modules).

## 1. Single source of truth (the owner's core requirement)

One TYPE + one VALIDATOR + one SERIALIZER — not one storage medium:

- `lib/icon-look.ts` owns `IconLookPayload { config: ConfigDto, typeOverrides:
  TypeOverrides, kindPolicy?: KindPolicy }` plus `serializeIconLook` /
  `parseIconLook` / `normalizeIconLook` (validation + clamping). The store's
  apply path, history parsing (`parseRecipe` migrates in), built-in preset
  normalization, and package import/export ALL flow through these three
  functions. No second serialization path may exist.
- **Built-in presets stay code constants** (`BASE_CONFIGS` +
  `PRESET_TYPE_OVERRIDES`): compile-time enum safety is the guard the
  MATERIAL_MIGRATION incident proved necessary; a data file rots silently.
- The user preset library's on-disk layout IS the unpacked package layout
  (`data_dir/presets/<entryId>/` mirrors the archive) — library format ==
  package format == internal payload.

## 2. Container format

- Extension **`.dmpreset`**; plain zip inside (rename-to-.zip debuggable).
  Entry names use `/` separators, UTF-8 flag set.
- Layout: `manifest.json` at the root (the ONLY structural JSON — payloads are
  INLINED in it) + `assets/<entryId>/…` for binary assets only (thumbnail now;
  wallpaper images later).
- Manifest schema:

```jsonc
{
  "format": "dmpreset/1",            // container version — gates the whole file
  "generator": "DeskMakeover <ver>",
  "createdAt": "<ISO8601>",
  "entries": [                        // array from day one (theme packs later)
    {
      "id": "<uuid>",                // stable id, dedupe key on import
      "type": "icon",                // "icon" | "wallpaper" (reserved) | …
      "schemaVersion": 1,             // payload version → migration chain
      "meta": { "name": "≤80", "author": "≤80 optional", "description": "≤500",
                "createdAt": "<ISO8601>" },
      "payload": { /* IconLookPayload — config + typeOverrides;
                      kindPolicy only when export opted in (owner #4) */ },
      "thumbnail": "assets/<id>/thumb.png",   // optional
      "assets": [],                            // optional, future
      "integrity": { "sha256": "…" },          // optional, corruption-only
      "signature": null, "publicKeyId": null   // reserved, v1 always null
    }
  ]
}
```

- Thumbnails are gallery hints ONLY (pre-import listing); after import the app
  re-renders the authoritative preview from the recipe on the user's own icons —
  an embedded image is never trusted as proof of the recipe.

## 3. Versioning & migrations

- Two independent levels: `format` (container/manifest shape, rarely bumps) and
  per-entry `schemaVersion` (recipe fields/enums, bumps with enum evolution).
- `lib/preset-migrations.ts` holds ONE ordered, pure, idempotent, unit-tested
  migration chain per payload type. The wallpaper loader's MATERIAL_MIGRATION
  graduates into this chain; import and load-from-disk call the same functions.
- Persisted recipes gain a version field (`v`) via `serializeIconLook`; absent
  `v` = 0 (legacy) and migrates forward on parse. Fixes the pre-2026-07-15 hole:
  styleJson carried no version, so enum renames would mis-render silently.
- Compatibility behavior: new app + old pack → migrate forward, always succeeds.
  Old app + newer payload → unknown NEW fields are ignored; unknown ENUM values
  in known fields reject THAT entry with a clear reason (never render garbage).
  Old app + newer `format` major → hard fail-closed («需更新 DeskMakeover»).
  Multi-entry packs import partially; failures report per-entry. Never
  all-or-nothing, never silent field drops.

## 4. Security pipeline (all in Rust, exportCompare posture)

1. **Zip-slip**: never extract by entry name; read referenced entries into
   memory; reject entry names containing `..`, absolute paths, drive letters or
   NUL; any output path is app-constructed, canonicalized, and asserted under
   the target root.
2. **Bounds**: pack ≤20MB compressed · ≤64 entries · total decompressed ≤100MB ·
   per-entry cap · compression ratio >200:1 rejected · nested archives rejected;
   streaming decompression aborts at the running counter.
3. **Payload**: strict schema; enum WHITELISTS per field (unknown → migrate or
   reject, never passed to the renderer); hex color validation; numeric clamps;
   string caps (name 80 / description 500 / author 80). Clamping lives in
   `normalizeIconLook` — the same function the editor uses.
4. **Images**: magic-byte sniffing (PNG/JPEG/WebP signatures, extension
   ignored), encoded+decoded size caps, ≤16M px, bounded decoder; always
   re-encoded by our codec before any OS-facing use.
5. **Fonts**: not allowed in v1 packs. `fontFamily` remains a name reference;
   missing fonts fall back to the bundled default.
6. **Strings**: rendered as data through React (no dangerouslySetInnerHTML),
   control characters stripped, no auto-linking.
7. **Trust honesty**: v1 packs are unsigned; the UI/docs never imply author
   verification. `integrity.sha256` is corruption detection only.

Import pipeline: ① structural (bounded unzip → manifest → schema → migration;
per-entry failure marking) → ② value clamp (`normalizeIconLook`) → ③ in-memory
preview (decode thumb + re-render from recipe; NOTHING written to disk) →
④ user confirms → atomic write into `data_dir/presets/<id>/`.

## 5. Import / export semantics

- **Import lands in the library, never auto-applies.** Flow: import → entry
  appears in the 风格库 popover's 「我的」 group → hover try-on → user applies
  deliberately (same grammar as built-ins).
- **Id collision → import as copy** (new id, name suffixed 「(导入)」); replace
  is an explicit option only. Same name / different id is allowed (author/date
  disambiguate).
- **Partial success is first-class**: good entries import; each failure reports
  its entry name + human reason.
- **Export source = the CURRENT recipe** (draft), not the selected preset.
  kindPolicy is stripped by default; an export-dialog checkbox 「包含参与策略」
  opts it in (owner decision #4). 「保存为我的风格」 (recipe → library) and
  「导出当前」 (recipe → file) are the same object through two exits.
- Export writes via save dialog; default filename is the sanitized preset name +
  UTC timestamp; `create_new` atomic non-overwrite (export.rs posture).

## 6. Bridge & storage

- Bridge schema **9**; commands (typed via `dm-contracts`, thin DTOs). The
  original single `import` verb split into **read + save** so the ONE validator
  stays in TS and preview-before-write falls out structurally:
  `presets.readPackage (path) → PresetPackageReadDto` (PURE bounded read —
  nothing written; per-entry candidates + inline preview thumbs) ·
  `presets.save (entry: PresetSaveDto, overwrite) → PresetEntryDto` (the ONLY
  library writer; used by import-confirm AND 保存为我的风格; import-as-copy =
  the frontend mints a fresh id) · `presets.list () → PresetEntryDto[]` ·
  `presets.delete (entryId)` · `presets.rename (entryId, name)` ·
  `presets.export (destPath, entries: PresetSaveDto[]) → path`.
  Library thumbnails ride the scoped `dmpreset://<entryId>` protocol (dmicon://
  clone), never JSON; package-read thumbs ride the read result once (bounded,
  PNG-sniffed).
- Import thumbnails are **PNG-only in v1** (the host compiles the png codec
  only; our exporter writes PNG) — a non-PNG thumb drops silently, never sinks
  its entry.
- Library storage: `data_dir/presets/<entryId>/` (manifest-entry JSON + assets),
  NOT localStorage (5MB cap, not backup-friendly, can't hold future wallpaper
  images).
- Frontend: `iconPresets()` merges built-ins (code) + library (via
  `presets.list`), both through `normalizeIconLook`, into one `PresetDto[]`;
  the popover groups 内置/我的.
- Import entry points: the popover toolbar 「导入」 (open dialog) AND file
  drag-drop onto the window (both feed the same `presets.import`).
  `.dmpreset` OS file association is a Windows-installer concern (deferred to
  the ship checklist).
- Dialog capability: `dialog:allow-open` + `dialog:allow-save` only; all fs and
  zip stay in Rust. Frontend never touches raw file bytes.

## 7. Verification

- bun tests: `icon-look` round-trip + clamp table; migration chain (v0 legacy
  styleJson → current; idempotency; unknown enum rejection); manifest schema
  accept/reject fixtures (oversize, zip-slip names, ratio bomb, bad hex, long
  strings).
- Rust tests: bounded unzip (entry caps, ratio, nested archive), zip-slip
  refusal, import→list→delete round-trip on a temp data_dir, atomic export
  non-overwrite.
- E2E (playwright): export current → import it back → appears in 「我的」 →
  try-on renders; corrupted pack shows per-entry failure without aborting good
  entries.
