---
updated: 2026-07-05
version: unreleased
branch: main
---

# State

## Active Work

- MVP foundation implementation is underway.
- Current foundation includes solution skeleton, domain models, icon-rendering primitives, read-only desktop scanning, snapshot persistence, dry-run operation planning, WPF preview grid, and elevated-helper stub.

## Last Done

- Owner approved the Phase 1 design.
- Owner approved [specs/01-product-architecture.md](specs/01-product-architecture.md).
- Existing PowerShell scripts under `D:\shells` were reviewed as behavior references.
- Expert sub-agent review completed for product, UX, visual system, and Windows Shell architecture.
- Local git repository initialized on `main`.
- Local .NET 10 SDK installed under `.dotnet/` for development use.
- Added fallback icon rendering, WPF preview cards, snapshot save UI, dry-run apply-plan UI, restore metadata collection, and URL shortcut icon writer scaffolding.
- Hardened safety gates so skipped items do not produce global operations, URL shortcuts require full original-content payloads, capture errors block apply plans, shortcuts require captured original icon locations, folder plans require complete folder restore metadata, and regular-file wrapping remains disabled until full wrapper restore mapping exists.
- Added Windows Shell Link COM adapters for `.lnk` icon-location reads, shortcut icon writes, and byte-exact shortcut restore scaffolding.
- Shortcut snapshots now capture original `.lnk` bytes plus original icon location/index before operation plans may mutate shortcuts.
- Added journaled non-privileged apply operations for `.lnk` and `.url` icon updates with snapshot-bound rollback and pre-apply target drift detection.
- Added generated `.ico` file storage and a non-privileged operation factory that binds plan steps to snapshot-backed shortcut operations.
- Verification: `dotnet test DeskMakeover.slnx` passed 57 tests; `dotnet build DeskMakeover.slnx` passed with 0 warnings and 0 errors.

## Next

1. Implement real icon extraction for `.lnk` and `.url` previews with fallback on extraction failure.
2. Execute non-privileged apply/restore for user desktop shortcuts behind explicit confirmation.
3. Add self-contained Windows publish and installer packaging.

## Blockers

- No blocker. GitHub CLI is not currently available in PATH, so no remote repository was created.

## Open Questions

- None for the MVP spec. Future commercial model, AI provider choice, and style-pack marketplace are intentionally out of MVP scope.
