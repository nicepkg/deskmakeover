# ADR-0017 — Per-Type Distinction System

- **Status**: Accepted (owner-disposed, 2026-07-10)
- **Owner**: Jinming Yang
- **Panel**: chief PM · chief UX · chief UI — three adversarial rounds, seat
  isolation round 1, cross-examination round 2, taxonomy battle round 3.
  Owner disposed every contested point.

## Context

The kind-split experiments (kindShapes, ADR-0016 D2) proved that SHAPE-coded
icon types make a crowded desktop legible — the owner called the effect
"清晰美观，非常实用". Users' twin complaints about earlier defaults framed the
goal: the all-white default was unfindable (「分不清，找不到应用」), the first
colour recipe was noisy (「眼花缭乱」). The owner proposed expanding the
Beautified-types area into a per-type styling system (five flat types
including 快捷方式, follow global/another-type/custom, free colour+filter
mixing per type). The panel stress-tested that proposal.

## Decision

**The feature is a TYPE DISTINCTION SYSTEM, not a per-type style editor.**
Findability is served by systematic difference (shape + saliency), not by
free mix-and-match.

### D1 — Type space: 4 buckets × shortcut modifier (mechanism semantics)

Buckets stay `App(程序) / Folder / File / System`. Shortcut is an ORTHOGONAL
modifier, never a fifth type (a folder-shortcut would make a flat five-type
enum ambiguous — unanimous). Composition is a pure function:
`style = base(bucket) + badge(isShortcut)` — base always comes from the
bucket, the shortcut layer only adds the mark badge (and the optional
uniform-shape override below).

- **Mechanism semantics** (owner override of the panel's 2:1 target-semantics
  vote): every shortcut (`Shortcut`/`UrlShortcut`/`AppxShortcut`) buckets to
  App regardless of what it points at. Zero resolution cost; the owner wants
  "一眼看出是快捷", not shape-shifting by target.
- **Bare executables join App** (unanimous): `.exe` on the desktop is a
  program in the user's mind — today it buckets with documents. Host (F8)
  classifies by extension (v1: `.exe` only; `.bat`/`.msi` deferred); the web
  mock flags fixtures now.
- **Bug fix**: `isShortcut` must include `AppxShortcut` — UWP shortcuts
  currently never receive the mark.

### D2 — Config model: sparse overrides + resolve

`base: ConfigDto` + `typeOverrides: Partial<Record<Bucket, {source: 'global'
| 'custom'; patch?: TypePatch}>>`. `resolve(bucket)` = base for `global`,
`{...base, ...patch}` for `custom`. No follow-another-type in v1 (live
cross-type subscription needs cycle detection and silently propagates edits —
unanimous cut; parked in v2 with copy-once and hue-bias alternatives).
`kindShapes: boolean` is DELETED — superseded by per-type shape.

### D3 — Per-type axis envelope

| Axis | Per-type? | Notes |
|---|---|---|
| Shape | ✅ | the validated findability axis (formalizes kindShapes) |
| Saliency (colorMode) | ✅ limited | `{Field, Mono, BlackWhite}` only — demote noise types to grayscale so target types pop (figure-ground separation). All four buckets may demote (owner; default all-Field) |
| Plate colour | ✅ bounded | owner override of panel hue-lock: fixed plate from ≤6 low-saturation swatches; a fixed-plate type EXITS the hue-spread pool (one hue authority per plate); Apps default stays per-icon Field derivation (never a default fixed plate — anti "blue wall") |
| Beautify on/off | ✅ | existing kindPolicy, absorbed into the type rows |
| Filter | ⛔ global | material mixing = the amateur tell, 3/3 hard veto |
| Original mode | ⛔ global | per-type Original islands read as un-beautified bugs; whole-type opt-out = the beautify toggle |
| Hue of Field derivation | ⛔ | always per-icon rim derivation + global hue-spread |

### D4 — Factory default: the saliency ladder (default IS the answer)

Zero-config resolution of both historical complaints:

| Bucket | Shape | Colour |
|---|---|---|
| App/程序 (incl. all shortcuts + exe) | Apple squircle | Field (Vivid) — brand colours pop |
| Folder | Bookmark | Field |
| File | Tile | Field |
| System | Circle | **BlackWhite — demoted to quiet grayscale** (Mono is a tinted ramp; grayscale is what recedes) |
| shortcut modifier | — | mark badge ON (replaces native arrow) |

Shape carries the type split (no colour noise); System demotion mutes the
background layer; Field keeps per-icon identity everywhere else.

### D5 — Panel structure: type accordion

- Shape section loses the One-shape/By-type segmented (owner order) — type
  switching lives ONLY in the type area.
- Beautified-types area becomes the TYPE ACCORDION: one row per bucket with a
  collapsed summary (名称 · 形状 · 显著性 · custom badge), expand-to-edit
  (reusing the global section controls), 「跟随全局 | 自定义」 two-state +
  reset, one row open at a time.
- **Scope feedback is a hard requirement** (UX): while a type row is
  expanded, the canvas highlights that type's icons and dims the rest —
  prevents "edited App thinking it was global" mode errors.
- Shortcut controls stay a dedicated cross-cutting area: mark (style/colour)
  + new toggle 「快捷方式统一形状」 (default OFF; when on, overrides every
  type shape for shortcuts, badge unchanged; type Shape sections show a
  ghost note while active).

### D6 — Rendering plumbing

`resolveTypeConfig(bucket)` feeds tileStyleKey (hash of resolved config),
bake, and preview identically. The hue-spread pool filters to icons whose
RESOLVED colorMode is Field and whose type has no fixed plate — the pool
shrinks gracefully, never fights a fixed-plate authority.

## Refuted owner proposals (recorded honestly)

- Five flat types (folder-shortcut paradox) → 4 buckets × modifier.
- Follow another type → v1 cut (v2 parking: copy-once / live chain / per-type
  hue bias over Field derivation).
- Per-type filter mixing → hard veto (material salad).
- Per-type assigned hue as the distinction mechanism → replaced by the
  saliency ladder; bounded plate colour survives as an opt-in with pool-exit
  (owner call), never a default.

## Consequences

- ConfigDto schema change (unreleased, no migration): `kindShapes` removed,
  `typeOverrides` added; presets updated; C# host re-port lands with F8
  (plus exe classification and the AppxShortcut mark fix).
- The new factory default changes the shipped look (shape ladder + System
  demotion) — designer acceptance required on the real pack before it locks.
- UX validation debt: a minimal timed find-target usability test (3 cluttered
  desktops × 2 presets × 5 users) is the evidence instrument if the default
  is ever contested again.
