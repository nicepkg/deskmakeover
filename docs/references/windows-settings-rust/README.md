# Windows 11 calm-settings Rust handoff

**Research date:** 2026-07-11  
**Scope:** DeskMakeover's first Windows 11 visual-noise and recommendation controls  
**Status:** implementation reference, not a shipping capability declaration

This directory is a handoff for the agent that integrates Windows settings into
DeskMakeover. It deliberately separates three questions that are often conflated:

1. Does Windows expose a user-facing setting?
2. Is there a stable public API or registry contract for changing it?
3. Has DeskMakeover certified that change on the exact Windows environment in front of it?

The answer to the first question does not imply either of the other two. The
[`reference-crate`](reference-crate/) implements the fail-closed transaction and
capability model. It is intentionally isolated from the production workspace.

## Executive decision

Keep both existing Windows dependencies:

- Use `winreg 0.55` for raw registry reads and writes, explicit 32/64-bit views, and exact
  preservation of standard Win32 registry type numbers `0..=11` plus bytes. Unknown extension
  types fail closed rather than being restored lossily.
- Use `windows 0.61` for real OS-version detection, edition and geography probes,
  package identity, bounded `WM_SETTINGCHANGE`, `SHChangeNotify`, future typed Advertising ID
  verification/registry notification adapters, and documented `ms-settings:` pages.

`windows-rs` is not a unified Windows Settings API. It cannot make an undocumented
CloudStore, package `LocalState`, or internal registry schema stable.

Do not add another production crate. The intended production split is:

```text
dm-domain/system-tweaks.rs
  setting IDs, environment snapshot, support/outcome states, raw restore anchors, ports

dm-operations/system-tweaks/
  catalog evaluation, SQLite WAL/ledger, apply/restore/recovery driver, host fakes

dm-windows/system-tweaks/
  winreg backend, RtlGetVersion/profile probes, refresh and Settings-page adapters

dm-contracts
  generated frontend DTOs

src-tauri
  composition root and commands only
```

Do not reuse `dm-domain::RegistryValue`: it is deliberately narrow and string-shaped for
Recycle Bin icons. General settings must retain the original registry kind and raw bytes.

The copyable handoff has two isolated crates:

- [`reference-crate`](reference-crate/) contains the complete first-batch descriptors, fail-closed
  resolver, capability fingerprint, WAL/managed-anchor contracts, reversible state machine, and
  mandatory delayed/effect verification port.
- [`platform-crate`](platform-crate/) contains compile-checked `winreg`/`windows-rs` primitives for
  raw registry CRUD, profile probing, refresh hints, Settings-page fallback, and formal adapters
  into the adjacent transaction reference.

They are intentionally not root workspace members. Copy their boundaries into `dm-domain`,
`dm-operations`, and `dm-windows`; do not add these reference crates as runtime dependencies.
The platform crate's local path dependency on the reference crate exists only to compile-check that
the handoff joins without dropping fields or inventing a second certification rule.

The joined reference path is:

```text
WindowsSystemProfileProbe
  -> strict SystemProfile -> WindowsEnvironment conversion
  -> ReferenceRuntimeProbe (fresh on every inspect/apply probe)
  -> FirstBatchCatalog resolver / SettingsEngine

WinRegistryBackend + explicit per-setting PolicyStateProbe
  -> ReferenceRegistryBackend logical CAS
  -> WAL / exact restore / delayed and typed effect verification
```

The default lock-screen adapter reports `Unknown`; lock-screen tips therefore stay unwritable until
a separately verified background-state probe is injected. No lock-screen registry value is guessed.
The registry adapter has no default “not managed” policy answer. Its native registry write and
delete-if-empty operations retain unavoidable external-process TOCTOU windows, even when the
transaction layer holds DeskMakeover's cross-process writer lease.

This handoff still does not provide a production SQLite `JournalStore`/writer lease, real typed
non-interactive/effective-state verifiers, a lock-screen background detector, Web Experience Pack
inventory, or a populated
Windows VM certification manifest. Those are required production extensions; a successful raw
registry read-back alone does not make any initial recipe writable.

## Evidence levels

