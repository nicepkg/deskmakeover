# ADR-0021 — Global transparent shortcut-arrow overlay is the default; the 60s penance gate retires

- **Status**: Accepted (owner, 2026-07-10)
- **Supersedes**: ADR-0006 (adaptive native-arrow semantics), spec 01 "kept-original
  shortcuts render the classic Windows arrow", spec 06 §3.10 (native-arrow 60s gate),
  spec 02 ArrowGateSheet ceremony. **Amends**: ADR-0013 (welcome-gate item list — the
  ritual survives minus the penance sheet).

## Context

Two findings forced this decision to the owner:

1. **Spec vs code contradiction**: the shipped code already defaults to the aggressive
   path — `DesktopBakeService` writes the global overlay on every distinction
   ("every distinction writes the global overlay now") and bakes simulated arrows into
   kept items (`OverlayBadgeService`), while specs still promised "kept shortcuts show
   the classic Windows arrow" (native). The 2026-07-10 doc-sync sweep missed this.
2. **Windows hard limit** (verified): there is no supported per-shortcut API to remove
   the link overlay — `IShellLink::SetIconLocation` controls only the base icon. The
   only lever is the machine-wide `HKLM ...\Shell Icons\29` swap: a compatibility
   feature affecting shortcuts OUTSIDE the desktop and other users of the machine.

The owner's standing wish (stated twice) is that native arrows never appear: everything
on the desktop is redrawn.

## Decision

1. **Default ON**: v1 applies the global transparent overlay (one batched UAC) and
   redraws every desktop shortcut's ICO. The shortcut mark axis (Shadow/Halo/Ring/…)
   is baked pixel language; nothing depends on the native overlay.
2. **「保留原样」 semantics**: original subject pixels preserved + a DeskMakeover-baked
   classic arrow — visual continuity without the native overlay. The spec language
   "keeps the real system pixels" is corrected everywhere.
3. **The 60-second penance gate (ArrowGateSheet) RETIRES.** Its object — the native
   arrow — no longer exists in the product. The baked classic-arrow mark becomes an
   ordinary option with no ceremony. The rest of the welcome-gate ritual (survey,
   send-off, bluff-call, typed confession) is untouched owner brand ceremony.
4. **Transaction requirements** (port of `OverlayCommands` + hardening):
   - One-time snapshot of the original overlay value (including an explicit
     `__absent__` marker) before first modification; stored under ProgramData.
   - The transparent ICO ships full sizes (16/20/24/32/48/256, true 32-bit alpha),
     installed to `%ProgramData%\DeskMakeover\Shell\`; the registry NEVER points at a
     caller-supplied path (LPE guard).
   - Explorer refresh/restart is a consented, explained step — never a silent kill.
   - Post-Windows-update health check: if the value no longer matches, show a
     needs-repair state; re-apply only with consent (never silently fight the OS).
   - Disable, uninstall, and the emergency-restore path each restore the exact
     original registry state and all shortcut ICOs; zero residue.
   - Disclosure copy states plainly: machine-wide, affects shortcuts outside the
     desktop and other users on this PC.
5. **Elevated helper scope stays minimal**: the overlay apply/restore verb pair remains
   the only v1 privileged surface (plus the public-desktop queue when the user
   triggers it); no scope creep during the Rust port.

## Consequences

- Spec 01/02/06 lines updated in this commit; the ArrowGateSheet component and its
  copy are deleted from the web app during migration (freeze-then-delete with the TS
  pixel code — no new work invested in it).
- Items kept original while the overlay is disabled (feature off) show whatever
  Windows shows natively; the baked classic arrow applies only to styled/kept items
  under an active makeover.

## Amendment (2026-07-19) — the transparent overlay's pixels carry alpha 1, never 0

The v0.1.0 field incident (customer black-block reports): the all-alpha-0 transparent
overlay ICO renders correctly on live load, but Explorer serializes the system image
list (arrow slot included) into `iconcache_*.db`; on deserialize an all-zero alpha
plane trips Windows' legacy "no nonzero alpha byte ⇒ icon has no alpha channel"
heuristic, and the zero-RGB bitmap comes back as an OPAQUE BLACK arrow stamped over
every shortcut on the next boot. On-box A/B (2026-07-19) confirmed alpha=1 pixels
survive the round trip; the alpha-derived AND mask experiment (2026-07-16) is reverted
codec-wide for the same round-trip reason (`dm-icon-codec/src/ico.rs`). Binding rule:
**every transparent-overlay pixel ships (0,0,0,alpha=1) — imperceptible (0.4%) but
alpha-carrying in every consumer path — and every ICO frame keeps an all-zero AND
mask.** The overlay content hash is the install signature, so shipping this change
self-heals installed machines on their next apply (one overlay-reinstall UAC).
