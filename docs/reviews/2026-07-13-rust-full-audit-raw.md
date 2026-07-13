# Rust Full-Audit — Raw codex Findings (2026-07-13)

> Verbatim output of the 12-slice carpet codex review (`/multi-ai solo --worker codex`).
> This is the UNVERIFIED raw capture. Dispositions + 二次核实 live in the sibling
> `2026-07-13-rust-full-audit.md`. Slices: A1 dm-domain · A2 dm-contracts · B/C dm-icon-core ·
> D codec+wasm · E txn · F icons/ledger/wallpaper/scope · G resident · H1/H2 dm-windows ·
> I dm-elevated · J tauri+bridge. Calm/清爽 module excluded (concurrent session).

## Pass A1 — dm-domain

[🔴] crates/dm-domain/src/fingerprint.rs:48 — `from_hex` can panic on malformed 64-byte Unicode input instead of returning `None`.
Scenario: Input `"aé"` followed by 61 ASCII bytes has length 64 but slicing `hex[0..2]` ends inside `é`, crashing deserialization.
Fix: Parse `hex.as_bytes()`, require 64 ASCII hex bytes, and decode byte pairs without string indexing.

[🔴] crates/dm-domain/src/ports.rs:42 — The icon apply/restore ports cannot perform the promised atomic compare-and-swap.
Scenario: The caller reads fingerprint A, a user changes the item to B, then `apply` or `restore` overwrites B because neither method receives an expected fingerprint.
Fix: Replace the split check/mutate API with `apply_if_current`/`restore_if_current` operations that verify and mutate under one platform-side lock/handle.

[🔴] crates/dm-domain/src/restore.rs:39 — `WrapperAnchor` permits an existing wrapper with no saved content, yet `has_material` accepts it.
Scenario: `wrapper_existed=true, wrapper_content=None` passes verification; apply overwrites the pre-existing `.lnk`, and restore cannot reconstruct it.
Fix: Model wrapper state as `Absent | Present { content, metadata }` and reject contradictory serialized states.

[🟠] crates/dm-domain/src/ports.rs:19 — Fingerprint reading and restore-anchor capture are separate, non-atomic observations.
Scenario: State A is fingerprinted, an external process writes B, and B is captured as the “original,” producing a journal record whose CAS anchor and restore material describe different states.
Fix: Return fingerprint and restore anchor together from one snapshot operation, deriving the fingerprint from the captured material.

[🟠] crates/dm-domain/src/restore.rs:61 — `RegistryValue` cannot guarantee the claimed byte-exact registry restore.
Scenario: An existing value contains malformed UTF-16, embedded terminators, or a non-string registry type; converting it to `String` loses bytes or makes capture impossible.
Fix: Store the raw registry bytes plus the exact type code, validating string values only when interpreting them.

[🟠] crates/dm-domain/src/restore.rs:51 — `RecycleBinAnchor` admits impossible key/value combinations and still counts them as restore material.
Scenario: `key_existed=false` with one or more saved values deserializes successfully, leaving restore ambiguous between deleting the key and recreating its values.
Fix: Use an enum such as `MissingKey | ExistingKey { values }` and validate it during deserialization.

[🟠] crates/dm-domain/src/restore.rs:70 — File-byte anchors omit filesystem metadata required by the stated exact/zero-residue restore guarantee. (VERIFY: by-design?)
Scenario: Applying to a read-only/hidden shortcut changes attributes or timestamps; replaying only its bytes does not restore the original filesystem state.
Fix: Capture and restore attributes and relevant timestamps alongside bytes, including metadata for a pre-existing regular-file wrapper.

[🟠] crates/dm-domain/src/wallpaper.rs:109 — `WallpaperSnapshot` cannot restore the slideshow it claims to snapshot. (VERIFY: by-design?)
Scenario: A rotating slideshow is active before first apply; the snapshot retains only a Boolean and static frame paths, so restore permanently converts it to a static wallpaper.
Fix: Capture slideshow collection, interval, shuffle/status, and per-monitor association, or explicitly block apply when exact restoration is unavailable.

[🟠] crates/dm-domain/src/source.rs:54 — Icon-source identity uses only path, size, mtime, and index rather than content or file identity.
Scenario: An icon DLL is replaced with different same-size bytes while its timestamp is preserved; the source fingerprint remains unchanged and resident formatting is not refreshed.
Fix: Include the icon file’s stable file ID and content hash or another change token that cannot be preserved across replacement.

[🟠] crates/dm-domain/src/source.rs:64 — Target identity can miss replacements because it lacks target file identity/content.
Scenario: An executable is replaced at the same path with the same version string and timestamp but a different icon; the source fingerprint does not change.
Fix: Add the target’s stable file ID and a content/version-resource fingerprint.

[🟠] crates/dm-domain/src/ports.rs:139 — Missing original material deliberately fails open to re-reading DeskMakeover’s styled output.
Scenario: A ledger proves the live icon is owned but its anchor is stale; fallback extracts the styled icon and subsequent passes compound `Style(Style(original))`.
Fix: Fail closed or mark the item unrecoverable when owned state lacks resolvable original material.

[🟠] crates/dm-domain/src/ports.rs:53 — The “content-addressed” store trusts an arbitrary caller-supplied hash. (VERIFY: by-design?)
Scenario: Two different byte blobs are submitted under one hash because of a caller bug or nondeterministic render; idempotent reuse returns the first blob for the second request.
Fix: Have the store derive/verify the key from bytes, or persist and compare a separate byte digest before reuse.

[🟠] crates/dm-domain/src/ports.rs:55 — Empty-state assets have no explicit identity independent of the primary asset. (VERIFY: by-design?)
Scenario: A Recycle Bin’s empty source changes while its full source/style key remains the same; a relative empty-variant key can reuse stale bytes.
Fix: Pass an independently derived empty hash or address the full/empty pair with one hash covering both byte streams.

[🟠] crates/dm-domain/src/ports.rs:63 — `exists` followed by icon mutation is a TOCTOU-prone asset-validity contract.
Scenario: An asset is deleted or garbage-collected after `exists` succeeds but before `apply`, leaving the desktop registry or shortcut pointing at a missing icon.
Fix: Add transactional pin/unpin semantics or make the applier validate and retain the asset as part of the mutation.

[🟠] crates/dm-domain/src/asset.rs:29 — `ApplyAssets` does not encode the per-kind single-versus-paired invariant.
Scenario: A Recycle Bin can receive `single`, while any other item can receive `paired`; behavior depends on each applier noticing the invalid combination.
Fix: Use distinct `SingleAsset`/`PairedAssets` variants and validate them against `ItemKind` before mutation.

[🟠] crates/dm-domain/src/item.rs:153 — `can_style` ignores `requires_explicit_consent`.
Scenario: A malformed or inconsistent item with `state=Ready` and `requires_explicit_consent=true` is reported styleable, bypassing consent.
Fix: Remove the redundant Boolean or require it to be false in `can_style`.

[🟠] crates/dm-domain/src/ports.rs:69 — `DesktopScanner` cannot return the `SourceIdentity` needed for source-change reconciliation. (VERIFY: by-design?)
Scenario: A target executable or package changes without altering the desktop entry; scanning yields only `DesktopItem`, so the resident engine cannot obtain the richer identity defined in `source.rs`.
Fix: Return a scanned record containing both `DesktopItem` and `SourceIdentity`, or add an explicit source-identity port.

[🟠] crates/dm-domain/src/ports.rs:74 — Desktop positions are keyed only by non-unique display name.
Scenario: `Report.lnk` and `Report.url` both display as “Report”; position matching can assign both items the same slot or swap them.
Fix: Carry a shell PIDL/parsing name or another stable shell identity in each slot.

[🟠] crates/dm-domain/src/item.rs:125 — Windows paths are represented as UTF-8 `String`, which cannot round-trip every valid NT path.
Scenario: A desktop filename contains an unpaired UTF-16 surrogate; scanning must reject or lossy-convert it, potentially making the item unaddressable or colliding with another lossy path.
Fix: Introduce a durable WTF-16/UTF-16 path representation and convert to `Path`/`OsStr` only at platform boundaries.

[🟠] crates/dm-domain/src/item.rs:22 — `ItemId::from_raw` bypasses every documented ID invariant.
Scenario: Empty, non-hex, overlong, or duplicate IDs deserialize into valid `ItemId` values and can alias or overwrite ledger records.
Fix: Make raw construction fallible and enforce exactly 16 lowercase hexadecimal characters, with a separate migration-only escape hatch if required.

[🟠] crates/dm-domain/src/item.rs:31 — Stable IDs retain only 64 bits of SHA-256. (VERIFY: by-design?)
Scenario: A collision causes two desktop items to share one ledger identity, so ownership, restore material, or generated assets may be associated with the wrong target.
Fix: Migrate to the full digest or at least 128 bits while preserving an explicit legacy-ID lookup path.

[🟠] crates/dm-domain/src/item.rs:29 — Rust Unicode uppercasing is not a canonical Windows-path identity operation. (VERIFY: by-design?)
Scenario: Equivalent Windows spellings involving aliases, separators, normalization, or case mappings can derive different IDs, while non-equivalent strings can normalize unpredictably across the oracle/Rust boundary.
Fix: Derive IDs from a canonical Windows path using the exact documented ordinal case-folding algorithm, with parity vectors for non-ASCII paths.

[🟠] crates/dm-domain/src/source.rs:47 — `FileId` is only 64 bits although modern Windows file IDs can be 128 bits. (VERIFY: by-design?)
Scenario: Two ReFS files differing only in the high 64 bits collapse to the same identity, defeating rename-versus-replace detection.
Fix: Store all 128 file-ID bits plus the volume identity.

[🟠] crates/dm-domain/src/restore.rs:69 — Restore-anchor shape is not coupled to `ItemKind`.
Scenario: A corrupted journal pairs a folder target with `FileBytes` or a shortcut with `RecycleBin`; the type system permits restore to dispatch incompatible material.
Fix: Validate `(ItemKind, RestoreAnchor)` at construction/deserialization or use kind-specific target/anchor request variants.

[🟠] crates/dm-domain/src/restore.rs:83 — Styleable `System` items have no explicit restore-anchor variant. (VERIFY: by-design?)
Scenario: System CLSID registry state must be squeezed into a Recycle-Bin-specific shape or cannot be represented, making dispatch and exact restoration ambiguous.
Fix: Add a generic registry-icon anchor carrying key identity and exact values, usable by both System and Recycle Bin targets.

[🟠] crates/dm-domain/src/error.rs:10 — `PortError` lacks permission/elevation, invalid-data, cancellation, and transient-busy categories.
Scenario: Access denied, corrupt shell data, UAC cancellation, and retryable sharing violations collapse into `Io`/`Com`, preventing correct escalation, fail-closed handling, or retry policy.
Fix: Add behaviorally distinct variants while retaining raw platform diagnostics as structured context.

[🟠] crates/dm-domain/src/ports.rs:197 — Wallpaper writes expose no CAS or all-monitor transactional operation. (VERIFY: by-design?)
Scenario: A user changes wallpaper after capture, or the process crashes after updating one monitor; `set` overwrites the external edit or leaves a partially applied desktop.
Fix: Add expected-state tokens and a batch apply operation with durable per-monitor progress/rollback reporting.

[🟠] crates/dm-domain/src/wallpaper.rs:128 — `DecodedImage` permits zero dimensions and PNG/dimension mismatches.
Scenario: A decoder returns width zero or dimensions unrelated to `png`; downstream crop arithmetic can divide by zero or serve inconsistent preview data.
Fix: Make construction fallible and validate nonzero dimensions plus decoded PNG metadata.

[🟡] crates/dm-domain/src/asset.rs:13 — `AssetRef` allows unrelated or unsafe hash/path pairs while deriving structural equality over both.
Scenario: The same content hash at two materialization paths compares unequal, while an arbitrary path can be paired with a trusted-looking hash and passed to an applier.
Fix: Use validated hash/path newtypes and define identity by the immutable content key, keeping location separate.

[🟡] crates/dm-domain/src/asset.rs:51 — `OwnedFields` admits the meaningless all-false ownership state.
Scenario: `OwnedFields { icon_location: false }` can be persisted after an icon apply, causing later CAS/restore logic to treat the mutation as unowned.
Fix: Represent v1 ownership as a non-empty enum/bitflags type and reject empty ownership.

[🟡] crates/dm-domain/src/item.rs:89 — `ItemState`, `ItemKind`, consent, and status fields permit contradictory combinations.
Scenario: `ItemKind::Unsupported` can be `Ready`, a healthy item can carry `Error`, or `RequiresConsent` can have a false consent flag, forcing callers to invent precedence rules.
Fix: Replace independent fields with a discriminated readiness enum carrying state-specific data.

[🟡] crates/dm-domain/src/wallpaper.rs:16 — `MonitorRect` accepts zero or negative dimensions.
Scenario: A corrupt adapter result such as `w=-1920, h=1080` is classified as portrait and can feed invalid layout arithmetic.
Fix: Use validated positive dimensions, keeping signed coordinates only for `x` and `y`.

[🟡] crates/dm-domain/src/wallpaper.rs:59 — `MonitorInfo` permits contradictory source-readability states.
Scenario: `source_path=Some(...)` with `has_readable_source=false` leaves callers unable to decide whether decoding is valid or the wallpaper is dynamic.
Fix: Replace the Boolean/option pair with `Readable(Option<Path>) | DynamicOrUnreadable`.

[🟡] crates/dm-domain/src/wallpaper.rs:77 — Derived equality treats monitor enumeration order as semantic topology.
Scenario: Windows returns the same keyed monitors in a different order, making equal physical topology compare unequal and triggering needless reconcile/UI churn.
Fix: Canonicalize by `monitor_id` or model monitors as a keyed map before comparison.

[🟡] crates/dm-domain/src/wallpaper.rs:109 — Snapshot monitor entries permit duplicate IDs and order-dependent restoration.
Scenario: A malformed snapshot lists one monitor twice with different images; restore behavior becomes last-write-wins and is not an exact inverse.
Fix: Validate unique monitor IDs at snapshot construction/deserialization.

[🟡] crates/dm-domain/src/ports.rs:161 — Overlay failures have two incompatible representations and `Failed` carries no cause.
Scenario: Implementations can return either `Ok(Failed)` or `Err(PortError)`, leaving callers unable to apply one reliable retry/reporting policy.
Fix: Reserve `Result` for failures and keep `OverlayOutcome` for successful `Applied`/`Declined` outcomes.

[🟡] crates/dm-domain/src/restore.rs:23 — Restore-anchor deserialization has no size bound.
Scenario: A corrupt or attacker-controlled journal supplies a huge base64 string, causing proportional allocation before recovery can reject it.
Fix: Enforce per-anchor encoded and decoded byte limits during deserialization.

[🔵] crates/dm-domain/src/source.rs:119 — Every source fingerprint builds many heap-allocated byte vectors and a second references vector.
Scenario: Resident rescans allocate repeatedly per optional field for every desktop item despite SHA-256 supporting streaming updates.
Fix: Stream framed fields directly into one hasher with small stack buffers and no intermediate `Vec<Vec<u8>>`.