- **A — Microsoft contract:** current Support UI, Policy CSP/ADMX, or the public Windows
  settings data reference.
- **B — Microsoft sample/current repository behavior:** strong implementation evidence,
  but not a compatibility promise for arbitrary builds.
- **C — community or reverse-engineered behavior:** exact lab allowlist only.
- **D — no stable programmable setter found:** guided operation only.

An A-level policy does not automatically make an adjacent HKCU preference an A-level write
contract. Every direct setter still receives delayed read-back and functional verification.

## First-batch capability boundary

| Product control | Tier | Implementation candidate | Important boundary |
|---|---|---|---|
| Search highlights | automatic candidate | `HKCU\Software\Microsoft\Windows\CurrentVersion\SearchSettings` / `IsDynamicSearchBoxEnabled=0` DWORD | B-level preference; official Pro+ policy is a management guard, not a value to overwrite |
| Search only local | advanced | `HKCU\Software\Policies\Microsoft\Windows\Explorer` / `DisableSearchBoxSuggestions=1` DWORD | Empty exact-environment allowlist initially; affects Explorer search suggestions too; EEA uses the official provider UI |
| Widgets feed | guided | Widgets board settings | No stable independent setter |
| Widgets open on hover | guided | Widgets board settings | No stable independent setter |
| Widgets badges | guided | Widgets board settings | No stable independent setter |
| Widgets announcements | guided | Widgets board settings | No stable independent setter |
| Start promotional recommendations | automatic candidate | `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced` / `Start_IrisRecommendations=0` DWORD | Write this value only |
| Keep Start Recent | invariant | Do not mutate `Start_TrackDocs` | It also controls Explorer Recent and Jump Lists |
| Lock-screen status None | guided | `ms-settings:lockscreen` | Public data reference marks `lockScreenStatus` as `Not used`; no setter found |
| Lock-screen tips/fun facts | advanced | `SubscribedContent-338387Enabled=0` and `RotatingLockScreenOverlayEnabled=0` DWORD | C-level; Picture/Slideshow only after lab proof; never claim Spotlight image/content separation |
| Notification tips | automatic candidate | `SubscribedContent-338389Enabled=0` DWORD | `SoftLandingEnabled=0` may be changed only when the value already exists and the exact build is certified |
| Windows welcome experience | automatic candidate | `SubscribedContent-310093Enabled=0` DWORD | Future prompts only; no forced host restart |
| Finish device setup | automatic candidate | `UserProfileEngagement\ScoobeSystemSettingEnabled=0` DWORD | Do not disable ordinary toast notifications |
| Settings suggested content | automatic candidate | `SubscribedContent-338393Enabled=0` DWORD | Auxiliary `353694/353696/353698` values are existing-only, exact-build additions |
| Device Usage recommendations | advanced | Seven `CloudExperienceHost\Intent\*\Intent=0` DWORD values plus `OffDeviceConsent\accepted=0` | Empty exact-environment allowlist initially; preserve `Priority` rather than normalizing it |
| Explorer sync-provider notifications | automatic candidate | `Explorer\Advanced\ShowSyncProviderNotifications=0` DWORD | Can suppress legitimate provider education/status notices too |
| Taskbar Search | automatic candidate | `Search\SearchboxTaskbarMode=0` DWORD | Hides the entry, not Win+S/Search itself; policy enum uses different numeric values |
| Taskbar Widgets | guided (unconditional) | Open `ms-settings:taskbar` | `TaskbarDa` is observational compatibility evidence only; on serviced builds where UCPD taskbar protection is present and active, third-party writes can be rejected or reverted, so the app never writes it or disables/bypasses UCPD |
| Taskbar Task View | automatic candidate | `Explorer\Advanced\ShowTaskViewButton=0` DWORD | Hides the entry, not Win+Tab |
| System tray entries | guided | `ms-settings:taskbar` | Hardware/app-specific values are dynamic; no single stable switch |
| Advertising ID | automatic candidate | `AdvertisingInfo\Enabled=0` DWORD | Copy: “reduce personalized tracking,” never “reduce the number of ads” |

“Automatic candidate” means the recipe is implemented and testable. It does not mean the
shipping verification manifest may be populated without a Windows VM run.

