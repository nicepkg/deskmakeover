# 0002. One-Click Product Form Reset

**Status:** accepted — with two clauses amended: the **Dark-first default theme** is superseded by
[ADR-0010](0010-settings-page-i18n-icon-and-shape-expansion.md) / [ADR-0013](0013-v3-flat-light-redesign.md)
(default now follows the system, light-first), and the **no-resident-process** stance is superseded
by [ADR-0020](0020-background-resident-v1.md) (background resident ships in v1). The one-click
product form + reset decision stands.
**Date:** 2026-07-05

## Context

The MVP foundation shipped with a developer-grade UI: three engineering buttons
(Scan / Save Snapshot / Preview Apply Plan), raw C# enum values bound into user-facing
cards, no before/after comparison, no real apply action, and default WPF control chrome.
The owner judged the experience crude but could not articulate the exact fixes.

A four-expert design review (product philosophy, human-centered design, visual craft,
Windows platform design) was run against the current code and the approved spec. The
panel unanimously found:

- The UI exposes the internal operation pipeline instead of the user goal.
- The promised "one-click" action does not exist anywhere in the UI.
- The product's entire value (the visual transformation) is invisible: no before/after.
- The approved five-area navigation (Home / Icons / Layout / Restore Center / Settings)
  is engineering-module-driven, not user-mental-model-driven, and oversized for the
  actual user need.
- A product that sells continuous-corner beauty must itself look beautiful; the current
  shell does not.

The owner then resolved the four open direction questions.

## Decision

1. **Minimal one-click product form.** The app is a single main screen: an
   at-launch before/after preview, one primary action ("一键美化"), an ever-present
   one-click restore link, and a hidden settings flyout. The five-area navigation,
   batch selection, style presets, per-item filters, layout page, and restore center
   are removed from the product surface. Scanning, snapshotting, and plan building
   become invisible automation behind the primary action. The bet: the default style
   must be good enough that no tuning UI is needed.
2. **Dark-first theme.** Dark (warm charcoal) is the default stage because styled
   squircle icons read best on dark surfaces; a light theme and follow-system option
   remain available in settings. Both palettes are maintained.
3. **Shortcut-arrow removal stays inside the one-click flow.** It is the only
   privileged operation (HKLM + icon cache + Explorer refresh). The flow is:
   plain-language explanation card → single batched UAC prompt → on denial or policy
   block, all non-privileged styling still applies and the arrow step is skippable and
   retryable. It is not deferred to a later version and not hidden in settings.
4. **Chinese product name changes to 桌面美颜** (was 桌面整容大师). "整容" signals
   surgery/risk/irreversibility, which fights the core promise of full reversibility;
   "美颜" borrows the beauty-camera mental model that has reversibility built in.
   The English name DeskMakeover, repo name, and code identifiers are unchanged.

## Consequences

- `specs/01-product-architecture.md` UX and IA sections are rewritten; the previous
  five-area IA is superseded.
- A dedicated visual-language spec (`specs/02-visual-language.md`) becomes load-bearing:
  with no tuning UI, default rendering quality carries the product.
- The domain model keeps per-item operation plans (needed for journaling and restore),
  but no per-item selection UI exists in the MVP.
- Any future capability (AI icon generation, color filters, style packs) must either
  fold into the default one-click result or live in settings as an advanced option;
  it may not add steps to the primary flow.
- Two theme palettes must be designed and tested; high-contrast mode must fall back to
  system colors.
- Keeping arrow removal in the MVP keeps the elevated helper, registry writes, icon
  cache refresh, and antivirus false-positive exposure in scope for the first release;
  the trust flow and graceful degradation are therefore MVP-critical, not optional.
- Regular files are not wrapped by default; wrapping stays behind an explicit opt-in
  in settings (no silent wrapping, unchanged from ADR-0001-era safety rules).