CLEAN: crates/dm-domain/src/lib.rs

## Pass A2 — dm-contracts

[🟡] crates/dm-contracts/src/icons.rs:131 — `f64` permits non-finite timestamps, which serialize as `null` despite the frontend contract requiring `number`.
Scenario: A `LookVersionDto` containing `NaN` or infinity serializes with `"createdAt":null`, then fails Rust round-trip and reaches `formatTime` with an invalid frontend value.
Fix: Use an integer/finite-number newtype with an explicit Specta `number` representation and reject non-finite values.

[🟡] crates/dm-contracts/src/settings.rs:49 — `SettingsPatch` accepts explicit `null`, but the hand-authored frontend `Partial<SettingsDto>` permits only absent or concrete values.
Scenario: Generated bindings expose nullable properties while `src/bridge/types.ts` rejects `{theme:null}` at compile time, leaving two incompatible definitions of the same patch contract.
Fix: Align the frontend type with generated nullable fields, or make the Rust patch reject `null` and document absent-only semantics.

[🔵] crates/dm-contracts/src/icons.rs:67 — Rust and frontend define incompatible shapes under the same `IconItemDto` name. (VERIFY: by-design?)
Scenario: Rust omits required frontend `overrideMode`/`overrideTint`; the current Tauri scan adapter injects them, but any direct/generated consumer receives a structurally incompatible DTO.
Fix: Give the raw wire shape a distinct name such as `RawIconItemDto`, then explicitly map it into frontend `IconItemDto`.

[⚪] crates/dm-contracts/src/icons.rs:55 — `OverrideModeDto` is legacy dead contract code.
Scenario: It is only re-exported and tested; no DTO field, command input, generated binding, host mapping, or frontend consumer uses it after `setLook` left the bridge.
Fix: Remove it and its tests/re-export, or attach it to a real wire field if retention is intended.

[⚪] crates/dm-contracts/src/lib.rs:4 — Crate-level contract documentation is stale, still describing an M2/schema-4 partial migration.
Scenario: The crate now exports settings, diagnostics, wallpaper, and schema-7 icon contracts, contradicting the claim that later bridge schemas remain unmigrated.
Fix: Update the module documentation to describe the current thin wallpaper/icon contract boundary.

CLEAN: crates/dm-contracts/src/common.rs, crates/dm-contracts/src/style.rs, crates/dm-contracts/src/wallpaper.rs.

## Pass B — dm-icon-core pipeline

[🔴] crates/dm-icon-core/src/source_facts.rs:76 — Background-fact extraction assumes square sources and panics on valid wide PNGs.
Scenario: Native ingress accepts arbitrary dimensions; an opaque 8×4 raster makes the ring analyzer use width for the y-axis and read row 7, while tall rasters sample the wrong ring.
Fix: Make background analysis width/height-aware or explicitly normalize sources before registration, preserving square-oracle goldens.

[🟠] crates/dm-icon-core/src/render_session.rs:87 — Long-lived sessions retain removed sources and every superseded profile/fact indefinitely.
Scenario: Repeated rescans replace or delete IDs; removed IDs retain their raster, while replaced IDs leave old content-digest profiles and segmentation facts resident until worker shutdown.
Fix: Add unregister/reset APIs and byte-capped eviction or reference-counted cleanup of unreferenced content keys.

[🟠] crates/dm-icon-core/src/output_cache.rs:168 — The 64 MiB cap counts only pixel bytes, permitting metadata-driven OOM.
Scenario: Distinct 1×1 entries allow 16,777,216 hash-map records before the nominal cap; zero-length direct inserts never trigger eviction.
Fix: Charge key/map/entry overhead, enforce an entry-count limit, and reject empty or malformed tiles.

[🟠] crates/dm-icon-core/src/hue_spread.rs:100 — The ±π seam relaxation leaves a feasible four-icon cluster only about 3° apart.
Scenario: Distinct art keys seeded `#70F8E0,#70F8E0,#58E8D0,#58E8D0` produce roughly 3° gaps although four 12° gaps fit within every ±18° rotation cap; the frozen TS oracle shares the defect.
Fix: Unwrap at the largest circular gap before relaxation, correcting Rust, the frozen oracle, and goldens together.

[🟠] crates/dm-icon-core/src/hue_spread.rs:47 — Native orchestration never calls the Rust hue-spread port, so resident/version-switch renders omit required allocations (parity-retained: verify vs oracle).
Scenario: Production native callers pass `field_seed=None`, while the frontend uses a separate TS copy; colliding icons therefore get unspread native plates and cannot allocate against pinned existing seeds.
Fix: Invoke this port during native seed collection and feed its results through `RenderOpts.field_seed`; retire the live TS duplicate only after the oracle gate.

[🟠] crates/dm-icon-core/src/output_cache.rs:249 — Keying and rendering can observe different arrow snapshots and cache pixels under the wrong digest. (VERIFY: by-design?)
Scenario: One thread hashes arrow A, another installs B, then the first renders B and stores it under A’s key; production currently relies on an unenforced boot-once convention.
Fix: Pass one immutable arrow snapshot through keying and rendering or enforce one-time initialization.

[🟠] crates/dm-icon-core/src/batch.rs:20 — A batch does not actually freeze `NATIVE_ARROW`, making its outputs scheduling-dependent. (VERIFY: by-design?)
Scenario: A concurrent arrow update lets separate jobs in one batch render a mixture of old and new badges despite the documented frozen-input invariant.
Fix: Include an immutable arrow in the batch render context or hold one read snapshot for the complete batch.

[🟡] crates/dm-icon-core/src/raster.rs:10 — `Raster` exposes unchecked invariants, and its constructor uses overflowing size arithmetic. (VERIFY: by-design?)
Scenario: A short public `data` vector causes widespread indexing panics; `Raster::new(usize::MAX / 2 + 1, 2)` panics in debug or wraps to an invalid raster in release.
Fix: Make fields private and provide checked, fallible constructors validating `width × height × 4 == data.len()`.

[🟡] crates/dm-icon-core/src/render_session.rs:152 — Public native callers can render size zero and hit a downstream assertion. (VERIFY: by-design?)
Scenario: A registered source and look followed by `render(..., size=0, ...)` panics; only the WASM adapter rejects zero.
Fix: Validate size at the session boundary and return a typed error.

[🟡] crates/dm-icon-core/src/sampling.rs:15 — Bilinear sampling panics on a zero-width or zero-height raster.
Scenario: `sample_bilinear_at(&Raster::new(0, 0), 0.0, 0.0)` underflows at `width - 1`; zero-dimensional rasters are otherwise supported by the core.
Fix: Return transparent before subtracting when either dimension is zero.

[🟡] crates/dm-icon-core/src/sampling.rs:222 — The width-only downscale shortcut violates the promised square target for non-square inputs. (VERIFY: by-design?)
Scenario: Downscaling a valid 256×512 raster to 256 returns the unchanged 256×512 raster instead of a 256² frame; the frozen TS function has the same assumption.
Fix: Define mixed-axis behavior explicitly and only short-circuit when both dimensions satisfy that contract.

[🟡] crates/dm-icon-core/src/batch.rs:85 — Transient pool construction panics on thread-creation failure or excessive thread counts.
Scenario: Resource exhaustion or `threads=usize::MAX` produces `ThreadPoolBuildError`, and `.expect("rayon pool")` unwinds the caller.
Fix: Bound thread counts and return `Result`, with an existing-pool or serial fallback.

[🟡] crates/dm-icon-core/src/mono.rs:18 — The thread-local tint LUT cache grows for the thread lifetime rather than the session lifetime.
Scenario: Continuous color-picker values permanently accumulate 768-byte ramps in every WASM/Rayon worker; destroying a session frees none of them.
Fix: Use a small bounded LRU or let each session/current tint own its LUT.

[🟡] crates/dm-icon-core/src/color.rs:275 — Reversed or NaN chroma windows panic instead of matching the oracle’s min/max semantics. (VERIFY: by-design?)
Scenario: `Some((0.12, 0.09))` panics in `f64::clamp`, whereas the frozen TS export returns through nested `Math.min`/`Math.max`.
Fix: Validate the window or reproduce the oracle’s ordered min/max and NaN policy.

[🟡] crates/dm-icon-core/src/hue_spread.rs:54 — Rust Unicode ordering differs from JavaScript UTF-16 ordering, changing deterministic rotations. (VERIFY: by-design?)
Scenario: `"\u{E000}"` and `"😀"` sort oppositely in Rust and JS; equal-hue representatives can consequently exchange the unchanged and +12° assignments.
Fix: Use an explicit UTF-16 code-unit comparator or enforce an ASCII-only input invariant.

[🟡] crates/dm-icon-core/src/js_math.rs:13 — `js_round` loses JavaScript’s negative zero.
Scenario: Rust returns `+0.0` for `js_round(-0.1)`, while `Math.round(-0.1)` returns `-0`; equality-based tests conceal the bit-level mismatch.
Fix: Preserve a negative sign whenever a negative input rounds to zero and test `to_bits`/`is_sign_negative`.

[🟡] crates/dm-icon-core/src/output_cache.rs:342 — The cache-HIT test also passes if every lookup misses.
Scenario: Recomputing and replacing the same key twice still leaves `len()==1` and identical output bytes.
Fix: Add hit/miss counters or make the second call’s compute branch fail the test.

[🟡] crates/dm-icon-core/src/output_cache.rs:410 — The schema-key test cannot prove that all four version fields are encoded.
Scenario: Every version equals 1, and `[1,0,0,0]` occurs elsewhere in the material, so deleting the complete version suffix can leave all assertions green.
Fix: Assert the exact final 16-byte suffix using distinct sentinel versions.

[🟡] crates/dm-icon-core/src/render_session.rs:198 — Session cache tests do not observe actual cache hits.
Scenario: Recomputing profiles/facts on every call still yields equal profile kinds and the same final map cardinality, satisfying the current assertions.
Fix: Instrument computation/hit counts and test schema invalidation explicitly.

[🟡] crates/dm-icon-core/src/source_facts.rs:257 — The schema-bump guard omits segmentation and transparent-edge parameters. (VERIFY: by-design?)
Scenario: Flood tolerances, plate-split thresholds, the 2% cutoff, or edge rules can change without the sole `SOLID_ALPHA` test requiring a new cache schema.
Fix: Give cached algorithms explicit versions and mechanically test their composite version.

[🔵] crates/dm-icon-core/src/render_session.rs:72 — Native production dependency graphs disable the spec-required `fast` path.
Scenario: Desktop and resident resolve only `dm-icon-core/default`, so geometry/output/source-fact caches are scalar passthroughs despite spec 07 requiring `fast` for the warm-render budget.
Fix: Forward `dm-icon-core/fast` from native release features while retaining explicit scalar certification builds.

[🔵] crates/dm-icon-core/src/render_session.rs:60 — The required disk-persisted profile cache has no hydration or export boundary.
Scenario: Every fresh resident batch/process starts with empty private maps and repeats cold analysis, contrary to spec 07’s persisted warm single-icon path.
Fix: Add a versioned profile/facts DTO interface keyed by source digest and schema, with storage implemented outside the pure core.

[🔵] crates/dm-icon-core/src/output_cache.rs:237 — The Phase-4 cache is memory-only and has no non-test production caller. (VERIFY: by-design?)
Scenario: Reopen, re-apply, resident, version-switch, and process restart all recompute; the planned atomically persisted and verified artifact tier does not exist.
Fix: Own this cache in the native render executor and add an external persistent artifact store.

[🔵] crates/dm-icon-core/src/batch.rs:71 — The Phase-3 collector is test-only and lacks the cancellation/error contract needed by native callers. (VERIFY: by-design?)
Scenario: Resident and version-switch render sequentially; adopting this API as-is gives no generation cancellation and lets one job panic abort the complete collection.
Fix: Wire a bounded long-lived executor accepting cancellation and returning indexed per-item results.

[🔵] crates/dm-icon-core/src/render_session.rs:147 — Scalar sessions eagerly compute and retain facts that scalar accessors ignore.
Scenario: Current native builds call `SourceFacts::compute`, then every accessor recomputes the same analysis during composition and never reads the cached values.
Fix: Compile fact construction/storage only for `fast`, or enable and consume the fast path in production.

[🔵] crates/dm-icon-core/src/render_session.rs:146 — Analysis runs before checking the look, `show_original`, or whether the selected lane consumes a profile.
Scenario: Missing-look and original-card calls perform background detection and segmentation before returning `None` or immediately scaling the source; non-field lanes also compute an unused profile.
Fix: Validate the look first and lazily request profile/facts only for branches that consume them.

[🔵] crates/dm-icon-core/src/source_facts.rs:71 — One facts miss redundantly recomputes dependencies already produced by the profile and by other facts.
Scenario: `foreground_auto` repeats background detection and content bounds, segmentation repeats transparent-edge analysis, and `RenderSession` computes the overlapping profile first.
Fix: Compute one immutable analysis bundle and derive both `IconProfile` and `SourceFacts` from it.

[🔵] crates/dm-icon-core/src/profile.rs:27 — Certification-only fields force production scans and a duplicate owned mask (parity-retained: verify vs oracle).
Scenario: `background_lightness`, `subject_colour`, `subject_lightness`, and `subject_mask` have no production consumer outside xtask, while the mask duplicates cached segmentation.
Fix: Retain oracle diagnostics separately and expose a lean production profile sharing segmentation via `Arc`.

[🔵] crates/dm-icon-core/src/batch.rs:58 — Batch jobs discard source-fact reuse even within one icon render. (VERIFY: by-design?)
Scenario: Passing `None` makes repeated bounds, background, and segmentation requests recompute merely because different jobs use different sources.
Fix: Build one immutable per-job fact context and pass it throughout the render.

[🔵] crates/dm-icon-core/src/batch.rs:73 — `map_init` creates caches per Rayon job, not reliably one per worker as documented. (VERIFY: by-design?)
Scenario: Work-stealing splits can create multiple cold `MaskCache`s on one worker and recompute identical geometry across leaf jobs.
Fix: Use genuine worker-local contexts or deterministic worker-sized partitions.

[🔵] crates/dm-icon-core/src/output_cache.rs:253 — Addressed-cache misses bypass every `RenderSession` cache.
Scenario: A miss calls free `render_tile`, creating a throwaway mask cache and recomputing profile/facts, discarding the Phase-1/2 benefits.
Fix: Accept a caller-provided miss closure or wrap `RenderSession::render`.

[🔵] crates/dm-icon-core/src/output_cache.rs:75 — Keys include inputs that the selected render branch never reads.
Scenario: `show_original=true` still keys on config and `field_seed`; non-shortcuts still key on arrow and mark state, creating duplicate identical tiles.
Fix: Canonicalize key material using the same branch predicates as rendering.