## Exact registry recipes

All locations below use the 64-bit Windows OS view unless a Windows lab proves otherwise.
Every write is per-user and should not request elevation.

### Search highlights

```text
Target:
  HKCU\Software\Microsoft\Windows\CurrentVersion\SearchSettings
  IsDynamicSearchBoxEnabled  REG_DWORD  0

Management guard:
  HKLM\SOFTWARE\Policies\Microsoft\Windows\Windows Search
  EnableDynamicContentInWSB
```

The official policy applies to Windows 11 build `22000.1761` and later, or build `22621`
and later, on Pro/Enterprise/Education/IoT. Home has a user UI but not that policy
entitlement. If web Search is disabled, highlights can already be inapplicable.

### Search only local

The consumer workaround is:

```text
HKCU\Software\Policies\Microsoft\Windows\Explorer
DisableSearchBoxSuggestions  REG_DWORD  1
```

It is not the same contract as the Enterprise/education policy
`ConnectedSearchUseWeb=0` under `HKLM\...\Windows Search`. Do not write HKLM policy keys
from the consumer app. Do not use legacy `BingSearchEnabled`, `CortanaConsent`, or
`AllowCortana` as current Windows 11 implementations.

The consumer workaround stays disabled until an exact tuple has passed a functional nonce
query with no web-result affordance:

```text
major/minor + build + UBR + canonical DisplayVersion + canonical EditionID + normalized edition
+ InstallationType + GetProductInfo SKU + workstation/client role + geography
+ native architecture + process architecture + packaged/unpackaged context
```

EEA devices use Microsoft's supported Search-provider settings instead of this workaround.

### Start recommendations

```text
Write:
  HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced
  Start_IrisRecommendations  REG_DWORD  0

Never write as part of this feature:
  Start_TrackDocs
```

`Start_TrackDocs=0` hides recommended files in Start, recent files in File Explorer, and
Jump List items. `HideRecommendedSection` is also too broad. A functional verifier must prove
that a known recent file remains while the specific category controlled by “Show recommendations
for tips, shortcuts, new apps, and more” is disabled. It must not claim that every promotional or
account-related surface in Start disappears.

### Lock-screen tips

Advanced recipe only:

```text
HKCU\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager
SubscribedContent-338387Enabled       REG_DWORD  0
RotatingLockScreenOverlayEnabled      REG_DWORD  0
```

The public UI exposes fun facts for Picture/Slideshow, but Microsoft does not publish a
write API for the data-model `funItems` field. Windows Spotlight is explicitly a bundle of
rotating images plus tips, tricks, and notifications. Never present this recipe as a
supported “keep Spotlight images but remove its content” switch.

### Notifications and suggested content

```text
HKCU\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager
SubscribedContent-338389Enabled       REG_DWORD  0  # tips/suggestions
SubscribedContent-310093Enabled       REG_DWORD  0  # welcome experience
SubscribedContent-338393Enabled       REG_DWORD  0  # Settings suggested content

HKCU\Software\Microsoft\Windows\CurrentVersion\UserProfileEngagement
ScoobeSystemSettingEnabled            REG_DWORD  0  # finish setup
```

Existing-only additions, never created blindly:

```text
ContentDeliveryManager\SoftLandingEnabled
ContentDeliveryManager\SubscribedContent-353694Enabled
ContentDeliveryManager\SubscribedContent-353696Enabled
ContentDeliveryManager\SubscribedContent-353698Enabled
```

Do not use `PushNotifications\ToastEnabled=0`; it disables normal notifications.
Policies under `HKCU/HKLM\Software\Policies\Microsoft\Windows\CloudContent` are
management guards. Never delete them to make the Settings UI editable.

### Device Usage

Advanced all-off recipe:

```text
HKCU\Software\Microsoft\Windows\CurrentVersion\CloudExperienceHost\Intent\<name>
Intent  REG_DWORD  0

<name> = creative, business, developer, entertainment, family, gaming, schoolwork

HKCU\Software\Microsoft\Windows\CurrentVersion\CloudExperienceHost\Intent\OffDeviceConsent
accepted  REG_DWORD  0
```

