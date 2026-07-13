# Windows settings platform reference

This standalone crate contains Windows platform primitives plus the formal adapters into
`../reference-crate`. The path dependency exists only between these two isolated handoff crates;
neither is a production workspace member.

## Contents

- `src/model.rs`: platform-neutral raw registry, system-profile, and error types.
- `src/ports.rs`: `RegistryBackend`, `SystemProfileProbe`, and `RefreshBackend` ports.
- `src/reference_bridge/`: strict environment conversion, fresh runtime composition, and a
  logical-CAS registry adapter with required policy-state injection.
- `src/windows_backend.rs`:
  - `WinRegistryBackend` using `winreg 0.55` for exact raw CRUD and explicit WOW64 views;
  - `WindowsSystemProfileProbe` using `windows 0.61.3` plus 64-bit registry reads for
    build/revision, `DisplayVersion`, `EditionID`, `InstallationType`, SKU, workstation/client,
    OS/process architecture, user geography, package identity, and build cross-checking;
  - `WindowsRefreshBackend` for per-recipient-timeout `WM_SETTINGCHANGE`, shell-association hints,
    and guarded `ms-settings:` launch.

It contains no production tweak catalog or verification allowlist. Registry recipes, policy
precedence, verified build/region rows, and activation behavior belong in the transaction
reference only after per-switch Windows VM verification.

There is only one certification shape: `reference-crate::WindowsEnvironment`. The bridge requires
Windows 11 `10.0`, Client/workstation, UBR, canonical identities derived from DisplayVersion and
EditionID, `GetProductInfo` SKU, known geography, native/process architecture, and package-identity
kind. It canonicalizes all text used by exact comparison. Known EditionID/SKU pairs must agree; an
unknown nonzero pair maps to `Other` only when neither side impersonates a known pair. Native x86,
unknown architecture, impossible emulation pairs, missing fields, `ZZ`, and empty packaged
identities fail closed.

The concrete Windows probe requires `UBR`, `CurrentBuildNumber`, `DisplayVersion`, `EditionID`, and
`InstallationType` to exist in the 64-bit `CurrentVersion` key with their exact expected registry
types. Missing, empty, malformed, or type-mismatched values fail the complete profile probe; callers
must not substitute guesses or silently reuse a prior environment snapshot.

`ReferenceRuntimeProbe::with_unknown_lock_screen(WindowsSystemProfileProbe::default())` is the safe
composition for ordinary settings and re-runs the complete system probe on every call. Its
lock-screen background is deliberately `Unknown`, so lock-screen tips remain unwritable until a
separately verified `LockScreenBackgroundProbe` is injected. This reference does not guess a
lock-screen registry source.

`ReferenceRegistryBackend::new(WinRegistryBackend, policy_probe)` maps exact hive/view/key/value,
distinguishes missing keys from missing values, preserves known raw kinds/bytes, and implements the
transaction reference's logical compare-read-write contract. There is intentionally no default
`PolicyStateProbe`: every recipe needs its own GPO/MDM precedence implementation. The probe gates
only `Apply`; `Undo` always reaches native registry ACLs so policy state cannot suppress rollback,
restore, or recovery. The composed registry/runtime bridges are deliberately non-`Clone` and expose
no parts extractor, so the transaction engine cannot hand callers a second writable adapter around
the same registry.

## Deliberate production extensions

These are intentionally not guessed in this platform crate:

- **Per-setting policy/MDM detection.** The bridge enforces explicit injection, but every switch has
  a different GPO/MDM precedence chain. Production probes still require per-switch evidence.
- **Advertising ID effective verification.** Exact registry write/read-back is available, and
  `ms-settings:privacy-general` can be opened manually. A WinRT `AdvertisingManager` probe and the
  distinction between stored preference and effective personalization remain a production
  extension; this crate must not imply that disabling the ID reduces ad quantity.
- **Windows Web Experience Pack detection.** `GetCurrentPackageFullName` reports DeskMakeover's own
  identity only. Enumerating and versioning the Web Experience Pack needs a separate package
  inventory adapter and runtime test matrix.
- **CFR/internal feature-state detection.** There is no supported universal API for arbitrary
  Windows feature rollout state. Do not use ViVeTool/FeatureManagement internals; keep fragile
  recipes on discrete verified build families plus live per-setting probes.
- **Typed `SystemParametersInfoW` actions and registry watching.** Add them only for settings with
  a documented action or a resident reconciliation requirement. `SHChangeNotify` remains
  association-only, and `WM_SETTINGCHANGE` remains a hint rather than effect proof. For
  `HWND_BROADCAST`, `SendMessageTimeoutW` applies its timeout to each top-level recipient, so the
  worst-case total wait can exceed that value; callers must keep the per-recipient timeout small and
  run the hint off the UI thread.

## Verification

Run from this directory:

```console
cargo fmt -- --check
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
cargo check --offline --target x86_64-pc-windows-msvc
cargo clippy --offline --target x86_64-pc-windows-msvc -- -D warnings
```

The local `[workspace]` keeps this reference crate out of the repository root workspace and root
lockfile.

## Integration boundary

Copy the pure model/ports to `dm-domain` or another pure layer and the concrete implementations to
`dm-windows`. Keep WAL, CAS, exact restore, crash recovery, policy precedence, and composite-setting
orchestration in `dm-operations`.

Record missing prefixes as candidates before writing, then call `create_key` root-to-leaf and
durably promote each exact `Created` disposition. `OpenedExisting` is never cleanup ownership.
`write_value` opens only the already-materialized leaf. If native create/write succeeds but durable
confirmation does not, recovery restores the leaf and conservatively leaves the empty key. Shared
managed features inherit cleanup ownership; only the final owner may delete a confirmed empty key.
Registry has no atomic compare-and-set or delete-if-empty primitive, so external divergence must
stop restoration. The bridge serializes DeskMakeover writers at the transaction layer and relies
on double read-back, but an external process can still race the native write or empty-key deletion.

All standard Win32 registry type numbers 0 through 11 round-trip exactly, including reference
`Other(5)`, `Other(6)`, `Other(8)`, `Other(9)`, and `Other(10)`. The platform-neutral model can
represent a numeric kind above 11 so tests can prove rejection, but the concrete `winreg 0.55`
backend fails closed when reading or writing such an extension type. Every writable first-batch
recipe still accepts DWORD only.