[🔵] crates/dm-icon-core/src/output_cache.rs:70 — Every lookup allocates key material and re-hashes immutable raster inputs.
Scenario: A nominal hit hashes the full source, clones and hashes the arrow, allocates a `Vec`, and may reallocate because the 160-byte reservation is below the 169-byte maximum.
Fix: Cache source/arrow digests and stream canonical fields directly into the hasher.

[🔵] crates/dm-icon-core/src/output_cache.rs:165 — Both HIT and MISS paths copy the complete output tile.
Scenario: A 256px hit clones 256 KiB, while a miss clones another 256 KiB into the cache before returning.
Fix: Store and return `Arc<Raster>`/`Arc<[u8]>` or let consumers encode borrowed cached data.

[🔵] crates/dm-icon-core/src/output_cache.rs:75 — Output-key encoding is a hand-maintained duplicate with no exhaustive coupling to `Config`. (VERIFY: by-design?)
Scenario: Adding an output-affecting config field leaves this serializer compiling and can create stale false HITs.
Fix: Centralize stable canonical encoding or exhaustively destructure `Config` in one shared encoder.

[🔵] crates/dm-icon-core/src/profile.rs:75 — Rim extraction allocates and copies a full-canvas buffer on every erosion pass.
Scenario: A 256px source performs 16 `cur.clone()` allocations and roughly 1 MiB of copy traffic per alpha pass.
Fix: Reuse two preallocated erosion buffers and swap them between passes.

[🔵] crates/dm-icon-core/src/raster.rs:278 — Glass backdrop blur retains excessive full-frame channel scratch.
Scenario: A 256² blur holds four input-channel vectors plus accumulated outputs and per-channel blur scratch, peaking above 4 MiB per concurrent icon.
Fix: Blur one channel at a time through reusable two-buffer scratch and write it directly to the output.

[🔵] crates/dm-icon-core/src/mono.rs:69 — Each stretched-lightness pass allocates 576 KiB at 256².
Scenario: The `Vec<u8>` and `Vec<f64>` temporaries exceed 1.1 MiB when Mono plus Glass performs two passes on one icon.
Fix: Reuse session/thread scratch or let consumers operate on quantized lightness indices.

[🔵] crates/dm-icon-core/src/sampling.rs:223 — The unchanged 256px ICO rung clones the complete master.
Scenario: Every normal 256² ladder allocates and copies 262,144 bytes before encoding its first frame.
Fix: Represent unchanged frames as borrowed/owned `Cow` values or transfer ownership into the encoder.

[🔵] crates/dm-icon-core/src/color.rs:49 — Hot sRGB decoding re-enters `OnceLock::get_or_init` for every channel sample.
Scenario: A 256² supersampled draw can perform roughly 3.15 million initialized-state checks instead of holding one LUT reference.
Fix: Acquire the LUT once per outer resampling/render loop and index it directly.

[⚪] crates/dm-icon-core/src/config.rs:76 — `shortcut_shape` is dead after upstream resolution but remains in the ABI and cache identity. (VERIFY: by-design?)
Scenario: Both native and frontend resolvers fold it into `shape`; the compositor never reads it, yet changing it causes false cache misses.
Fix: Remove it from the resolved core contract/key or make the core solely responsible for shortcut-shape resolution.

[⚪] crates/dm-icon-core/src/color.rs:11 — Six public colour exports have no live Rust caller (parity-retained: verify vs oracle).
Scenario: `ORIGINAL_INK_THRESHOLD`, `hsl_of`, `hsl_to_rgb`, `field_plate_tone`, `clamp_plate_lightness`, and `shift_lightness` remain solely as named frozen-oracle mirrors.
Fix: Retain through certification; remove or rewire them only after coordinated oracle retirement.

[⚪] crates/dm-icon-core/src/mono.rs:134 — The `Subject::Mono` tail of `transform_pixel_in_place` is unreachable (parity-retained: verify vs oracle).
Scenario: Live composition maps Mono whole-tile and calls this function only for BlackWhite; the frozen oracle has the same dormant branch.
Fix: Retain through certification, then prune only with the corresponding oracle export.

[⚪] crates/dm-icon-core/src/source_facts.rs:16 — Cache-key documentation still describes the superseded caller-hash design.
Scenario: Native sessions already use a BLAKE3 content digest, while only WASM retains caller hashes; the stated residual collision plan is obsolete.
Fix: Document the actual native/WASM split and current trust contract.

[⚪] crates/dm-icon-core/src/lib.rs:14 — Crate documentation still says only the Spike-4 slice ships and the full pipeline lands at M5.
Scenario: HEAD exports the complete compositor, sessions, batching, and output cache.
Fix: Replace the historical status paragraph with the current pipeline/certification state.

[⚪] crates/dm-icon-core/src/output_cache.rs:135 — The cache-capacity comment is arithmetically wrong.
Scenario: A 512² RGBA tile is 1 MiB, so 64 MiB holds about 64 payloads, not 256.
Fix: Correct the estimate and distinguish pixel payload from actual resident memory.

CLEAN: crates/dm-icon-core/src/mask_cache.rs

## Pass C — dm-icon-core algorithm

[🟠] crates/dm-icon-core/src/analysis/background.rs:151 — Rect-ring probing uses width for both axes, so accepted non-square sources can panic or be misclassified.
Scenario: Native bake accepts any decodable dimensions; an 8×4 PNG sets `max=7` and reads row 7 out of bounds, while a 4×8 PNG ignores its lower rows.
Fix: Track independent x/y maxima and walk all four rectangular edges; reject zero axes before probing.

[🟠] crates/dm-icon-core/src/marks/mod.rs:203 — Native render paths never initialize `NATIVE_ARROW`, diverging from WASM preview and certified goldens.
Scenario: WASM loads `public/win-native-arrow.png`, but resident/version-switch instantiate `RenderSession` without calling the setter; `Distinction::Keep` silently bakes the drawn fallback.
Fix: Initialize a bundled immutable arrow in the native render context before rendering and fail closed if unavailable.

[🟠] crates/dm-icon-core/src/analysis/background.rs:32 — Integer division moves the frozen oracle’s one-third board-size boundary.
Scenario: A centered opaque 85×85 board on a transparent 256×256 raster passes Rust’s `85 < 256/3` test but fails TS’s `85 < 85.333…`, selecting different composition lanes.
Fix: Compare using floating-point division exactly like the oracle and add an 85/256 boundary fixture.

[🟠] crates/dm-icon-core/src/compose/mod.rs:315 — A fully transparent source fabricates an opaque styled plate instead of rendering nothing. (VERIFY: by-design?)
Scenario: A 256×256 all-alpha-zero raster receives full-canvas content bounds, bypasses `Empty`, and Original+Derived emits a neutral plate; an existing test and the oracle pin this contradiction.
Fix: Track whether any pixel exceeds the visibility threshold and route truly empty rasters to `ComposeLane::Empty` via a reviewed oracle correction.

[🟡] crates/dm-icon-core/src/compose/mod.rs:315 — One-axis-zero rasters bypass the empty guard and later panic.
Scenario: `Raster::new(0,16)` or `Raster::new(16,0)` has only one bound dimension ≤1, proceeds into analysis/sampling, then indexes a nonexistent row or underflows `width-1`/`height-1`.
Fix: Return a transparent Empty tile whenever either source dimension is zero, before profiling or sampling.

[🟡] crates/dm-icon-core/src/marks/styles.rs:33 — `ShadowMark` recomputes an unclamped inset and underflows on a 1px tile.
Scenario: Top-level composition clamps pad to zero, but Shadow recalculates pad=1 and evaluates `1-2`; debug builds panic and release builds wrap to invalid geometry.
Fix: Pass the clamped card geometry into the renderer or apply the identical clamp there.

[🟡] crates/dm-icon-core/src/filters/mod.rs:17 — The public filter API trusts an independent `size` that can disagree with the raster. (VERIFY: by-design?)
Scenario: Applying Gloss to an 8×8 tile with `size=16` indexes beyond `tile.data`; every non-None finish assumes a matching square raster.
Fix: Remove the redundant parameter or validate `width == height == size` before dispatch.

[🟡] crates/dm-icon-core/src/analysis/dominant.rs:46 — `dominant_color` indexes a caller-provided mask without validating its length. (VERIFY: by-design?)
Scenario: A 2×2 raster with a three-byte mask panics on pixel 3, whereas the TS oracle treats the missing entry as false and skips it.
Fix: Validate mask length or use `m.get(i).copied().unwrap_or(0)`.

[🟡] crates/dm-icon-core/src/compose/mod.rs:454 — Public `render_slice_tile` panics on a zero-dimension source. (VERIFY: by-design?)
Scenario: Passing `Raster::new(0,0)` with positive output size reaches bilinear sampling through `draw_bare_with_shadow`, although the WASM wrapper currently rejects this input.
Fix: Apply the same source-dimension guard as the primary render entry point.

[🟡] crates/dm-icon-core/src/compose/mod.rs:153 — Reusing `ComposeDiagnostics` leaks the previous render’s `field_lane`. (VERIFY: by-design?)
Scenario: A derived-field render sets `field_lane`; a subsequent original/classic render using the same sink updates `lane` but leaves that stale sub-lane present.
Fix: Reset all diagnostic fields at the start of each render.

[🟡] crates/dm-icon-core/src/marks/mod.rs:206 — A public mutable arrow global can change during rendering and corrupt cache/output consistency. (VERIFY: by-design?)
Scenario: A fast-cache render hashes arrow A, another thread installs B, then the first render draws B and stores it under A’s key; parallel batches can likewise mix badges.
Fix: Use a boot-once `OnceLock` or pass one immutable arrow snapshot through both key construction and drawing.

[🟡] crates/dm-icon-core/src/shapes/mod.rs:62 — `POLY_CACHE` grows without bound for distinct positive sizes on every render thread. (VERIFY: by-design?)
Scenario: Long-lived workers rendering continually varied output/card sizes retain a new polygon `Vec` for every exact f64-size key with no eviction.
Fix: Use a byte-capped cache or a bounded set of normalized integral size keys.

[🔵] crates/dm-icon-core/src/compose/mod.rs:133 — Shipped native consumers compile the advertised render caches as scalar pass-throughs. (VERIFY: by-design?)
Scenario: Cargo’s native feature graph contains only `dm-icon-core/default`; resident/version-switch therefore recompute geometry and source facts despite spec 07 requiring `fast`.
Fix: Propagate `dm-icon-core/fast` through native production dependencies and rerun the four-way parity certificate.

[🔵] crates/dm-icon-core/src/segment/mod.rs:36 — A cold non-square-profile render computes the expensive segmentation pipeline twice.
Scenario: `RenderSession` first calls `icon_profile`, which segments the source, then `SourceFacts::compute`, which independently calls `segment_subject` again.
Fix: Construct profile and source facts together or share one `Arc<Segmentation>` between them.

[🔵] crates/dm-icon-core/src/analysis/mod.rs:22 — `foreground_auto` repeats background and bounds analysis already computed by `SourceFacts`.
Scenario: Cold fact construction computes content bounds and detected background, then this wrapper reruns both scans before extracting foreground.
Fix: Add a helper accepting precomputed bounds/background and use it during fact construction.

[🔵] crates/dm-icon-core/src/analysis/shape_match.rs:12 — Silhouette matching and inscribe scaling omit the oracle’s per-source/per-shape memoization.
Scenario: Re-rendering the same source and shape rescans solid pixels; inscribed lanes also rebuild the boundary vector and repeat seven fit probes.
Fix: Cache match and maximum-scale results by source identity and `IconShape`, reusing cached content bounds.

[🔵] crates/dm-icon-core/src/compose/field.rs:136 — Bare-field shadow rendering omits the oracle’s per-size scratch reuse.
Scenario: Every 256px render allocates and zeros one RGBA raster plus two f32 fields—768 KiB, roughly 225 MiB of allocator traffic for 300 icons.
Fix: Add worker/session-owned size-keyed scratch buffers and fully overwrite them before reads.

[🔵] crates/dm-icon-core/src/filters/glass.rs:41 — Glass allocates over 2.25 MiB of full-frame temporary storage per 256px render.
Scenario: Two f64 chamfer fields, stretched lightness, subject coverage, and a Raster clone coexist on every filtered icon, creating heavy allocation churn across batches.
Fix: Reuse worker-owned scratch buffers and source storage, with memory capped by worker count.

[🔵] crates/dm-icon-core/src/compose/helpers.rs:131 — Backdrop replacement clones and scans the full source every render despite oracle memoization.
Scenario: Re-rendering one plated source with the same replacement colour repeatedly copies 256 KiB and scans 65,536 pixels.
Fix: Cache the swapped raster by source digest and plate RGB with a byte cap.

[🔵] crates/dm-icon-core/src/compose/mod.rs:198 — Card masks are eagerly built even for no-mark pass-through lanes that never read them.
Scenario: Shape-None or already-matching tiles with no mark compute a full mask, then `pass_through=true` skips clipping and discards it; native scalar builds repeat this every render.
Fix: Compose first and lazily construct the card mask only when clipping or mark rendering requires it.

[🔵] crates/dm-icon-core/src/compose/mod.rs:177 — Monolithic `MarkContext` forces unnecessary masks and luminance scans for shaped Shadow/Halo marks.
Scenario: These styles neither use the initial full-tile mask nor composed luminance, yet every render constructs/scans both.
Fix: Split geometry and adaptive contexts and let each mark declare its required inputs.

[🔵] crates/dm-icon-core/src/marks/mod.rs:98 — Halo/free-form Ring allocate a dummy RGBA raster solely to transport alpha into chamfer distance.
Scenario: Each 256px mark zeros 256 KiB, writes only alpha, then `chamfer_distance` ignores RGB and allocates its own distance field.
Fix: Add a chamfer primitive that consumes the coverage field directly.

[🔵] crates/dm-icon-core/src/compose/helpers.rs:37 — Rust repeats a 24-step centered-square search that the oracle caches once per shape.
Scenario: Three hundred pinched-shape tiles perform about 57,600 redundant `shape_contains` probes for identical geometry.
Fix: Memoize by `IconShape` or precompute the frozen results without changing operation order.

[🔵] crates/dm-icon-core/src/segment/plate.rs:33 — Segmentation calculates byte medians through repeated O(n log n) sorts and ring-vector clones.
Scenario: A 256px plate sorts three full silhouette channel vectors, then clones and sorts three rim vectors; flood-background performs three more byte sorts.
Fix: Use a shared 256-bin histogram selector preserving the current upper-median rule.

[🔵] crates/dm-icon-core/src/segment/plate.rs:238 — Plate detection rescans silhouette statistics already counted upstream.
Scenario: `segment_subject` counts solid pixels, `plate_split` recounts them while finding bounds, and `detect_flat_plate` scans the same bounding box again for square IoU.
Fix: Carry a small bounds/count statistics struct through the segmentation stages.

[🔵] crates/dm-icon-core/src/filters/sticker.rs:25 — Sticker clones the entire tile before a read-only downscale.
Scenario: Every valid 256px Sticker render copies 256 KiB even though `downscale(tile, target)` can borrow the original until it returns.
Fix: Pass `tile` directly to `downscale`, then clear it after the borrow ends.

