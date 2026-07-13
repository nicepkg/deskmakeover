# ADR-0013 — v3 "Premium Flat" redesign: light-first, bundled type, ceremony & gesture unification

- **Status**: accepted (owner interview 2026-07-08, 12 decisions D1-D12) — later **amended by
  [ADR-0021](0021-global-arrow-overlay-default.md)**: this ADR's native-arrow 60s penance gate is
  retired (the global transparent overlay is now the default; the rest of the welcome ceremony stands).
- **Supersedes / amends**: ADR-0012's dark-default and Segoe-stack typography; spec 02 v2
  chrome sections (rewritten as v3). Engine rendering law (WYSIWYG sections) untouched.
- **Evidence**: `docs/reviews/2026-07-08-ui-v3-premium-flat-panel.md` (panel run #2 on
  real screenshots — the final full-panel run for this artifact).

## Context

After the v2 "Quiet Material" redesign shipped to the web layer, the owner judged the
rendered app still not beautiful ("丑的要命") and issued a new brief: **flat,
white-leaning, Apple-flavored premium; dark mode follows the system; zh/en; responsive;
bundled free-commercial fonts; deep-but-legible customization**. Pre-release, zero
users — breaking chrome rewrites allowed. A five-seat isolated panel judged the real
pixels and converged on three inherited root causes (taupe dark-flip palette, YaHei-
fallback typography, a handful of load-bearing components/feel gaps).

## Decisions

1. **Theme (D3, D12)** — Default follows the system. The light theme is the design
   first-citizen: rebuild the neutral ramp light-first in OKLCH (cool/true-white, no
   warm taupe), then derive dark from it. Both themes ship at equal quality; current
   responsive breakpoints stay (≥1100 regular / <1100 compact / min 1024×700).
2. **Accent (D1)** — Coral `#FF6F5E` remains the only accent; blue/violet stay banned.
   Add an `ink-coral` (deeper, less orange) for large solid fills on white; selected
   states get border/ink reinforcement instead of wash-only.
3. **Typography (D2)** — Bundle **Inter** (Latin, variable, OFL) + **HarmonyOS Sans SC**
   (Regular 400 + Medium 500; license verified to permit embed/bundle/redistribute;
   attribution line added to About). Subsets ≈3-5MB total. First frame gates on
   `document.fonts.ready` (zero FOUT). Fallback chain → system CJK for rare glyphs.
   The desktop mirror's tile labels keep an OS-faithful stack (`--font-os-mirror`).
   Type ladder v3: caption 12/400 · body 13/400 · body-strong 13/500 · cardtitle 15/500
   · section 19/500 · display 26/500 (700 escape hatch). Latin display/section tracking
   −0.01em; CJK letter-spacing always 0. MiSans rejected on license risk.
4. **Customization IA (D5, D6)** — Shape row shows 6 curated chips
   (苹果/纯圆/三星/方块/水滴/无) + a 「更多形状」 fold for the remaining 7; all names
   Chinese-first. The filter axis keeps all four options (无/玻璃/像素/贴纸) — owner
   decision, PM's cut recommendation rejected.
5. **Wallpaper narrative (D7)** — 清晰度 (clarity rescue) becomes the module's hero;
   zones demote to a secondary, tool-gated capability. No feature add/remove.
6. **First-run wow (D8)** — After the first scan, the mirror auto-plays one skippable
   原样→美化 transformation (preview only; the real apply stays a human click).