Preserve every `Priority` value. `accepted=0` is valid only when the composite operation
turns all categories off. This feature reduces category-based recommendations; it does not
disable telemetry or all ads.

### Explorer and taskbar

```text
HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced
ShowSyncProviderNotifications  REG_DWORD  0
ShowTaskViewButton             REG_DWORD  0

HKCU\Software\Microsoft\Windows\CurrentVersion\Search
SearchboxTaskbarMode           REG_DWORD  0
```

The Pro+ machine policy `SearchOnTaskbarMode` uses a different enum from
`SearchboxTaskbarMode`; never share a Rust enum between them. Taskbar Widgets is always guided:
`TaskbarDa` is observed compatibility evidence, not a recipe. On serviced profiles where UCPD
taskbar protection is present and active, writes can be rejected or reverted; DeskMakeover never
writes it and never disables or bypasses UCPD.

### Advertising ID

```text
Target:
  HKCU\Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo
  Enabled  REG_DWORD  0

Management guard:
  HKLM\Software\Policies\Microsoft\Windows\AdvertisingInfo
  DisabledByGroupPolicy
```

If the policy value exists, show “Managed by your organization” and do not delete it.
When disabled, `AdvertisingManager.AdvertisingId` should be empty. Re-enabling creates a
new ID; the prior ID is not restored. Child accounts cannot enable it.

## Guided-only surfaces

These are product features, but not direct registry recipes:

- Widgets feed, hover, badges, and announcements: open Win+W and explain the exact current
  UI route. `AllowNewsAndInterests=0` disables the whole Widgets experience and is not a
  granular-news substitute.
- Lock-screen status None: open `ms-settings:lockscreen`.
- Widgets taskbar entry: always open `ms-settings:taskbar`; the app never attempts a direct
  `TaskbarDa` write.
- System tray: open `ms-settings:taskbar` and enumerate only entries Windows exposes for
  this device.

Do not edit Widgets package `LocalState`, CloudStore, ESE/SQLite stores, or internal feature
flags. Do not automate Settings by screen coordinates.

## Version and capability gate

The current public release table on the research date is:

| Windows 11 version | Build | Current UBR | Treatment |
|---|---:|---:|---|
| 23H2 | 22631 | 7219 | Home/Pro unsupported; Enterprise/Education only |
| 24H2 | 26100 | 8737 | Separate certification branch |
| 25H2 | 26200 | 8737 | Separate certification branch |
| 26H1 | 28000 | 2340 | New-device platform; never infer support from 24H2/25H2 |

Never implement `build >= 26100`. Use discrete build families and fail closed for unknown
builds. Every family requires lower and upper UBR bounds plus a complete runtime-profile
allowlist; dimensions are not combined into untested Cartesian products. The initial lab manifest
is empty for every B/C direct write.

Probe and persist:

- `RtlGetVersion` major/minor/build;
- 64-bit-view `UBR`, `DisplayVersion`, `EditionID`, and `InstallationType`;
- `GetProductInfo` edition/SKU;
- `GetUserDefaultGeoName` geography;
- process and OS architecture;
- relevant component/package versions, especially Windows Web Experience Pack;
- package identity/registry virtualization assumptions;
- domain/AAD/MDM and controlling policy state;
- UCPD presence and observed behavior.

Cross-check `RtlGetVersion` build against the registry. A mismatch, missing UBR, unknown SKU,
unknown geography/architecture, untested package context, future build, or changed component
schema is `Unverified`, not “probably supported.” Re-probe immediately before mutation. A feature
update invalidates the old certification and must not auto-replay prior tweaks.

The bridge accepts only canonical ISO alpha-2 geography or a strict three-digit Windows UN M.49
shape; `ZZ`, empty, and `unknown` fail closed. Known EditionID/SKU pairs are checked together
(`Core` 101 / `CoreN` 98, `Professional` 48 / `ProfessionalN` 49, `Enterprise` 4 /
`EnterpriseN` 27, `Education` 121 / `EducationN` 122). An unknown nonzero pair maps to `Other`
only when neither half masquerades as a known identity, and it still requires a dedicated exact
certification row.