[🔵] crates/dm-icon-core/src/marks/mod.rs:215 — Every classic-arrow render clones the complete 100×100 native raster.
Scenario: A 300-shortcut bake copies roughly 12 MiB solely to obtain immutable snapshots, with fast cache keying potentially cloning it again.
Fix: Store and return `Arc<Raster>` and precompute the immutable arrow digest.

[🔵] crates/dm-icon-core/src/shapes/mod.rs:182 — Cubic Bernstein evaluation is duplicated across three geometry paths. (VERIFY: by-design?)
Scenario: Authored curves, Apple corners, and smooth-corner flattening independently maintain the same weight/evaluation sequence, creating parity-sensitive drift risk.
Fix: Share an operation-order-preserving evaluator parameterized by steps/scaling and recertify byte parity.

[⚪] crates/dm-icon-core/src/marks/styles.rs:362 — `GlassMark` remains shipping-callable after removal from the product gallery (parity-retained: verify vs oracle).
Scenario: Spec 02 calls it legacy test scaffolding, but config decoding and `resolve_mark` still expose its allocation-heavy renderer to persisted/imported looks.
Fix: Retain through certification, then normalize legacy configs and isolate the renderer to compatibility tests.

[⚪] crates/dm-icon-core/src/compose/field.rs:116 — `DerivedPlate` is unreachable from any honest production profile (parity-retained: verify vs oracle).
Scenario: `Bare && !transparent_edges` cannot survive classification because a sufficiently opaque border yields full-canvas coverage and routes to `FullSquare`; only a forged-profile test reaches this lane.
Fix: Retain through certification, then remove the lane or deliberately revise classification if the capability is wanted.

[⚪] crates/dm-icon-core/src/compose/mod.rs:448 — `render_slice_tile` is a superseded Spike-4 compatibility pipeline. (VERIFY: by-design?)
Scenario: Production uses `RenderSession`; this separate orchestration remains only for xtask/WASM certification exports and carries an independent robustness surface.
Fix: Retire it and its exports once the replacement parity certificate is formally sufficient.

[⚪] crates/dm-icon-core/src/compose/field.rs:25 — `ShadowMode::Halo` has no reachable caller (parity-retained: verify vs oracle).
Scenario: Every field-shadow call selects `Dock`; the unused arm mirrors the frozen oracle’s likewise-unselected pale-art mode.
Fix: Retain through certification, then wire an approved pale lane or remove both oracle and Rust branches.

[⚪] crates/dm-icon-core/src/analysis/dominant.rs:20 — `dispersion` and its circular-moment calculations have no workspace consumer (parity-retained: verify vs oracle).
Scenario: Every chromatic voter pays `cos`/`sin` and a final square root, while profile and composition read only `colour`.
Fix: Retain through certification, then remove the field/calculation or restore a normative consumer.

[⚪] crates/dm-icon-core/src/filters/mod.rs:14 — `pub use glass::glass` is an unused API leak. (VERIFY: by-design?)
Scenario: No workspace caller uses the re-export, while `apply_filter` calls the child module directly and supplies the required Subject-to-hue routing.
Fix: Remove the re-export and narrow `glass` visibility.

[⚪] crates/dm-icon-core/src/shapes/mod.rs:126 — `Seg::Line` and `Seg::Quad` are never constructed by frozen shape definitions (parity-retained: verify vs oracle).
Scenario: All Samsung/Flower/Pebble segments are cubic; the two variants only preserve the oracle’s generic path grammar.
Fix: Retain through certification, then remove them or restore a real generic-path consumer.

[⚪] crates/dm-icon-core/src/shapes/smooth.rs:188 — The stored corner-demand `q` component is never read (parity-retained: verify vs oracle).
Scenario: `resolve_budgets` projects only tuple element `.1` (`p`), so every `.0` value is computed and retained without affecting geometry.
Fix: Retain through certification, then store only `p`.

[⚪] crates/dm-icon-core/src/shapes/smooth.rs:248 — The initial and Sharp-branch `cur` assignments are overwritten before any read (parity-retained: verify vs oracle).
Scenario: Every Smooth corner resets `cur` to `start_point` before cubic evaluation, and Sharp corners never feed the following corner.
Fix: Retain through certification, then remove the dead writes while preserving emitted points.

CLEAN: crates/dm-icon-core/src/filters/pixel.rs

## Pass D — dm-icon-codec + dm-icon-wasm

[🟠] crates/dm-icon-wasm/src/lib.rs:127 — Raw session handles permit double-free and use-after-free.
Scenario: `dispose()` is called twice, or `render()` follows `dispose()` -> `Box::from_raw`/`s.as_mut()` accesses freed storage, corrupting or trapping the WASM instance.
Fix: Use generation-checked integer handles, or make the TS wrapper invalidate the handle and reject every post-disposal call.

[🟠] crates/dm-icon-wasm/src/lib.rs:248 — Render ABI writes `size²·4` bytes without receiving or validating the output buffer capacity. (VERIFY: by-design?)
Scenario: `WasmIconRenderer.render(..., 512)` passes its fixed 256² output allocation -> Rust writes 1 MiB into a 256 KiB block, corrupting adjacent WASM allocations; current UI clamping is not enforced at the ABI/wrapper boundary.
Fix: Pass `out_capacity` and reject undersized buffers, or enforce `size <= 256` before entering WASM.

[🟠] src/icon-wasm/wasm-loader.ts:95 — UTF-8 IDs longer than 512 bytes are silently truncated, creating Rust session-key collisions.
Scenario: two long paths share the first 512 encoded bytes -> the second registration replaces the first Rust source while the JS `registered` set records both full IDs, so both icons render from one source.
Fix: Detect incomplete `encodeInto`, dynamically allocate the complete encoded ID, and pass its exact length.

[🟠] crates/dm-icon-codec/src/ico.rs:178 — ICO validation accepts malformed or contradictory DIB headers.
Scenario: an attacker supplies tightly sized payloads with invalid `biSize`, planes, bit depth, compression, `biSizeImage`, directory dimensions, or dimensions above ICO’s 256px limit -> `parse()` succeeds and the elevated-helper guard treats the file as structurally valid.
Fix: Validate every BITMAPINFOHEADER invariant and require directory dimensions to match the DIB dimensions.

[🟡] crates/dm-icon-codec/src/ladder.rs:38 — A non-square source whose width equals a ladder rung produces a non-square “rung.”
Scenario: a 48×96 source reaches `downscale(source, 48)`, whose width-only short circuit clones it -> the ladder contains 48×96 instead of 48×48, unlike the transposed 96×48 input.
Fix: Use a resampler that short-circuits only when both dimensions equal the target.

[🟡] crates/dm-icon-codec/src/ico.rs:95 — Public ICO assembly panics when `Raster::data` is inconsistent with its dimensions.
Scenario: `Raster { width: 16, height: 16, data: vec![] }` reaches pixel indexing -> an out-of-bounds panic terminates the operation instead of returning a codec error.
Fix: Validate `data.len() == width.checked_mul(height).and_then(|n| n.checked_mul(4))` and return `Result`.

[🟡] crates/dm-icon-codec/src/ico.rs:32 — ICO frame-count and offset field limits are enforced only by `debug_assert!`.
Scenario: a release build receives 65,536 small frames -> the directory count truncates to zero; sufficiently large payload totals can likewise wrap the `u32` offset.
Fix: Make `write_ico` fallible and use checked arithmetic plus checked `u16`/`u32` conversions.

[🟡] crates/dm-icon-codec/src/ico.rs:64 — Dimensions above 256 are encoded as zero, which specifically means 256 rather than “256 or larger.”
Scenario: a 300×300 frame is written with directory dimensions `0×0` but a 300×300 DIB; the crate’s own parser accepts the contradictory file.
Fix: Reject dimensions outside `1..=256` before assembly.

[🟡] crates/dm-icon-wasm/src/abi.rs:125 — Config decoding accepts noncanonical presence flags and a nonzero reserved byte.
Scenario: a corrupted record with `has_mark_color=255` or reserved byte 11 set still renders successfully, hiding ABI drift instead of returning error code 2.
Fix: Require presence flags in `0..=1`, reserved byte 11 to be zero, and packed colours to fit 24 bits.

[🟡] crates/dm-icon-wasm/src/lib.rs:83 — The allocation ABI has no reclamation path and repeated arrow installation leaks permanently.
Scenario: repeated valid `arrow` messages call `dm_alloc` each time; `dm_session_free` reclaims only the session, so linear memory grows until the worker traps.
Fix: Add a matching `dm_dealloc(ptr, len)` or reuse a capacity-tracked arrow scratch buffer.

[⚪] crates/dm-icon-wasm/src/lib.rs:22 — Spike-4 allocation/render exports remain in the production WASM after the M6 session ABI replaced them.
Scenario: live frontend code uses only `dm_*`; `spike4_*` is referenced by obsolete parity tooling/tests, increasing shipped surface and retaining a second intentionally leaking allocator.
Fix: Gate spike exports behind a test/tooling feature or move them to a dedicated harness crate.

CLEAN: crates/dm-icon-codec/src/hash.rs, crates/dm-icon-codec/src/lib.rs

## Pass E — dm-operations/txn

[🔴] crates/dm-operations/src/txn/driver.rs:125 — Batch-wide preflight creates a CAS TOCTOU window that can overwrite newer user edits.
Scenario: item B passes preflight, the user edits B while item A is processed, then B is applied without another conditional check and rollback restores the older captured anchor, destroying the edit.
Fix: make anchor capture plus mutation a platform-level conditional operation against the expected fingerprint, or revalidate atomically at each item’s write boundary.

[🔴] crates/dm-operations/src/txn/recovery.rs:265 — Recovery unconditionally restores every prepared item without checking whether DeskMakeover mutated it or the user changed it afterward.
Scenario: a crash occurs after B is prepared but before B is applied; the user edits B before restart, and recovery replaces that untouched user edit with the preflight anchor.
Fix: journal the intended applied fingerprint before mutation and restore only when live state matches it; treat third-party or ambiguous state as degraded and leave it untouched.

[🔴] crates/dm-operations/src/txn/recovery.rs:250 — `ItemRolledBack` records are ignored, causing already-restored items to be restored again.
Scenario: rollback restores A and durably appends `ItemRolledBack`, but the terminal append fails; the user edits A before restart, and recovery treats the transaction as wholly incomplete and destroys the edit.
Fix: track per-item rollback state during replay and skip both restore and ledger removal for durably rolled-back items.

[🔴] crates/dm-operations/src/txn/asset_store.rs:56 — Caller-provided content hashes are not verified, and mismatched bytes overwrite an existing live asset.
Scenario: two requests reuse hash H with different ICO bytes; the second `put` replaces `H.ico` before `AssetWritten`, instantly changing every committed icon referencing H outside the transaction’s rollback set.
Fix: key files by a digest computed from the actual ICO bytes and reject any existing hash whose content differs instead of overwriting it.

[🔴] crates/dm-operations/src/txn/journal.rs:147 — Creating or recreating the WAL never fsyncs its parent directory.
Scenario: after checkpoint deletes `txn.log`, a new transaction recreates and fsyncs the file, mutates the desktop, then power fails; the directory entry can disappear or an old log can reappear, losing the only restore anchor.
Fix: fsync the journal’s parent after first creation and propagate failure before allowing guarded mutations.

[🔴] crates/dm-operations/src/fs_atomic.rs:45 — Directory fsync is best-effort, unsupported on Windows, and its errors are discarded despite a crash-durability contract.
Scenario: an asset, ledger, or first wallpaper snapshot rename succeeds but its directory update is not durable; power loss removes the new file while the caller already proceeded to reference it or discarded older recovery state.
Fix: make directory publication durability mandatory, sync newly created ancestor entries, and use a Windows durable-replace implementation with write-through semantics.

[🟠] crates/dm-operations/src/txn/recovery.rs:299 — A committed transaction missing `AssetWritten` or `ItemApplied` data silently skips the item and is then checkpointed away.
Scenario: a parse-valid but truncated, reordered, or corrupted log contains `TxnCommitted` without a complete item sequence; recovery fabricates no ledger row, reports success, and deletes the only remaining recovery evidence.
Fix: structurally validate every transaction before mutation and fail closed unless every committed item has one complete prepared→asset→applied→verified sequence.

[🟠] crates/dm-operations/src/txn/recovery.rs:210 — Replay accepts duplicate, out-of-order, unknown-item, post-terminal, and `TxnBegin`-mismatched records.
Scenario: parse-valid corruption or ID reuse produces a structurally impossible sequence; `HashMap::insert` overwrites earlier anchors and recovery may restore or reconcile using unrelated material.
Fix: implement a strict per-transaction replay state machine and reject the entire journal before touching the desktop on any illegal transition.

[🟠] crates/dm-operations/src/txn/recovery.rs:293 — Committed recovery ignores the supplied state reader and never confirms that the durable desktop matches `new_fingerprint`.
Scenario: `TxnCommitted` survives a power loss but the external icon mutation does not; recovery rebuilds a committed ledger row for a desktop that is still original or otherwise changed.
Fix: verify live fingerprint and required assets before reconciliation; surface mismatches as degraded instead of claiming commit. (VERIFY: by-design?)

[🟠] crates/dm-operations/src/txn/journal.rs:214 — Any malformed final content line is silently classified as a torn append, even when newline-terminated and previously durable.
Scenario: bit rot changes the final durable `TxnCommitted` line into invalid JSON; recovery drops it as a tail fragment and rolls back a transaction that had committed.
Fix: tolerate parsing failure only when the physical file lacks a trailing newline; treat malformed newline-terminated records as corruption.

[🟠] crates/dm-operations/src/txn/journal.rs:134 — `FileJournal` has no process/file locking around truncate, append, ID inspection, or checkpoint.
Scenario: two cloned instances or overlapping processes pass the same max-ID check while one checkpoint races another append, yielding reused IDs, lost records, or interleaved JSON.
Fix: hold an OS-level exclusive journal lock across recovery/checkpoint and each complete transaction, including ID allocation. (VERIFY: single-instance serialization sufficient?)

[🟡] crates/dm-operations/src/txn/asset_store.rs:54 — Root and asset-type checks are path-based TOCTOU checks that can be swapped before read, rename, existence checks, or GC.
Scenario: the asset root or a validated regular asset is replaced by a symlink/reparse point after inspection; subsequent operations escape the intended ownership boundary or accept an externally owned file.
Fix: operate through securely opened directory/file handles and reject Windows reparse points at the handle level. (VERIFY: threat model excludes same-user filesystem races?)

[🟡] crates/dm-operations/src/txn/journal.rs:205 — Torn-tail handling fails before JSON parsing when the partial write ends inside a multibyte UTF-8 value.
Scenario: power loss splits a non-ASCII item path mid-codepoint; `BufRead::lines` returns `InvalidData`, so startup recovery cannot reach the otherwise valid durable prefix.
Fix: read raw bytes, locate newline boundaries, parse complete lines as UTF-8 individually, and discard only the unterminated byte tail.

