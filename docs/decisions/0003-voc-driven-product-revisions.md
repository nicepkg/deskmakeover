# 0003. VOC-Driven Product Revisions

**Status:** accepted (partially supersedes 0002) — two clauses later reversed: the **default
shortcut mark** was reversed to **None** (owner 2026-07-07; see [ADR-0017](0017-per-type-distinction-system.md)
+ owner decree), and the **"no resident process / real-time watcher not before v1.1"** stance is
superseded by [ADR-0020](0020-background-resident-v1.md) / [ADR-0022](0022-m7-appearance-model-and-consent.md)
(resident auto-format ships in v1); and the **badge design** was superseded by
[ADR-0005](0005-distinction-shape-color-system.md) (then 0007/0008). The VOC three-state governance stands.
**Date:** 2026-07-05

## Context

The founder's launch video drew substantial audience feedback (Douyin + Bilibili,
archived off-repo as "VOC v1"). The dominant objection — highest-liked comments —
was functional: "without the arrow badge, how do I know it's a shortcut?" The
founder's own position agrees: distinction is fine, ugliness is the problem. The
VOC also surfaced demand for multiple style presets, an expectation that newly
added shortcuts stay styled ("switch, not one-shot action"), an active open-source
competitor harvesting the comment section, and many high-intent "please share"
comments — making time-to-first-release a real constraint.

A second expert-panel round (product, human-centered design, visual craft, Windows
platform) debated the conflicts with ADR-0002, and the owner resolved four
questions.

## Decision

1. **The primary control is a Makeover Switch, not a one-shot button.** On =
   style everything now and keep new shortcuts styled; off = complete zero-residue
   restore. The restore link and apply button of ADR-0002 merge into this one
   symmetric control. "Keep new shortcuts styled" ships without any resident
   process: a logon-triggered run-and-exit task plus a catch-up pass on app launch.
   Real-time desktop watching arrives no earlier than v1.1 as an explicit opt-in,
   default off, with visible exit/uninstall.
2. **The default badge state is a Refined Mark, not removal.** The ugly system
   arrow is replaced by a designed multi-size full-alpha overlay (frosted squircle
   chip + ribbon-fold mark): distinction information is preserved, one global file
   covers every shortcut including future ones, and deleting the registry value
   restores stock. Three states — refined mark (default) / clean no-mark /
   keep original — chosen visually from three real thumbnails, never via jargon
   radio buttons. A per-icon baked top-right premium badge is deferred to v1.0+.
3. **Presets ship as a filter bar, on a ladder.** v0.9 ships one excellent default
   (Apple continuous-corner). The rendering engine is parameterized NOW via a
   `StylePreset` value object (four axes: mask shape / tile strategy / badge style
   / color treatment) so presets are data, not code. v1.0 adds a same-screen
   filter bar: three live-preview pills (苹果圆角 / 极简描边 / 单色滤镜), zero
   sliders, zero numeric controls. Skeuomorphic preset is rejected.
4. **Code signing is a v0.9 entry ticket.** Start with an OV/individual
   certificate immediately (EV upgrade later), plus proactive Microsoft/360/火绒
   whitelist submissions. An unsigned HKLM-touching exe distributed to novices via
   viral traffic is a launch-killer (SmartScreen).

## Consequences

- ADR-0002's "arrow removal inside the one-click flow" is superseded by the
  three-state badge model with Refined Mark as default; "removal" remains one of
  the three states.
- ADR-0002's separate ever-present restore link is superseded by the switch's off
  position; restore stays one click away at all times.
- No resident process exists in v0.9/v1.0; anti-bundleware guarantees ("no
  background residence, local-only, off = clean exit, zero residue") become
  spec-level copy commitments.
- The renderer must accept `StylePreset` from day one; retrofitting parameters
  later would mean engine rework.
- Product tone rule (from VOC defensiveness analysis): the app never judges the
  user's desktop ("your desktop is ugly" framing is banned); the voice is additive
  ("give your desktop a beauty pass"), and the public narrative is "补上 Windows
  缺失的系统级兜底", not "fixing Microsoft's bad taste".
- The "save before/after image" share hook is promoted to a v1.0 must-have (the
  comparison image is the growth engine).
- Layout snapshot/restore stays silent best-effort and must never gate a release.
