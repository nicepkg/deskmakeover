# Windows settings Rust reference

This is a compile-tested, copyable handoff artifact, not a production
DeskMakeover module or workspace member. It demonstrates the safety boundary
for reversible Windows settings and contains typed first-batch recipe candidates.
The initial verification manifest intentionally makes every direct recipe
unwritable until a matching Windows VM certification is added.

The production implementation should provide:

- a `RegistryBackend` backed by `winreg` raw values;
- a `VerificationBackend` that implements the typed delayed/effect proofs;
- a durable `JournalStore` backed by SQLite or an fsync'd write-ahead log;
- a production SQLite/file-lock implementation of the associated RAII writer lease;
- a fresh environment fingerprint from `RtlGetVersion`, UBR, and canonical identities derived
  from DisplayVersion, EditionID, InstallationType, `GetProductInfo` SKU, workstation/client role,
  `GetUserDefaultGeoName`, native/process architecture, and package-identity kind.

None of those contracts may be inferred by the frontend. Standard support is a
list of bounded, discrete build families plus complete runtime fingerprints,
never one continuous numeric range or a Cartesian product of independently tested
dimensions. Empty profiles and an unbounded maximum UBR fail closed; Windows 11
build 22000 starts at UBR 1761 in the tests.

`../platform-crate` is the compile-tested composition seam. Its strict `TryFrom<SystemProfile>`
bridge canonicalizes every textual field and rejects missing revision/SKU/geography, non-client or
non-workstation installations, native x86, unknown or impossible architecture pairs, empty package
identity, and known EditionID/SKU mismatches. Unknown nonzero edition/SKU pairs remain `Other` and
still need their own exact certification row. `ZZ`, empty/unknown geography, and noncanonical
fingerprints never certify; ISO alpha-2 and three-digit Windows UN M.49 geography shapes are
accepted.

`src/first_batch.rs` types tier, evidence, primary mutations, existing-only
auxiliary mutations, policy guards, forbidden mutations, manual fallback, notes,
and effect-verifier requirements. `first_batch_catalog()` returns a validated
`FirstBatchCatalog`, not a lossy primary-only projection. Its resolver rejects
duplicate IDs, case-insensitive resource collisions, forbidden leaves, policy
guards, tier/certification mismatches, and missing effect verifiers. An auxiliary
leaf is selected only when it currently exists and its complete exact-environment
allowlist matches; all four initial auxiliary allowlists are empty.

Every resolved `SettingDefinition` carries a non-optional, finite-budget
`VerificationPlan`. The engine requires the verifier's pre-write setup hook to
produce a typed `VerificationReceipt`, validates it against both the effect and
transaction values, and persists it with the WAL before the first registry write.
Start records a nonempty known-Recent marker; Device Usage records the exact raw
snapshot of all seven `Priority` values. Apply, restore, and recovery reuse that
receipt for immediate raw read-back, bounded settle, delayed raw read-back, and the
typed feature proof. Recovery is explicitly `UnattendedRecovery`: a production
verifier must not display UI, wait for confirmation, or retry past the persisted
budget.

`Start_TrackDocs` is a globally forbidden invariant. Widgets, lock-screen status,
taskbar Widgets, and system tray remain guided/manual-only, have no mutations, and
have no effect verifier that could accidentally enter a writable plan.

`RegistrySnapshot` distinguishes `KeyMissing`, `ValueMissing`, and exact present raw
kind/bytes. The WAL records missing prefixes only as pre-write candidates. Each native
create/open disposition is obtained root-to-leaf and a separate leased journal mutation
promotes only `Created` prefixes to confirmed cleanup ownership. A crash after native
creation/write but before that confirmation deliberately restores the leaf and leaves the empty
key behind. Restore removes only confirmed, still-empty keys deepest first. The Windows
delete-if-empty operation has an unavoidable external TOCTOU window, so production
must retain a key whenever ownership becomes uncertain. Each recipe also states
`CreateAllowed` or `MustAlreadyExist`; auxiliary `IfPresent` leaves are never created.

Cleanup ownership is shared metadata, not exclusive feature ownership. A later managed feature
under an app-created key inherits every covering cleanup prefix and the pre-DeskMakeover
`KeyMissing` original. Restoring any owner retains shared keys and may still commit; only the last
owner removes an empty key. External values/subkeys with no remaining owner still stop cleanup and
leave the restore Prepared. `RegistryWriteIntent::Apply` performs policy/MDM gating;
`RegistryWriteIntent::Undo` deliberately bypasses that advisory gate so rollback, restore, and
recovery always attempt exact undo. Native ACL/I/O failure still leaves the transaction Prepared.

Safety properties demonstrated here:

- exact registry kind, bytes, and missing-value state are preserved;
- the associated `WriterLease` token is required by every journal transaction method;
- apply/restore/recovery hold one lease from the first journal read through every exit,
  while public inspections acquire their own consistency lease;
- `ApplyRequest` is constructible only from an `Inspection` and carries its complete
  `WindowsEnvironment` fingerprint—including canonical display/edition/install identities plus
  the SKU and workstation role;
  apply compares it exactly with both a fresh probe
  and the resolved recipe before any write;
- a prepared journal record exists before the first external write;
- candidate keys, confirmed-created keys, and shared cleanup ownership are distinct durable facts;
- external race-created keys are never promoted or deleted, while a write-to-confirmation crash
  leaks an empty key rather than claiming unsafe ownership;
- two or more managed features may share a created key and restore in any order;
- a Prepared entry is a write barrier until startup recovery finishes;
- journal prepare atomically compares the prior managed generation;
- journal records recipe version, complete environment fingerprint, verification plan,
  bounded budget, and typed receipt; the managed anchor retains the apply audit receipt;
- entry state plus managed anchor must commit in one durable database transaction;
- every write is compare-then-write and followed by immediate plus delayed raw verification;
- typed effect verification uses the same persisted receipt before apply, restore, or
  unattended recovery can commit;
- failed apply rolls back to the captured original;
- restore never guesses a Windows default;
- recovery aborts incomplete apply and finishes incomplete restore;
- same-recipe reapply is a no-op, while a recipe-version mismatch requires migration;
- restore inspection remains available even when the current feature is manual-only/unverified;
- policy-managed, manual-only, stale, and unverified environments never apply; policy changes do
  not suppress a later undo attempt;
- the recommended default set explicitly excludes advanced, manual-only, and
  device-usage settings, and preserves Start Recent.

The generic engine exposes no backend, journal, verifier, or runtime accessor and no parts
extractor. `fake_*` inspection, mutation, and extraction methods exist only for the concrete
in-memory test composition; they are fault-injection seams and must not be copied into a frontend
or Windows composition root.

Run with:

```text
cargo fmt --manifest-path docs/references/windows-settings-rust/reference-crate/Cargo.toml -- --check
cargo test --manifest-path docs/references/windows-settings-rust/reference-crate/Cargo.toml
cargo clippy --manifest-path docs/references/windows-settings-rust/reference-crate/Cargo.toml --all-targets -- -D warnings
cargo check --manifest-path docs/references/windows-settings-rust/reference-crate/Cargo.toml --target x86_64-pc-windows-msvc
cargo clippy --manifest-path docs/references/windows-settings-rust/reference-crate/Cargo.toml --target x86_64-pc-windows-msvc -- -D warnings
```