[🟡] crates/dm-operations/src/txn/fakes.rs:295 — Kill-point snapshots occur only at journal appends and cannot model the world after mutation but before the following record.
Scenario: a crash after `applier.apply` but before `ItemApplied` has the earlier journal prefix with a modified desktop, a state absent from `snapshots`, leaving a central WAL window under-tested.
Fix: add mutation-boundary crash hooks or record world snapshots independently after each fake external mutation.

[🟡] crates/dm-operations/src/txn/id.rs:25 — Transaction-ID arithmetic can overflow and either panic or wrap to a reused ID.
Scenario: a journal containing transaction `u64::MAX` makes `max + 1` panic in checked builds or become zero in wrapping builds; `next_id` has the same failure.
Fix: use `checked_add` and return a terminal journal-exhaustion error.

[🟡] crates/dm-operations/src/txn/asset_store.rs:59 — Asset paths are serialized with lossy Unicode conversion.
Scenario: a non-Unicode app-data path is materialized successfully, but `AssetRef.path` contains replacement characters and later icon application or recovery references a different nonexistent path.
Fix: preserve platform-native paths in the durable representation or reject non-round-trippable roots explicitly.

[🟡] crates/dm-operations/src/fs_atomic.rs:23 — Relative targets with an empty parent never fsync the current directory.
Scenario: `write_atomic(Path::new("ledger.json"), …)` returns success after rename, but a crash can lose the directory entry because `"."` was filtered out.
Fix: normalize an empty parent to `"."` and require its successful durability sync.

[🔵] crates/dm-operations/src/txn/driver.rs:234 — The specified persisted prepared→asset-written→applied→verified ledger state machine is absent; only `Committed` is ever written.
Scenario: intermediate `TxnState` variants are never constructed in production, while the specification and ledger model claim each transition is persisted, splitting the state model between an unused enum and the journal.
Fix: either persist the declared ledger transitions or remove/document the legacy states and make the WAL the single authoritative state machine. (VERIFY: by-design?)

[🔵] crates/dm-operations/src/txn/driver.rs:161 — Every nonempty apply rereads and deserializes the entire journal solely to find its maximum ID.
Scenario: a retained degraded or oversized log is cloned and parsed once during recovery and again during apply, increasing latency and memory at the mutation boundary.
Fix: have the locked journal allocate/reserve the next ID or retain the recovered maximum without a second full read.

[🔵] crates/dm-operations/src/txn/journal.rs:231 — Checkpoint membership is O(records × active transactions).
Scenario: a large recovery log with many active transactions repeatedly linearly scans `active_txns` for every record.
Fix: convert active IDs to a `HashSet` before filtering.

[🔵] crates/dm-operations/src/txn/journal.rs:205 — Journal reading holds both all decoded lines and all deserialized records in memory without a size bound.
Scenario: a large or locally corrupted `txn.log` can cause excessive allocation or process OOM during startup recovery.
Fix: stream records with explicit file, line, record-count, and anchor-size limits while retaining structural validation.

[🔵] crates/dm-operations/src/txn/driver.rs:1 — The 520-line driver combines preflight policy, WAL sequencing, mutation, verification, rollback, and abandonment.
Scenario: tightly coupled failure paths duplicate restore/error bookkeeping and make durability-order changes difficult to audit independently.
Fix: split preparation, mutation-state progression, and rollback/abandon execution into small state-machine components.

CLEAN: crates/dm-operations/src/txn/mod.rs

## Pass F — icons+ledger+wallpaper+scope

[🔴 critical bug] crates/dm-operations/src/icons/mod.rs:75 — Untrusted `count` directly reserves an unbounded apply buffer.
Scenario: `applyBakedBegin` receives `count = u32::MAX` → `Vec::with_capacity` attempts hundreds of GB and can abort the process.
Fix: Cap count against the live scan/source count and a small absolute maximum before allocating.

[🔴 critical bug] crates/dm-operations/src/wallpaper/mod.rs:62 — Wallpaper base64 is decoded without any encoded or decoded size limit.
Scenario: A huge command payload causes another proportional allocation during decode → OOM/process abort before validation.
Fix: Reject oversized encoded input before decoding and enforce decoded-byte and image-dimension limits.

[🔴 critical bug] crates/dm-operations/src/icons/scope.rs:89 — Empty privileged-root lists fail open, and the current Windows host constructs both lists empty.
Scenario: A version switch scans `C:\Users\Public\Desktop\Tool.lnk` with empty roots → classification returns `None` and the public item reaches the writer.
Fix: Make missing roots an error on Windows and require validated non-empty known-folder roots before any automated operation.

[🔴 critical bug] crates/dm-operations/src/icons/scope.rs:22 — Lexical normalization cannot establish Windows filesystem ancestry.
Scenario: A user-desktop path is replaced with a junction/reparse point into ProgramData, or reaches the same directory through a device/short-name alias → the string check passes while the writer mutates privileged storage.
Fix: Resolve and validate opened-handle final paths, reject every reparse component, and repeat the check immediately at the write boundary.

[🔴 critical bug] crates/dm-operations/src/icons/mod.rs:578 — Reset restores every ledger target without the mandatory Public Desktop/ProgramData red-line.
Scenario: A foreground/manual apply records a public-desktop item, then reset walks that row → automation writes the administrator-managed item despite spec §6/§14.
Fix: Inject privileged roots into reset and enforce the same handle-safe scope gate before every restore/remove action.

[🔴 critical bug] crates/dm-operations/src/ledger/store.rs:91 — Syntactically valid but semantically corrupt ledger rows are trusted as restore authority.
Scenario: Duplicate item rows or a corrupted target/state/anchor deserialize successfully → reset can process stale duplicates or restore anchor bytes onto the wrong path, destroying current content.
Fix: Validate unique item IDs/targets, committed state, material-bearing anchors, asset hashes, and identity binding on load; otherwise return `CorruptLedger`.

[🟠 major bug] crates/dm-operations/src/icons/version_switch.rs:88 — Store ② is promoted before extraction, baking, packaging, CAS preparation, or journaling.
Scenario: Promotion succeeds and extraction fails—or the process crashes immediately afterward—leaving saved-style Y while the desktop still wears X with no switch record to recover.
Fix: Fully prepare and durably journal the switch before promotion, then make promotion/projection recoverable as one state machine.

[🟠 major bug] crates/dm-operations/src/icons/version_switch.rs:142 — Kind-policy opt-outs are skipped instead of reverting previously styled items.
Scenario: Switch from style X to Y where `Folder=false` → folders retain X rather than returning to their originals, so the desktop does not match Y.
Fix: Generate CAS-gated restore requests for opted-out ledger entries, matching foreground `restore_ids` behavior.

[🟠 major bug] crates/dm-operations/src/icons/version_switch.rs:113 — Per-item projection failures are returned in a side-channel the current caller ignores.
Scenario: One source extraction/bake fails while others commit → `outcome.error` remains empty, the host reports success, and the desktop is silently partial.
Fix: Fold nonempty projection errors into the authoritative operation status or require callers to handle an explicit partial-failure variant.

[🟠 major bug] crates/dm-operations/src/icons/version_switch.rs:173 — Native projection always renders with `field_seed=None`.
Scenario: A derived-plate preset previously used frontend hue spreading/brand accents → switching back rebakes different pixels and records no pinned seed.
Fix: Compute the same hue-spread and app-accent allocation as the frontend, reuse ledger pins where required, and persist each request’s seed.

[🟠 major bug] crates/dm-operations/src/icons/style_resolve.rs:74 — `OverrideSource` accepts `"follow"` while the canonical frontend contract uses `"global"`.
Scenario: A valid saved recipe contains `{source:"global"}` → native version switching rejects it as malformed.
Fix: Rename the serde variant to `Global` and accept `"global"`; migrate `"follow"` only if legacy data actually exists.

[🟠 major bug] crates/dm-operations/src/icons/style_resolve.rs:42 — Native policy buckets every bare executable as `File`, unlike the frontend’s `ExecutableFile → App` rule.
Scenario: App and File policies/type patches differ for `tool.exe` → foreground apply treats it as App but resident/version switching applies File styling.
Fix: Add executable identity to the domain taxonomy and map it to `KindBucket::App` end-to-end.

[🟠 major bug] crates/dm-operations/src/icons/mod.rs:421 — Desktop/ledger commit, saved-style, and look-history are three unjournaled writes.
Scenario: Power loss after ledger commit but before ②/③ writes → the desktop wears X while automation resumes Y and history omits X.
Fix: Add a durable finalize record and recover ①→②→③ idempotently before accepting another operation.

[🟠 major bug] crates/dm-operations/src/icons/mod.rs:519 — Reset restores/removes rows before its saved-style/toggle finalizer, without a reset journal.
Scenario: Crash midway through reset → some icons are original, other rows remain, and auto-format plus saved-style stay enabled, allowing old styling to resume.
Fix: Journal reset intent and per-item progress, then recover through the coupled ②/toggle finalization.

[🟠 major bug] crates/dm-operations/src/wallpaper/mod.rs:125 — Sanitized monitor IDs are not unique and drive stale-file deletion.
Scenario: Two monitor IDs differing only in punctuation sanitize identically → applying monitor B prunes monitor A’s currently referenced baked PNG.
Fix: Include a collision-resistant hash of the raw monitor ID in both filename and pruning key.

[🟠 major bug] crates/dm-operations/src/wallpaper/mod.rs:65 — Apply validates only the eight-byte PNG signature.
Scenario: Signature-prefixed garbage or a malformed/decompression-bomb PNG is persisted and handed to Windows → apply fails late or leaves the shell referencing unusable content.
Fix: Decode with strict limits and verify PNG structure, dimensions, and complete image data before snapshot or disk mutation.

[🟠 major bug] crates/dm-operations/src/wallpaper/decode.rs:21 — Source decode has no product-level dimension or memory budget.
Scenario: A malicious or extreme-resolution wallpaper expands into hundreds of MB/GB, then duplicates memory during RGBA conversion and PNG re-encoding.
Fix: Configure decoder limits, reject excessive dimensions/pixels, and downscale or stream within a documented budget.

[🟠 major bug] crates/dm-operations/src/settings_store.rs:96 — Saved-style/reset updates do not verify that the singleton row was changed.
Scenario: The row is missing but the schema remains valid → `set_saved_style` or reset reports success after affecting zero rows, losing the requested persistence.
Fix: Require exactly one affected row or recreate/repair the singleton transactionally and report corruption otherwise.

[🟡 minor bug/robustness] crates/dm-operations/src/ledger/store.rs:40 — `next_version` overflows at `u64::MAX`.
Scenario: A valid JSON ledger contains version `18446744073709551615` → debug builds panic and release builds wrap to zero, breaking ordering.
Fix: Use `checked_add(1)` and fail closed as corrupt/exhausted ledger state.

[🟡 minor bug/robustness] crates/dm-operations/src/wallpaper/snapshot_store.rs:53 — Snapshot deletion is not directory-synced.
Scenario: Power loss after successful restore and `clear()` → the deletion may be lost, resurrecting a stale snapshot that suppresses the next true-original capture. (VERIFY: by-design?)
Fix: Durably remove/rename-and-sync the snapshot and its parent directory where supported.

[🟡 minor bug/robustness] crates/dm-operations/src/wallpaper/mod.rs:82 — A failed `set` strands every newly materialized bake.
Scenario: Repeated COM failures with different payloads write unique PNGs, never reach pruning, and grow the app-data directory without bound.
Fix: Remove the just-written file on `set` failure when it is not referenced, and run startup orphan cleanup.

[🟡 minor bug/robustness] crates/dm-operations/src/ledger/history.rs:113 — Head dedup updates only the timestamp and discards the new label.
Scenario: Reapplying the identical recipe with a new caller-supplied name returns `BumpedHead` but preserves the stale label. (VERIFY: by-design?)
Fix: Define dedup metadata semantics explicitly and update the label when the new apply supplies one.

[🟡 minor bug/robustness] crates/dm-operations/src/wallpaper/decode.rs:18 — `exists()` introduces a redundant TOCTOU check.
Scenario: The file disappears after `exists()` but before `image::open()` → a missing source is misreported as generic I/O, while every successful decode performs two path lookups.
Fix: Open directly and map `NotFound` from the open/decode error chain.

[🟡 minor bug/robustness] crates/dm-operations/src/wallpaper/decode.rs:9 — Windows-codec wallpaper formats such as HEIC remain unimplemented. (VERIFY: by-design?)
Scenario: Windows reports a valid HEIC wallpaper → source decode fails and the wallpaper preview/import path becomes unavailable.
Fix: Add a bounded Windows WIC fallback or explicitly reject unsupported active-wallpaper formats in product readiness.

[⚪ dead/legacy code] crates/dm-operations/src/ledger/entry.rs:38 — `TxnState::is_terminal` is used only by its unit test.
Scenario: No production recovery/store path calls the predicate, so it adds an unused state-machine API. (VERIFY: by-design?)
Fix: Remove it or make recovery consume it as the canonical terminal-state test.

[⚪ dead/legacy code] crates/dm-operations/src/ledger/history.rs:48 — `LookVersion::new` is used only in tests.
Scenario: Production callers construct `LookVersion` directly, leaving the public constructor as test-only surface. (VERIFY: by-design?)
Fix: Use it in production construction or confine the helper to tests.

[🔵 perf/architecture/DRY-SOLID] crates/dm-operations/src/ledger/store.rs:106 — Every item upsert reloads, reserializes, fsyncs, and replaces the entire ledger.
Scenario: A 300-icon commit calls `next_version` plus `upsert` per item → quadratic JSON work and hundreds of full durable rewrites.
Fix: Add a validated batch mutation/transaction API that allocates versions and persists the final ledger once.

[🔵 perf/architecture/DRY-SOLID] crates/dm-operations/src/icons/version_switch.rs:132 — Projection retains base64 masters, decoded ICOs, cloned requests, and render caches for the whole desktop.
Scenario: Hundreds of icons produce multiple simultaneous full-batch representations and high peak memory; item lookup at line 211 also adds O(n²) scanning.
Fix: Use ID-indexed maps and bounded preparation batches while preserving one durable transaction envelope.

[🔵 perf/architecture/DRY-SOLID] crates/dm-operations/src/icons/native_bake.rs:75 — Native baking PNG-encodes to base64 only for packaging to decode it immediately.
Scenario: Every resident/version-switch icon incurs PNG compression, base64 expansion, decode, and another raster allocation inside one Rust process.
Fix: Add an in-process raster/PNG-byte packaging path and reserve base64 for the frontend ABI.

[🔵 perf/architecture/DRY-SOLID] crates/dm-operations/src/icons/mod.rs:191 — The 675-line orchestration module owns sessions, apply, keep-restore, recovery policy, finalization, reset, GC, and state reads.
Scenario: Safety invariants span long interleaved methods, making scope/finalization omissions difficult to detect and test independently.
Fix: Split apply-session validation, projection/finalization, reset, and asset-liveness into cohesive components behind explicit transactional interfaces.

