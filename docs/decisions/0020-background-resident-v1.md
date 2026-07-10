# ADR-0020 — Background resident auto-format ships in v1.0

- **Status**: Accepted (owner, 2026-07-10)
- **Relates to**: ADR-0019 (replatform — the resident process is native Rust, no
  WebView), spec 06 §7 (superseded), **spec 07** (the normative behaviour spec).

## Context

The panel recommended deferring resident automation to v1.1 ("watcher is a
reconciliation engine; don't let an unattended writer front a trust-first product
before apply/restore is battle-tested"). The owner overruled the deferral and refuted
the restore objection on the merits: an automatic incremental format is just another
ledger entry — "把增量部分塞进之前记录的版本里，多了一个增量图标的存储而已". The
existing snapshot/history model extends naturally to incremental background applies;
restore is an algorithm problem, not a scheduling problem.

## Decision

1. **Resident auto-format is a v1.0 feature.** A single long-lived, non-elevated Rust
   process (tray, `--background` startup, per-user single instance, no WebView until
   the user opens the window; window close destroys the WebView again).
2. **Restore model = incremental ledger** (the owner's model, formalized):
   - Every background apply appends per-item incremental version entries to the SAME
     history/ledger the manual flow uses — one undo surface, no parallel bookkeeping.
   - Ledger entry: original fingerprint + original bytes/anchor, last-applied
     fingerprint, owned fields, content-addressed generated ICO
     (`<source-hash>-<style-hash>.ico`), transaction state.
   - Restore is per-item compare-and-swap: if the current state ≠ our last-applied
     fingerprint, the item is a visible CONFLICT (user/installer changes win), never a
     silent overwrite.
   - Background additions allocate hue against **pinned existing seeds** — a new icon
     never reflows the colours of icons already on the desktop; global hue rebalance
     happens only through an explicit foreground re-apply.
3. **Trust rails (non-negotiable, carried from the spec 06 §7 contract)**:
   default OFF · the first run is a proposal, silent mode only after that consent ·
   autostart is enabled only when the user opts into resident automation · every
   automatic change is an undoable history entry · turning it off never retro-reverts
   (and says so) · the type/kindPolicy section is the opt-out surface · per-icon
   「保留原样」 always wins.
4. **Privilege rails**: the background process NEVER pops UAC. Public-desktop and
   machine-level items are observed and queued as visible pending work; one batched
   UAC completes them when the user opens the window. No Windows service, no
   permanently elevated process.
5. **Scope v1**: user-desktop items of every bucket the user's saved style +
   `kindPolicy` already covers, with one carve-out — ordinary-file WRAPPING (which
   creates a companion `.lnk` and hides the original file, a structural filesystem
   change) defaults to the proposal queue rather than silent execution; a setting may
   promote it to silent for users who accept the trade.
6. **Watcher discipline** (spec 07 normative): events are hints, reconciliation is the
   truth — debounce + file-stability probe, source fingerprints beyond the `.lnk`
   bytes (target path/version, IconLocation state, package identity), self-write
   suppression (operation id + before/expected hash + window), buffer-overflow →
   full rescan, catch-up reconcile on startup/resume/Explorer-restart/app-update.

## Consequences

- Spec 06 §7 ("build later", C# TileRenderer renders in-process) is superseded by
  spec 07; the renderer is the native Rust core (ADR-0019).
- The `keepNewIconsStyled` setting un-hides in v1 behind the trust rails above.
- The kill-point/burst/overflow/OneDrive-redirect test battery in spec 07 §Verification
  is a v1 release gate for the resident path.