7. **Apply ceremony (D9)** — Wire the stranded strings: first-apply consent sheet
   (what changes / what's untouched / one UAC), completion state with 「去看看桌面」,
   restore confirmation. Icons and wallpaper share the ceremony components.
8. **Gesture model (D10)** — Drag = pan on both canvases; wallpaper zone creation is an
   explicit tool state (crosshair mode, Alt+drag shortcut); Space does compare only and
   never hijacks focused-button activation.
9. **Feel contract (D11)** — Accepted in bulk: gesture-time DOM approximation for zone
   frost (reconcile on true frame — WYSIWYG holds at rest), latency-gated restyle cue
   (no instant 45% dim), CTA working shimmer + text crossfade, `pop` on all menus,
   reduced-motion completeness, macOS-true segmented (sliding thumb), elevated canvas
   stage (no pure-black letterbox), single-source motion tokens.
10. **Release (D4)** — No partial ship: icons + wallpaper + settings all land the v3
    redesign before the first public release.

## Amendments

- **2026-07-08 (owner): version narrative RESTORED** — supersedes ADR-0012's
  no-version-narrative rule. The identity card shows a clickable version line;
  it opens the in-app changelog dialog (per-locale entries over the bridge).
  The changelog auto-opens exactly ONCE per installed update, never on first
  install. The host's changelog data source + the real version number are
  restored on the Windows side in F8.
- **2026-07-08 (owner): one axis-glyph keyline + one 无 dialect** — every
  shape/filter/mark glyph draws exactly 16px on a 20px canvas (no optical
  exceptions); every axis's 「无」 sits FIRST wearing the slash-circle
  (dashed = auto, slash = none, never conflated). Axis rows render through
  the shared `SwatchPicker`. *Amended 2026-07-09 (owner legibility call):
  the authored 20/16 grid now renders at a 25px canvas — 20px ink — via one
  `GLYPH` constant; the uniformity law itself is unchanged.*
- **2026-07-08 (owner): the native arrow is gated** — it sits LAST in the
  mark row and picking it opens a 60-second penance sheet (verbatim owner
  copy, escalating stare captions, instant cancel). Presets never carry any
  mark (engine `NamedStyles` truth); preset thumbnails render badge-free.
- **2026-07-08 (owner): first-run welcome gate** — language roster
  (OS-setup style, self-labeled, right-set 继续) → editorial brand beat
  (two-column, BrandMosaic collage) → TWO innocent survey questions (native
  arrow ugly? third-party icons tidy?) judged only after the last answer;
  wrong answers route to the owner's verbatim send-off, the "go uninstall"
  bluff-call, and a hand-TYPED confession (paste refused) before entry.
  The survey face must never reveal it is a gate. Shown once
  (`dm.welcome.done`); a DEV-only flask menu resets all first-run states.
- **2026-07-08 (owner): copy law — no dashes** in any user-facing string
  (reads as AI text). All existing values purged; grep-check before adding
  strings.
- **2026-07-10 (owner): D10 gesture — wallpaper canvas REVERSED.** D10 above
  said drag = pan and zone creation is an explicit tool. Shipped behaviour
  (kept by owner call): on the wallpaper canvas a blank left-drag CREATES a zone
  directly; panning is middle-drag / compare-hold. The icons canvas keeps
  drag = pan. D10's "explicit create tool" no longer holds for wallpaper.
- **2026-07-10 (owner): D10 Space — stays a GLOBAL compare gesture.** The
  "never hijacks focused-button activation" clause does NOT apply here: the
  inspector is button-dense and a just-clicked control keeps focus, so letting
  a focused button eat Space would break compare exactly when it is used. Space
  is global compare (text inputs excluded); buttons activate via ENTER. Spec 02's
  generic Space-activates-button a11y clause is amended to match.
- **2026-07-10 (owner): layout is a RIGHT inspector**, not a left panel — the
  canvas sits left, a 280px (248px compact) inspector sits right. (Spec 02/03's
  "left 300px panel / compact drawer" language is superseded.)
- **2026-07-10 (owner): multicolour celebration confetti** — one first-apply-per-
  launch celebration (飘丝带 from both screen corners) is allowed a full festive
  palette; a reviewed single-file exception to the coral-only rule (persistent UI
  stays coral). The first-scan original→beautified reveal was TRIED then rolled back.

## Consequences

- Spec 02 is rewritten to v3 (chrome half); the token table's exact OKLCH values are
  finalized during the build's design phase and locked by the banned-color and
  contrast gates — the spec records the constraints and roles.
- Version narrative is BACK (see Amendments): the version line + in-app changelog
  ship; `Directory.Build.props` moves off `0.0.0` when the owner names the first
  public version (F8).
- The i18n string table gains ceremony/coach strings already present; stranded strings
  become referenced or get deleted at build end (no dead strings at release gate).
- Panel protocol: this artifact has consumed its 2 full-panel runs; future UI judgment
  comes from real-user evidence.