CLEAN: crates/dm-operations/src/icons/package.rs; crates/dm-operations/src/ledger/mod.rs.

## Pass G — dm-resident

[🔴] crates/dm-resident/src/reconciler/mod.rs:385 — The promised proposal-snapshot CAS is batch-preflighted, leaving a TOCTOU window before each actual mutation.
Scenario: item B passes `TxnDriver` preflight, then the user edits B while item A is being written; B is subsequently overwritten without another fingerprint check.
Fix: re-read and compare each fingerprint immediately before its external mutation, and abort/skip that item on mismatch.

[🟠] crates/dm-resident/src/reconciler/mod.rs:334 — Existing pinned hue seeds are never supplied, contrary to spec 07 §5.
Scenario: every newcomer is baked with `None` and committed with `pinned_seed: None`, so hue allocation cannot account for existing icons and produces collisions or unstable palette continuity.
Fix: load live ledger seeds, allocate against them, and persist each selected seed in the request.

[🟠] crates/dm-resident/src/reconciler/mod.rs:385 — Activity is not checked between transaction writes as required by spec 07 §11.
Scenario: the desktop becomes busy after the final poll; `TxnDriver::apply` writes the entire batch sequentially while the user is dragging or editing icons.
Fix: give the transaction driver a fail-closed per-item activity/cancellation hook checked immediately before every mutation.

[🟠] crates/dm-resident/src/reconciler/mod.rs:157 — Activity-monitor failures silently masquerade as ordinary busy state and can strand automation forever.
Scenario: a broken WinEvent/foreground adapter returns `Err` every cycle; outcomes contain only `deferred_busy=true`, with no error or tray-failure signal.
Fix: remain fail-closed but also record the port error and transition/report a diagnosable degraded state.

[🟠] crates/dm-resident/src/reconciler/mod.rs:169 — Reconciliation never removes pending-privileged entries that vanished or moved out of privileged scope.
Scenario: a Public Desktop item is queued, then deleted or moved to the user desktop; its stale queue entry and tray count persist, and a later UAC drain receives the obsolete path.
Fix: reconcile the queue against the complete live scan each cycle, updating current privileged targets and removing all others.

[🟠] crates/dm-resident/src/pending_privileged.rs:48 — `drain_for_elevation` destroys pending work before UAC success is known.
Scenario: opening the window drains the queue and the user cancels UAC or the elevated batch fails; pending state disappears until a later rescan happens to reconstruct it.
Fix: use peek/lease plus explicit acknowledgement, retaining or requeuing every uncommitted item. (VERIFY: by-design?)

[🟠] crates/dm-resident/src/reconciler/mod.rs:231 — Pending proposals have no identity, deadline, cancellation state, or deduplication.
Scenario: periodic reconciles repeatedly propose the same unledgered item, creating multiple notifications/timeouts; an old persisted proposal can also apply after disable/re-enable if its fingerprint still matches.
Fix: maintain a durable proposal registry keyed by item identity with creation/deadline/style revision, merge repeats, and invalidate it on disable, edit, or completion. (VERIFY: by-design?)

[🟠] crates/dm-resident/src/lib.rs:25 — [WV] The decision core has no production caller; only tests construct `Reconciler`, tray transitions, or the privileged queue.
Scenario: at HEAD the watcher→reconcile→proposal-timeout→apply loop and tray/residency wiring do not exist, so no Windows user can exercise M7 auto-format.
Fix: wire the exported core into serialized host state, the watcher/periodic loop, durable proposals, tray events, and the shared transaction stores.

[🟡] crates/dm-resident/src/stability.rs:43 — Stability means two equal observations regardless of elapsed quiet time.
Scenario: two reconciles run back-to-back during a temporary writer pause and mark an item settled before the writer resumes, defeating the intended debounce interval.
Fix: retain observation time/generation and require the configured quiet duration in addition to two equal readable snapshots. (VERIFY: by-design?)

[🟡] crates/dm-resident/src/tray_state.rs:77 — Activity beginning during `Working` is discarded, so `BatchDone` can incorrectly return to `Watching`.
Scenario: `ActivityStart` arrives while a batch runs and is a no-op; the batch finishes while activity remains live, but the state becomes `Watching` without another start event.
Fix: preserve busy as orthogonal state or make completion consume the current activity flag and land in `Paused`. (VERIFY: by-design?)

[🟡] crates/dm-resident/src/reconciler/mod.rs:163 — Scanner infrastructure failures are mislabeled as `InvalidPayload`.
Scenario: filesystem/COM scanning fails and the host receives a baked-payload validation error, leading to misleading diagnostics and potentially incorrect error handling.
Fix: convert `PortError` through `OperationError::Port` instead of `InvalidPayload`.

[🔵] crates/dm-resident/src/reconciler/mod.rs:273 — A whole batch is held in several full-size representations before any bounded handoff.
Scenario: a large installer burst accumulates source PNGs, base64 masters, packaged ICOs, and cloned request bytes simultaneously, causing high RSS or OOM.
Fix: impose a batch cap and process deferred candidates in bounded transactional waves.

[🔵] crates/dm-resident/src/reconciler/mod.rs:363 — Request construction performs an O(n²) candidate search despite already building an index.
Scenario: every packaged item scans all `anchors`, adding avoidable quadratic work to large catch-up batches.
Fix: build one `HashMap<ItemId, VettedCandidate>` and use it for both target and fingerprint lookup.

[🔵] crates/dm-resident/src/pending_privileged.rs:38 — Vec-based deduplication makes repeated privileged rescans O(n²).
Scenario: each full scan linearly searches an ever-growing queue, compounded by the missing stale-entry pruning.
Fix: use an insertion-ordered map keyed by `ItemId`.

[⚪] crates/dm-resident/src/reconciler/mod.rs:133 — `_txn`, `ctx.trust`, and `ctx.freshness` are accepted by reconciliation but never affect it.
Scenario: callers must manufacture policy and allocator inputs that the v1 decision path ignores, obscuring which layer actually owns consent.
Fix: remove dormant parameters until needed or return an explicit notification/consent decision from the core. (VERIFY: by-design?)

[🔵] crates/dm-resident/src/reconciler/tests.rs:333 — The 692-line gate suite omits the highest-risk lifecycle and post-preflight race cases.
Scenario: all 26 tests pass despite stale privileged entries, destructive failed drains, repeated proposals, and activity/user edits after the final poll remaining uncovered.
Fix: split tests by invariant and add adversarial lifecycle and mutation-interleaving fakes.

CLEAN: crates/dm-resident/src/consent.rs

## Pass H1 — dm-windows core

[🔴] crates/dm-windows/src/com/sta_actor.rs:133 — `WM_APP+1` carries an unauthenticated raw `Box` pointer, allowing memory corruption.
Scenario: Any same-integrity process posts `WM_STA_JOB` with an arbitrary `lParam` -> `Box::from_raw` dereferences/frees attacker-selected memory and crashes or compromises the process.
Fix: Keep jobs in a synchronized Rust queue and use the thread message only as a pointer-free wake signal.

[🔴] crates/dm-windows/src/shell/attrs.rs:26 — Reparse-point validation is check-then-use and does not close the advertised TOCTOU defense.
Scenario: A desktop directory/file is replaced with a junction or symlink after `is_reparse_point` returns false but before the later pathname write -> `desktop.ini` or shortcut changes land in another writable location.
Fix: Open targets with `FILE_FLAG_OPEN_REPARSE_POINT`, verify handles/file IDs, and perform handle-relative or otherwise race-safe writes.

[🔴] crates/dm-windows/src/shell/shell_link.rs:43 — Preclaiming then closing the temp file does not protect the subsequent COM reopen.
Scenario: A same-user process replaces the claimed temp between `claim_temp_for` and `IPersistFile::Save` with a reparse point -> COM truncates the link target, causing arbitrary same-user data loss.
Fix: Serialize through `IPersistStream` into memory/a retained handle, then publish without reopening an attacker-replaceable pathname.

[🟠] crates/dm-windows/src/source.rs:36 — Icon extraction uses the shared permanent STA instead of ADR-0019’s disposable isolation worker.
Scenario: A third-party shell extension hangs inside `IShellItemImageFactory::GetImage` -> the sole STA is permanently occupied and scanning, layout, shortcut reads, and writes all stop.
Fix: Run shell-image extraction in a disposable STA worker with bounded timeout and abandon/recreate it on hangs.

[🟠] crates/dm-windows/src/com/sta_actor.rs:80 — `run` deadlocks when invoked recursively from its own STA thread.
Scenario: An STA job calls any adapter backed by the same executor -> the nested job is posted, then the only consumer blocks waiting for its own reply.
Fix: Detect the worker thread and execute reentrant jobs inline, or reject them explicitly before blocking.

[🟠] crates/dm-windows/src/shell/scan.rs:88 — Fresh regular files are incorrectly marked as requiring no wrapper consent.
Scenario: A user applies a look containing `report.pdf` -> the app may silently hide/system-mark the file and create a wrapper despite the “no silent wrapping” safety rule.
Fix: Set the regular-file consent flag/state and require recorded per-item consent before packaging or apply.

[🟠] crates/dm-windows/src/shell/scan.rs:93 — Required SystemIcon CLSID discovery is completely absent.
Scenario: This PC, Network, User Files, or Control Panel is enabled on the desktop -> the filesystem walk never sees it, so the mirror and style operation omit a spec-required icon.
Fix: Enumerate enabled desktop namespace CLSIDs and emit `ItemKind::System` items with stable CLSID identities.

[🟠] crates/dm-windows/src/shell/scan.rs:61 — Every `.lnk` is classified as `Shortcut`; Appx/AUMID discovery and manifest-logo extraction are missing.
Scenario: A Store/UWP desktop shortcut is scanned -> it receives the wrong taxonomy and bypasses the specified AUMID-to-manifest source path.
Fix: Inspect shell-link PIDLs/property-store AUMIDs, classify Appx links, and resolve scale-qualified manifest logos.

[🟠] crates/dm-windows/src/shell/scan.rs:37 — Concatenating user and public filesystem roots does not implement the merged Desktop namespace.
Scenario: User Desktop and Public Desktop contain the same visible shortcut name -> the scan emits duplicate tiles although Explorer resolves them as one namespace collision.
Fix: Enumerate the merged Desktop shell folder via PIDLs, retaining filesystem paths where available.

[🟠] crates/dm-windows/src/shell/scan.rs:38 — Root and entry enumeration errors silently publish an incomplete scan.
Scenario: OneDrive, antivirus, or a transient sharing error makes a root/entry unreadable -> it disappears without a degraded reason, and callers cannot distinguish absence from scan failure.
Fix: Propagate root failures and retain per-entry error items/statuses instead of `continue`/`flatten`.

[🟠] crates/dm-windows/src/shell/known_folders.rs:23 — All known-folder API failures are treated as benign missing roots.
Scenario: Resolving the user Desktop fails from COM/profile corruption -> `desktop_roots` returns success, and the scan may show only virtual/public items.
Fix: Skip only documented not-found cases; require the user Desktop or return a contextual error.

[🟠] crates/dm-windows/src/source.rs:123 — Quoted `IconResource="path",index` values are parsed incorrectly.
Scenario: Captured `desktop.ini` contains `IconResource="C:\\Icons\\folder.dll",-7` -> the parser leaves a quote attached to the path and fails to recover the original folder icon.
Fix: Reuse `parse_icon_location` on the raw value, splitting the comma before stripping path quotes.

[🟠] crates/dm-windows/src/source.rs:451 — Live Recycle Bin extraction does not fall back from an unresolvable `full` value to `default`.
Scenario: `full` exists but names a missing DLL while `default` is valid -> `.or(default)` selects the bad raw value and falls back to the current shell state, potentially using the empty image as “full.”
Fix: Attempt `resource_from_value(full)` first, then independently attempt `default`.

[🟠] crates/dm-windows/src/shell/layout.rs:133 — Position identity is only the localized display name and is inherently ambiguous.
Scenario: Two desktop items display as “App,” or Explorer hides `report.pdf`’s extension while the scanner names it `report.pdf` -> the host overwrites one HashMap slot or assigns a synthetic/wrong position.
Fix: Return a stable parsing path/PIDL-derived identity and join positions by item ID rather than display text.

[🟠] crates/dm-windows/src/source.rs:645 — Bitmap buffer sizing uses unchecked dimension multiplication before an unsafe GDI write. (VERIFY: by-design?)
Scenario: A malformed shell extension returns an unexpectedly large bitmap -> multiplication can overflow or allocation can be undersized before `GetDIBits` writes into the buffer.
Fix: Use checked multiplication, impose a dimension/byte cap, and reject inconsistent bitmap metadata.

[🟡] crates/dm-windows/src/com/sta_actor.rs:112 — Executor shutdown ignores a failed `PostThreadMessageW` and then joins indefinitely.
Scenario: Posting `WM_QUIT` fails while the worker remains blocked in `GetMessageW` -> dropping the executor hangs the process.
Fix: Check the result and provide a second bounded shutdown path rather than unconditional join.

[🟡] crates/dm-windows/src/com/sta_actor.rs:129 — `GetMessageW` failure abandons raw queued jobs without reclaiming them.
Scenario: The message loop returns `-1` with pending job messages -> their heap pointers leak and callers receive only an opaque disconnected-channel error.
Fix: Report the Win32 error and drain/reclaim queued jobs; a pointer-free job queue also removes this leak.

[🟡] crates/dm-windows/src/shell/layout.rs:12 — The documented SysListView32 fallback remains unwritten. (VERIFY: by-design?)
Scenario: Technique A fails after an Explorer change or restart while the ListView remains readable -> all positions degrade to fabricated layout.
Fix: Implement the gated Technique B fallback or formally verify and remove the claimed requirement.

[🟡] crates/dm-windows/src/shell/shell_link.rs:14 — Fixed 1,024-unit buffers silently cap icon, target, description, and working-directory reads.
Scenario: A long redirected Desktop path or shell-link field exceeds 1,023 UTF-16 units -> the value is truncated or read fails, breaking wrapper reunification and fingerprint verification.
Fix: Use Windows’ full path ceiling or PIDL/property-store APIs and detect missing terminators/truncation explicitly.