Search-only-local is deliberately stricter: it accepts only canonical ISO alpha-2 non-EEA
geography. Every three-digit UN M.49 value, including `276`, `156`, and `001`, is inapplicable and
fails closed even when the wider environment bridge can represent it for other settings.

## Transaction and restore contract

1. Validate the frontend request against a compile-time catalog ID. Never accept a registry
   path from the webview.
2. Resolve the complete descriptor. Reject tier/rule mismatches, policy guards, forbidden leaves,
   resource collisions, missing effect verifiers, and auxiliary values without both current
   presence and exact-environment certification.
3. Re-probe environment, component presence, policy, and registry value shape.
4. Snapshot hive, registry view, key/value presence, raw type and bytes, recipe version,
   environment fingerprint, and typed verification plan before any external write.
5. Before the first write, obtain and persist the typed `VerificationReceipt`: Start needs a known
   Recent marker, while Device Usage needs the exact raw snapshot of all seven `Priority` values.
6. Acquire a cross-process writer lease covering inspect through terminal commit. In the same
   SQLite transaction, compare the prior managed generation and durably prepare the full WAL entry.
7. Registry has no atomic compare-and-set; perform a
   logical CAS with a tight pre-read/write/read-back window and report the residual TOCTOU
   limitation honestly.
8. Verify raw equality immediately, wait, verify again, then run a feature-specific effect
   verifier. A matching registry value is not proof that Start/Search/Widgets reloaded it.
9. On apply failure, roll back every owned leaf in reverse order. Continue after individual
   rollback failures and retain an incomplete transaction for startup recovery.
10. On restore, continue forward toward the true original. If the current value differs from
   DeskMakeover's recorded last-applied value, report an external conflict and do not overwrite.
11. Restore an originally absent value by deleting it. Remove app-created keys only in reverse
   order, only when still empty; never call `delete_subkey_all`.
12. Never silently kill Explorer, SearchHost, StartMenuExperienceHost, or Widgets. Report
    activation pending and offer a documented Settings/sign-out route where necessary.

Apply recovery aborts an incomplete apply back to its originals. Restore recovery finishes an
incomplete restore forward to those originals. Both recovery directions re-run the persisted
receipt with an explicit `UnattendedRecovery` mode and a finite settle/attempt budget; they never
open UI, wait for confirmation, or retry without bound. Both re-run delayed-read and effect proof
before changing the journal terminal state. A recipe or
verification-plan change requires a version bump plus explicit migration or restore-before-reapply;
it must not silently orphan leaves from the prior recipe.

## Refresh boundary

- `SHChangeNotify(SHCNE_ASSOCCHANGED, ...)` is appropriate for icon/association Shell work;
  it is not a universal Settings refresh or success signal.
- Use `SystemParametersInfoW` only when Microsoft documents the matching SPI action.
- A bounded `SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, ...)` may be a per-recipe
  hint. Modern hosts can ignore it or time out.
- `ShellExecuteW` opens a documented `ms-settings:` fallback; success is legacy
  `INT_PTR > 32`. It cannot set or inspect a toggle.

## Required Windows lab matrix

At minimum, certify clean snapshots of builds `26100.8737`, `26200.8737`, and
`28000.2340` independently across:

- Home, Pro, and Enterprise where applicable;
- CN, US/non-EEA, and one EEA geography;
- x64 and ARM64 where shipped;
- standard and administrator users;
- unmanaged, Group Policy-managed, and MDM-managed states;
- packaged and unpackaged DeskMakeover builds;
- UCPD present/active; confirm Taskbar Widgets remains guided and `TaskbarDa` remains protected.

For each candidate: inspect → apply → immediate raw read → delayed raw read → UI/effect
verification → sign-in/reboot if declared → restore → byte-for-byte comparison. Also test
policy conflicts, access denied, unexpected value types, missing components, a feature update,
external edits after apply, and process death at every journal/write/verify/commit point.

## Research confidence

- **`winreg` + `windows-rs` split: ★★★★★ (15+ API/repository sources plus two MSVC
  cross-checks).** The remaining risk is runtime behavior, not Rust API availability.