[🟡] crates/dm-windows/src/classify.rs:57 — Extended-length conversion excludes long UNC desktop paths.
Scenario: Enterprise folder redirection places a shortcut under a `\\server\share\...` path longer than `MAX_PATH` -> `IPersistFile::Load/Save` receives no `\\?\UNC\` form and fails.
Fix: Convert absolute UNC paths to `\\?\UNC\server\share\...` while preserving existing extended prefixes.

[🟡] crates/dm-windows/src/source.rs:139 — Captured `desktop.ini` decoding omits the Windows ANSI code page and UTF-16BE.
Scenario: A legacy ANSI `desktop.ini` contains a non-ASCII icon filename -> lossy UTF-8 changes the path and original-icon extraction degrades.
Fix: Decode BOM variants first, then use the applicable Windows code page rather than lossy UTF-8.

[🟡] crates/dm-windows/src/source.rs:114 — Folder-icon parsing ignores section boundaries.
Scenario: Another INI section contains `IconResource` before `[.ShellClassInfo]` -> that unrelated value is selected as the original folder icon.
Fix: Parse only the first applicable `.ShellClassInfo` section with normal INI section rules.

[🟡] crates/dm-windows/src/source.rs:338 — A failed scratch-file write strands the newly created partial file.
Scenario: Disk-full or an interrupted write makes `write_all` fail -> `?` returns without removing the `dm-orig-*` file, allowing repeated scans to accumulate residue.
Fix: Use an RAII temporary file or explicitly remove the path on every post-create failure.

[🟡] crates/dm-windows/src/source.rs:647 — Partial `GetDIBits` success is accepted as a complete image.
Scenario: GDI returns fewer than `h` scan lines but nonzero -> zero-filled trailing rows are encoded into the source PNG.
Fix: Require `got == h` and reject or retry partial reads.

[🔵] crates/dm-windows/src/source.rs:1 — The 757-line module combines anchor recovery, INI parsing, scratch storage, shell extraction, GDI rasterization, PNG encoding, and tests.
Scenario: Unrelated Windows lifetime, parsing, and pixel changes share one high-risk unsafe module and are difficult to review independently.
Fix: Split into focused `anchors`, `ini`, `resources`, `gdi`, and pixel/encoding modules.

[🔵] crates/dm-windows/src/shell/shell_link.rs:19 — COM create/cast/load boilerplate is duplicated across four readers.
Scenario: Path flags, long-path handling, or HRESULT mapping changes can drift between icon, wrapper, target, and description reads.
Fix: Centralize loaded `IShellLinkW` construction in one STA-confined helper.

[🔵] crates/dm-windows/src/shell/scan.rs:111 — Sorting allocates two lowercase strings for every comparison.
Scenario: A large desktop causes `O(n log n)` repeated Unicode allocations during every scan.
Fix: Precompute a case-folded key once per item or use `sort_by_cached_key`.

CLEAN: crates/dm-windows/src/lib.rs, crates/dm-windows/src/fingerprint_surface.rs, crates/dm-windows/src/com/apartment.rs, crates/dm-windows/src/com/mod.rs, crates/dm-windows/src/shell/mod.rs

## Pass H2 — dm-windows apply+runtime

🔴 crates/dm-windows/src/state_reader.rs:49 — Shortcut and `.url` CAS fingerprints cover only icon fields, but restore replays the entire original file.
Scenario: After styling, the user changes a shortcut target/arguments or `.url` URL while retaining the icon; CAS still matches and restore overwrites the edit with stale bytes.
Fix: Fingerprint every field restore overwrites, or restore only the owned icon fields.

🔴 crates/dm-windows/src/state_reader.rs:73 — Folder CAS omits most `desktop.ini` content although restore replaces the entire file.
Scenario: The user edits `InfoTip`, localization, sharing, or other shell settings after styling; the icon fingerprint remains unchanged and restore silently discards those edits.
Fix: Fingerprint the complete file or perform an owned-field merge during restore.

🔴 crates/dm-windows/src/state_reader.rs:91 — Wrapper CAS omits description, arguments, hotkey, show state, and other fields later deleted or byte-restored.
Scenario: A user customizes the generated or pre-existing wrapper while retaining icon/target/workdir; reset accepts the stale CAS and deletes or overwrites the customized shortcut.
Fix: Fingerprint full wrapper bytes/identity, including the ownership marker, before destructive restore.

🔴 crates/dm-windows/src/apply/recyclebin.rs:56 — Restoring an originally absent HKCU key recursively deletes the entire current key.
Scenario: Another program adds an unrelated value/subkey after DeskMakeover creates `DefaultIcon`; restore’s `delete_subkey_all` destroys that external shell state.
Fix: Delete only owned values and remove the key only after verifying it remains app-created and empty.

🔴 crates/dm-windows/src/apply/recyclebin.rs:57 — Registry deletion failures are discarded and reported as successful restore.
Scenario: Access denial or hive failure leaves styled values installed, but the transaction considers restore complete and may discard the only anchor.
Fix: Ignore only explicit not-found errors; propagate every other `delete_subkey_all`/`delete_value` failure.

🔴 crates/dm-windows/src/apply/recyclebin.rs:132 — String registry anchors are decoded lossily instead of failing closed.
Scenario: An odd-length, unterminated, invalid-UTF-16, or embedded-NUL `REG_SZ`/`REG_EXPAND_SZ` is captured as altered text and later restored with different bytes.
Fix: Preserve the complete raw registry blob and type, or reject any non-canonical string encoding.

🔴 crates/dm-windows/src/wallpaper.rs:75 — Slideshow configuration is neither captured nor restored despite the exact-restore contract.
Scenario: Applying over a slideshow records only a boolean/current frames; restore returns success, clears the snapshot, and permanently converts the slideshow to static images.
Fix: Capture its `IShellItemArray` and options and re-arm it with `SetSlideshow`, failing restore if exact recovery is impossible.

🔴 crates/dm-windows/src/wallpaper.rs:161 — Failure to clear a solid-colour monitor is swallowed.
Scenario: `SetWallpaper("", …)` fails, restore returns success, and `WallpaperOperations` deletes the snapshot while the DeskMakeover image remains.
Fix: Propagate the COM error and retain the snapshot for retry.

🔴 crates/dm-windows/src/durable.rs:127 — The fresh-target publication path can overwrite a file created after the existence check.
Scenario: A wrapper is absent at capture, the user creates `<file>.lnk` between `exists()` and `MoveFileExW`, and `MOVEFILE_REPLACE_EXISTING` destroys it; restore then deletes the replacement.
Fix: Publish new targets without replace semantics and treat an already-existing destination as a CAS conflict.

🔴 crates/dm-windows/src/durable.rs:127 — `Path::exists()` fails open on metadata errors and selects the metadata-destroying move path.
Scenario: An existing target cannot be statted but can be replaced; it is treated as absent and `MoveFileExW` drops its DACL/ADS/compression state.
Fix: Use `try_exists`, propagate errors, and bind the decision to an opened destination handle.

🔴 crates/dm-windows/src/durable.rs:179 — COM temp files have an acknowledged delete-and-symlink TOCTOU.
Scenario: A same-user process replaces the claimed temp between handle close and `IPersistFile::Save`; COM follows the replacement and truncates an attacker-selected accessible file.
Fix: Save through `IPersistStream`/memory or retain a non-reopenable handle-based publication path.

🔴 crates/dm-windows/src/durable.rs:127 — Existing-target replacement has no proven power-loss durability barrier. (VERIFY: by-design?)
Scenario: Power fails after `ReplaceFileW` returns but before NTFS persists the namespace replacement; the journal can be committed while the live file reverts or disappears, poisoning CAS.
Fix: Use a verified Windows durability protocol and add kill/power-failure recovery re-verification.

🟠 crates/dm-windows/src/apply/mod.rs:62 — Manual styling of advertised `ItemKind::System` icons is still unimplemented.
Scenario: This PC, Network, User Files, or Control Panel is discovered as styleable, but read, anchor capture, and apply all return `Unsupported`.
Fix: Implement generalized per-user CLSID `DefaultIcon` discovery, typed capture, apply, and restore.

🟠 crates/dm-windows/src/apply/folder.rs:44 — Restore treats every `is_dir()` failure as successful absence.
Scenario: Permission, sharing, or device failure makes `is_dir()` return false; restore returns `Ok`, leaving styled state while recovery may discard its anchor.
Fix: Use fallible metadata and accept only genuine not-found as an idempotent success.

🟠 crates/dm-windows/src/apply/file_wrapper.rs:43 — Wrapper restore uses fail-open `exists()` checks for both files.
Scenario: Metadata access fails for the wrapper or original; removal/byte replay/attribute restoration is silently skipped and restore returns success.
Fix: Use `try_exists` and propagate all errors except genuine absence.

🟠 crates/dm-windows/src/apply/folder.rs:30 — Apply replaces the complete existing `desktop.ini` instead of updating owned icon fields.
Scenario: A folder with localization, infotip, sharing, or view metadata loses those active settings for the entire styled period.
Fix: Preserve encoding and upsert only `IconResource`/`ConfirmFileOp` in the relevant section.

🟠 crates/dm-windows/src/apply/url_shortcut.rs:14 — `.url` apply accepts only UTF-8 text. (VERIFY: by-design?)
Scenario: Explorer or legacy software produces UTF-16LE or system-codepage `.url` content; a valid shortcut cannot be styled and aborts the transaction.
Fix: Detect BOM/encoding, edit losslessly, and preserve the original encoding.

🟠 crates/dm-windows/src/apply/url_shortcut.rs:13 — `.url` apply lacks the leaf reparse-point guard used by other writers.
Scenario: The shortcut is swapped for a symlink between scan and apply; reading follows it and publication replaces or rewrites state outside the scanned identity.
Fix: Reject reparse points at mutation time and bind read/publish to the same opened file identity.

🟠 crates/dm-windows/src/pathcheck.rs:18 — Path checks return only booleans, leaving every later pathname mutation exposed to TOCTOU. (VERIFY: by-design?)
Scenario: A checked desktop entry or ancestor is replaced by a reparse point before COM/filesystem mutation; the operation acts on a different object than CAS inspected.
Fix: Open with reparse-safe flags, verify volume/file ID and allowed ancestry, and mutate relative to retained handles.

🟠 crates/dm-windows/src/textfmt.rs:68 — Malformed icon indices collapse to zero and can collide with valid state.
Scenario: An external edit changes `IconIndex=0` to `IconIndex=garbage`; both fingerprint as index zero, so CAS misses the change and restore overwrites it.
Fix: Preserve malformed raw values distinctly or return a parse error instead of defaulting.

🟠 crates/dm-windows/src/watcher.rs:99 — The desktop watcher has no production call site, so the resident watcher-to-reconciler loop is absent.
Scenario: New or changed desktop items never generate runtime reconciliation work despite the exported watcher implementation.
Fix: Wire and retain `DesktopWatch` in resident startup/lifecycle, including restart and resume catch-up.

🟠 crates/dm-windows/src/watcher.rs:16 — Windows buffer overflow is knowingly silent.
Scenario: A sufficiently large installer burst overflows `ReadDirectoryChangesW`; no event or `Overflow` hint is emitted, so items remain unformatted indefinitely without another full reconcile.
Fix: Patch the backend to surface zero-byte completions and add periodic full reconciliation.

🟠 crates/dm-windows/src/watcher.rs:130 — Backend errors request reconciliation but never re-arm a dead watch.
Scenario: The desktop root disappears, Explorer/session state changes, or the backend terminates; one `Overflow` fires, then future changes remain permanently unwatched.
Fix: Treat errors as watcher-lifecycle faults and recreate watches after re-resolving known-folder roots.

🟠 crates/dm-windows/src/watcher.rs:159 — Multi-path and unclassified events can be silently discarded.
Scenario: A backend supplies several changed paths or `EventKind::Any/Other`; only the first path or no hint reaches reconciliation, violating fail-safe under-notification.
Fix: Emit every path and map ambiguous content events to `Changed` or `Overflow`.

🟠 crates/dm-windows/src/overlay.rs:73 — Elevated-helper execution waits forever without cancellation or timeout.
Scenario: The helper deadlocks or a shell/registry operation hangs; the calling command thread remains blocked indefinitely and shutdown cannot complete.
Fix: Use a bounded wait with cancellation, then report an indeterminate outcome without losing recovery state.

🟠 crates/dm-windows/src/wallpaper.rs:76 — `GetStatus` failure is recorded as “not a slideshow.”
Scenario: Status retrieval fails during the pre-first-apply snapshot; mutation proceeds with incomplete restore material.
Fix: Make snapshot capture strict and propagate status-read failures.

🟠 crates/dm-windows/src/wallpaper.rs:81 — Capture does not actually verify that enumerated monitors are attached.
Scenario: `GetMonitorDevicePathAt` returns a remembered detached display; it enters the snapshot and later makes whole-desktop restore fail or target stale topology.
Fix: Require a successful `GetMonitorRECT` before capturing a monitor.

🟡 crates/dm-windows/src/activity.rs:75 — The primary desktop-scoped drag/capture hook is absent. (VERIFY: by-design?)
Scenario: A browser-to-desktop drag leaves a browser class foreground, so the fallback reports idle and automation can repaint beneath the cursor.
Fix: Add the specified `SetWinEventHook` message-pump layer and combine it with the existing fallback.

🟡 crates/dm-windows/src/apply/url_shortcut.rs:15 — Rewriting normalizes all line endings and removes the original terminal newline.
Scenario: A mixed-ending or CRLF-terminated `.url` is styled; unrelated bytes change despite the claim that the rest of the file is preserved.
Fix: Parse with retained separators and patch only the owned value spans.

🟡 crates/dm-windows/src/apply/shortcut.rs:16 — Existence preflight collapses metadata errors into `AssetMissing`/`NotFound`.
Scenario: An unreadable asset or shortcut is reported as absent, hiding the actual access/integrity failure.
Fix: Replace `exists()` with `try_exists()` and propagate metadata errors.

🟡 crates/dm-windows/src/apply/file_wrapper.rs:21 — File preflight collapses metadata errors and non-file states into `NotFound`.
Scenario: Permission or device failure is treated as benign disappearance, obscuring a compromised apply path.
Fix: Use fallible metadata and distinguish absence, wrong type, and I/O failure.

CLEAN: crates/dm-windows/src/refresh.rs, crates/dm-windows/src/cmdline.rs, crates/dm-windows/src/topology.rs

## Pass I — dm-elevated

[🔴] crates/dm-elevated/src/main.rs:9 — Helper authenticity and elevation depend on nonexistent packaging, while the host resolves it beside a potentially user-writable per-user installation.
Scenario: attacker replaces `dm-elevated.exe` beside the Tauri executable -> the next `runas` prompt launches arbitrary attacker code as administrator.
Fix: install and ACL the signed helper under Program Files, embed `requireAdministrator`, and verify its canonical path and Authenticode identity before launch.

[🔴] crates/dm-elevated/src/guards.rs:55 — `--file` permits arbitrary UNC, device, protected, and reparse-resolved paths to be opened with administrator authority.
Scenario: `--file \\attacker\share\x.ico` triggers privileged network authentication, or a symlink to another user’s protected valid ICO copies its contents into world-readable ProgramData.
Fix: accept only local files beneath an authenticated staging root; reject UNC/device/reparse paths and verify the final opened handle’s path, owner, and file type.

[🔴] crates/dm-elevated/src/secure_dir.rs:103 — Existing directories are trusted from owner and reparse bits alone, without verifying their DACL or holding a stable directory handle.
Scenario: an Administrators-owned but Users-writable `DeskMakeover` directory passes validation -> a standard user plants state or swaps the directory after inspection, steering elevated file operations through attacker-controlled objects.
Fix: open without following reparse points, validate owner plus protected DACL/effective non-admin write denial on that handle, and perform/revalidate operations against the same directory identity.

[🔴] crates/dm-elevated/src/main.rs:21 — The privileged helper authenticates no caller or request provenance, making it a publisher-branded confused deputy. (VERIFY: by-design?)
Scenario: any local process invokes the signed helper with a valid custom ICO -> after a generic DeskMakeover UAC prompt, attacker-selected content is installed into a machine-wide Explorer setting.
Fix: bind each invocation to an authenticated, nonce-bearing request from the trusted installed host and reject direct or replayed command lines.

[🔴] crates/dm-elevated/src/guards.rs:40 — Claimed “full structural validation” accepts ICOs with unchecked DIB header size, compression, planes/bit depth, and directory-to-DIB dimension mismatches.
Scenario: a crafted payload keeps offsets and computed length consistent but sets invalid compression/header fields -> validation succeeds and Explorer repeatedly consumes the malformed machine-wide icon.
Fix: validate every supported ICO/DIB invariant or fully decode and re-encode into a canonical trusted format before publishing.

[🔴] crates/dm-elevated/src/args.rs:51 — The privilege-boundary parser silently ignores unknown arguments and converts an invalid `--style` into `Refined`.
Scenario: `apply-overlay --style attacker --unexpected value --file valid.ico` reaches the privileged registry write instead of being rejected as malformed.
Fix: parse an exact per-verb grammar returning `Result`, reject duplicates, unknown flags, missing values, surplus arguments, and non-whitelisted styles.

[🔴] crates/dm-elevated/src/overlay.rs:73 — The original registry value is snapshotted lossily as a `String`, and every read/type error is treated as “absent.”
Scenario: value 29 is `REG_EXPAND_SZ`, another registry type, unreadable, or literally `__absent__` -> apply proceeds and restore deletes or type-changes the original value.
Fix: use `get_raw_value`, distinguish only genuine not-found, and persist a tagged format containing key existence, registry type, and exact bytes.

[🔴] crates/dm-elevated/src/overlay.rs:96 — Restore ignores every `delete_value` failure and then destroys the sole recovery snapshot.
Scenario: deletion fails with access denied or an I/O fault -> helper removes `overlay-state.txt`, reports success, and permanently loses the restore anchor while the overlay remains.
Fix: ignore only not-found; propagate every other error and retain the snapshot until the registry postcondition is verified.

[🔴] crates/dm-elevated/src/overlay.rs:86 — Apply and restore are not serialized, allowing a cross-process race to erase the recovery anchor.
Scenario: restore reads state; apply observes it exists; restore reinstates the original and removes state; apply then writes DeskMakeover’s value -> the overlay is active with no snapshot, so future restore is a no-op.
Fix: hold a machine-wide mutex or equivalent durable lock across each complete apply/restore transaction.

[🔴] crates/dm-elevated/src/overlay.rs:95 — Restore performs no compare-and-swap check against the value DeskMakeover installed. (VERIFY: by-design?)
Scenario: another administrator or product changes value 29 after DeskMakeover applies -> DeskMakeover restore silently overwrites that newer value with its older snapshot.
Fix: record the applied raw value and restore only if the current value still matches it; otherwise report a conflict without deleting state.

[🟠] crates/dm-elevated/src/overlay.rs:92 — The privileged restore reads an unbounded, unversioned state file directly into memory and trusts all decoded text.
Scenario: filesystem corruption or a writable-directory misconfiguration produces a multi-gigabyte or forged state file -> the helper can exhaust memory or write attacker-selected text to HKLM.
Fix: cap the read to the registry maximum and validate a versioned, length-delimited, integrity-checked state record.

[🟡] crates/dm-elevated/src/overlay.rs:94 — Restore creates the `Shell Icons` registry key even when it did not exist before apply.
Scenario: the key was initially absent -> restore deletes value 29 but leaves a new empty HKLM key, violating exact restoration and zero-residue requirements.
Fix: snapshot key existence and conditionally remove only a helper-created key when it remains empty and unchanged.

[🟡] crates/dm-elevated/src/overlay.rs:100 — Restore removes only the snapshot and leaves all installed overlay ICO files behind.
Scenario: apply multiple styles and then disable or uninstall -> `*-overlay.ico` files remain under ProgramData despite the zero-residue contract.
Fix: after verified registry restoration, safely delete the fixed helper-owned ICO names and stale helper temp files.

[🟡] crates/dm-elevated/src/args.rs:44 — `std::env::args` requires Unicode strings and can panic before malformed Windows arguments are rejected.
Scenario: a command line containing an unpaired UTF-16 surrogate reaches the helper -> the elevated process aborts instead of returning the documented rejection exit code.
Fix: consume `args_os`, validate verb/options as `OsStr`, and convert only fields whose grammar requires Unicode.

[🟡] crates/dm-elevated/src/secure_dir.rs:159 — A successful owner query with an invalid owner pointer returns without freeing the allocated security descriptor. (VERIFY: by-design?)
Scenario: an unusual or corrupt filesystem security descriptor yields success with no usable owner -> the allocation leaks on the refusal path.
Fix: wrap the descriptor in RAII or free it on every post-call return path.

CLEAN: none.

## Pass J — src-tauri + bridge

[🔴] src-tauri/src/icon_host.rs:171 — Windows privileged-scope roots are always empty, despite the adjacent fail-closed requirement.
Scenario: A Public Desktop or ProgramData-backed item reaches version switch -> no root matches -> machine-wide/privileged content can be modified as if it were per-user.
Fix: Resolve both known folders during Windows startup and abort privileged mutation paths if either resolution fails.

[🔴] src-tauri/src/icon_host.rs:431 — `count` directly controls `Vec::with_capacity` without any upper bound.
Scenario: Webview invokes `icons_apply_baked_begin` with `count = 4_294_967_295` -> multi-gigabyte allocation attempt -> process abort/OOM.
Fix: Reject counts above the scan-derived maximum before constructing `IconApplySession`.

[🔴] src-tauri/src/icon_host.rs:449 — Chunks are buffered without count, per-item-size, or cumulative-byte enforcement.
Scenario: Repeated chunk calls submit oversized strings or more items than promised -> memory grows until commit, where validation happens too late -> process OOM.
Fix: Enforce expected count, per-master limit, unique slots, and a cumulative session byte cap while pushing each chunk.

[🔴] src-tauri/src/wallpaper_host.rs:96 — Wallpaper base64 is unbounded before full decoding/materialization.
Scenario: A hostile webview sends a multi-gigabyte `pngBase64` or compact decompression bomb -> IPC/string allocation and decoding exhaust memory; later screen decoding can also explode.
Fix: Cap encoded/decoded bytes and validate PNG dimensions before decoding or writing.

[🟠] src-tauri/src/commands.rs:74 — Heavy scan, decode, packaging, filesystem, SQLite, and COM commands are synchronous Tauri commands.
Scenario: A large desktop scan or icon commit runs through a synchronous invoke handler -> the webview/event loop stalls and the application appears hung.
Fix: Make heavy commands asynchronous and run blocking work via `spawn_blocking` or the dedicated STA executor.

[🟠] src/bridge/client.ts:24 — Shipped Tauri builds route `app.getInfo` to the browser mock because no Rust command handles it.
Scenario: Real installation boots -> reports version `0.0.0`, preview changelog, frontend-derived theme, and a schema version copied from the same frontend constant -> host/web drift check can never fail.
Fix: Implement/register/generate/dispatch a real `app_get_info` command and include it in `HANDLED`.

[🟠] src/bridge/tauri.ts:143 — `shell.openExternal` forwards arbitrary URL schemes despite the spec’s http(s)/mailto whitelist.
Scenario: Compromised web content calls the bridge with a `file:`, executable-associated, or other privileged scheme -> the OS opener handles unintended content.
Fix: Parse the URL, allow only `https:`, `http:`, and explicitly supported `mailto:`, and scope the Tauri opener permission likewise.

[🟠] src-tauri/src/icon_host.rs:543 — Overlay install errors and Declined/Failed outcomes are silently ignored after icons have committed.
Scenario: Icon writes succeed but elevation is declined or the overlay ICO is unavailable -> operation returns clean success while native arrows remain, potentially producing double marks and inconsistent UI.
Fix: Treat every non-Applied overlay result as degraded, return `ok:false` with a repair toast, and retain authoritative arrow state.

[🟠] src-tauri/src/icon_host.rs:182 — Arrow state persistence is non-atomic and all write failures are discarded.
Scenario: Overlay installation succeeds but marker writing fails or tears -> after restart Rust reports `native` while the machine-wide registry still hides arrows.
Fix: Persist atomically with error propagation and reconcile marker state against observed registry state at startup.

[🟠] src-tauri/src/icon_host.rs:128 — Transparent overlay ICO materialization is best-effort and non-atomic.
Scenario: Data directory is unwritable or a partial ICO remains after interruption -> later overlay installation fails only after icon mutation, with no reliable preflight.
Fix: Atomically materialize and validate the ICO during fallible host construction; abort startup or mutation if unavailable.

[🟠] src-tauri/src/icon_host.rs:627 — Full restore changes `keepNewIconsStyled` internally but emits no `settings-changed` event.
Scenario: Auto-format is enabled and the user restores originals -> SQLite becomes false, but frontend settings remain true until restart and display the wrong resident state.
Fix: Emit the updated settings DTO after the internal write and subscribe to the Tauri event in the TS bridge.

[🟠] src-tauri/src/wallpaper_host.rs:107 — Restore errors cannot represent “desktop changed but snapshot cleanup failed.”
Scenario: Platform restore succeeds, then snapshot deletion fails -> command rejects as if nothing happened, while the desktop has already reverted and frontend state remains stale.
Fix: Return a structured degraded result containing the post-operation backup state and force a screen refresh.

[🟠] src-tauri/src/icon_host.rs:935 — The advertised 32 MiB source-cache cap deliberately permits an unlimited current generation.
Scenario: A desktop contains thousands of high-entropy icons -> every current PNG is pinned -> cache grows to gigabytes and can terminate the process.
Fix: Bound scan item count/bytes or stream/lazily serve sources rather than pinning an unlimited generation.

[🟠] src-tauri/src/lib.rs:104 — Active-user-profile count is hardcoded to one on Windows.
Scenario: Multiple profiles exist -> frontend treats the machine-wide arrow disclosure as skippable and understates cross-user impact.
Fix: Enumerate active profiles on Windows and fail closed when the count cannot be established.

[🟠] src-tauri/src/wallpaper_host.rs:86 — A corrupt snapshot is reported as “no backup,” but subsequent apply remains permanently fail-closed on that same file.
Scenario: Snapshot JSON is truncated -> restore affordance disappears, every new apply errors on corrupt load, and UI exposes no repair path.
Fix: Surface explicit corrupt-backup state and provide a guarded quarantine/repair workflow.

[🟡] src-tauri/src/wallpaper_host.rs:119 — Decoded wallpaper cache validity depends only on the path string.
Scenario: The current image file is replaced or edited in place without changing its path -> repeated `getScreens` calls serve stale pixels indefinitely.
Fix: Include stable file metadata or a content fingerprint in cache validation.

[🟡] src-tauri/src/wallpaper_host.rs:165 — Sanitization is non-injective and can alias distinct monitor IDs. (VERIFY: by-design?)
Scenario: Two device IDs differ only in punctuation placement -> both share one protocol/cache key; stale-file pruning can also delete the other monitor’s baked wallpaper.
Fix: Use a collision-resistant hash of the full monitor ID in protocol and baked-file keys.

[🟡] src-tauri/src/icon_host.rs:868 — Real scans cannot emit the promised `ExecutableFile` refinement, and Windows Appx refinement is likewise absent.
Scenario: A desktop `.exe` is mapped as `RegularFile` -> frontend puts a launcher in the File bucket instead of App, applying the wrong participation/type policy.
Fix: Refine wire kind using the scanned path/source while retaining the operations-layer write mechanism.

[🟡] src-tauri/src/commands.rs:23 — Diagnostics still omit Windows build details and always return an empty host log tail.
Scenario: A host-side failure reaches the crash/support flow -> report contains only `windows` and no recent native errors, preventing useful diagnosis.
Fix: Capture bounded host error logs and return the actual Windows version/build.

[🟡] src/bridge/tauri.ts:161 — Only window-resize state is bridged; OS-theme, settings, toast, and host-error event streams are not connected.
Scenario: System theme changes while preference is `System`, or Rust changes settings internally -> subscribed frontend stores never receive the declared events.
Fix: Register Tauri listeners for every declared host event and add an OS-theme watcher.

[🟡] src-tauri/src/icon_host.rs:1027 — Failed comparison-image writes leave partial files behind and successful writes are reported before durability is assured.
Scenario: Disk fills during `write_all` -> a truncated PNG remains under the final filename; a sudden power loss after success can lose the reported export.
Fix: Write to a temporary file, flush/sync, atomically rename, and remove temporary files on failure.

[🟡] src/bridge/tauri.ts:177 — Window-state synchronization is launched without handling initialization rejection.
Scenario: Dynamic API import or `onResized` registration fails -> an unhandled promise rejection is produced and titlebar state silently stops updating.
Fix: Attach a terminal catch that logs/emits a host error and optionally retries.

[🔵] src/bridge/types.ts:307 — Handwritten DTOs already drift from generated bindings: `createdAt` is `number` here but `number | null` in generated Rust output.
Scenario: Rust/Specta contract evolves or produces its represented nullable case -> handwritten consumers compile under a false non-null guarantee.
Fix: Re-export generated bridge DTOs and restrict handwritten types to frontend-only assembled state.

[🔵] src/bridge/types.ts:21 — Rust bridge DTOs are duplicated manually despite the architecture banning hand-mirrored schemas.
Scenario: A Rust DTO changes -> bindings drift test remains green while `BridgeMethods` and handwritten interfaces silently retain the old shape.
Fix: Build `BridgeMethods` from generated command signatures/types and remove duplicated wire DTO declarations.

[🔵] src/bridge/tauri.ts:16 — The handled-method set and switch dispatch are separate manually maintained registries.
Scenario: A future method is added to only one -> it either falls through to mock in production or reaches the switch’s unhandled error.
Fix: Define one typed dispatch table and derive `tauriHandles` from its keys.

[⚪] src-tauri/src/lib.rs:86 — Windows composition comments still claim source extraction is deferred and placeholder-only after the extractor was implemented.
Scenario: Reviewers and release gates rely on stale source comments and misclassify current readiness.
Fix: Update the comment to the actual remaining Windows-verification status.

[⚪] src/bridge/types.ts:265 — Arrow-overlay comments still call the feature mock-only even though the Rust command and elevated path are wired.
Scenario: Maintainers may preserve or reintroduce mock-only assumptions around a live machine-wide operation.
Fix: Remove the obsolete cutover commentary and document the current failure/degraded contract.

CLEAN: src-tauri/src/devhost.rs, src-tauri/src/devhost_icons.rs, src-tauri/src/main.rs, src-tauri/build.rs, src-tauri/src/bin/gen-bindings.rs, src-tauri/tests/bindings.rs, src/bridge/generated.ts.