- **Start Recent/promotion separation: ★★★★★ (three Microsoft sources).** The values and
  cross-surface side effect are now public; the build-specific visual result still needs VM proof.
- **Automatic-candidate HKCU recipes: ★★★★☆ (Microsoft data references/samples plus two
  maintained community implementations).** Semantics are well corroborated; compatibility is not
  contractual for every servicing revision.
- **Search-only-local consumer workaround: ★★☆☆☆ (three corroborating implementations, no
  consumer Windows Search contract).** Exact environment and functional-query proof are mandatory.
- **Granular Widgets/lock-status/tray automation unavailable: ★★★★★ (current Support, Policy CSP,
  settings reference, and negative API/schema searches).** A future Microsoft setter can change
  this conclusion; until then GuidedOnly is the high-confidence result.
- **Direct lock-screen tips and Device Usage writes: ★★☆☆☆ (two maintained implementations plus
  public read semantics).** They remain advanced with an initially empty allowlist.

## Primary sources

- [Windows 11 release information](https://learn.microsoft.com/en-us/windows/release-health/windows11-release-information)
- [Windows 11 settings reference](https://learn.microsoft.com/en-us/windows/apps/develop/settings/settings-windows-11)
- [Common Windows settings reference](https://learn.microsoft.com/en-us/windows/apps/develop/settings/settings-common)
- [Search Policy CSP](https://learn.microsoft.com/en-us/windows/client-management/mdm/policy-csp-search)
- [Start Policy CSP](https://learn.microsoft.com/en-us/windows/client-management/mdm/policy-csp-start)
- [Experience Policy CSP](https://learn.microsoft.com/en-us/windows/client-management/mdm/policy-csp-experience)
- [Privacy Policy CSP](https://learn.microsoft.com/en-us/windows/client-management/mdm/policy-csp-privacy)
- [NewsAndInterests Policy CSP](https://learn.microsoft.com/en-us/windows/client-management/mdm/policy-csp-newsandinterests)
- [Windows Search providers in the EEA](https://learn.microsoft.com/en-us/windows/apps/develop/search/search-providers)
- [Current Widgets support](https://support.microsoft.com/en-us/windows/experience/personalization/stay-up-to-date-with-widgets-in-windows)
- [Current lock-screen support](https://support.microsoft.com/en-us/windows/experience/personalization/customize-the-lock-screen-in-windows)
- [Current taskbar support](https://support.microsoft.com/en-us/windows/experience/personalization/customize-the-taskbar-in-windows)
- [Microsoft WindowsDeveloperConfig examples](https://github.com/microsoft/WindowsDeveloperConfig/blob/366712847217e840103ffc8e38c4467fadb24a1d/windows-dev-config/dev-config.winget)
- [Microsoft HOBL notification mappings](https://github.com/microsoft/HOBL/blob/97591354ddf6925eb0d9dd92efe5f65f3888734b/scenarios/windows/system_prep.py#L67-L76)
- [Microsoft winget-dsc taskbar values](https://github.com/microsoft/winget-dsc/blob/cd4f1e30be8a08667f7309ef8df8dc9abf65c591/resources/Microsoft.Windows.Developer/Microsoft.Windows.Developer.psm1#L330-L359)
- [WindowsMize Device Usage implementation](https://github.com/agadiffe/WindowsMize/blob/7a3d5a2874bddd8ee97ed96132613dccb8340207/src/modules/settings_app/personnalization/public/Set-DeviceUsageSetting.ps1)
- [UCPD analysis](https://binary.ninja/2025/03/25/default-browser-upcd.html)
- [Sophia Script UCPD/TaskbarDa evidence](https://github.com/farag2/Sophia-Script-for-Windows/blob/02ad864539b6e8dd93233033bdfb3c70014cab08/CHANGELOG.md#5190--670--07102024)
- [RtlGetVersion](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-rtlgetversion)
- [Alternate registry views](https://learn.microsoft.com/en-us/windows/win32/winprog64/accessing-an-alternate-registry-view)
- [WM_SETTINGCHANGE](https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-settingchange)
- [ShellExecuteW](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecutew)
