# Build plan — v3 "Premium Flat" redesign

Governing: **ADR-0013**, **spec 02 v3**, review
`docs/reviews/2026-07-08-ui-v3-premium-flat-panel.md` (32 dispositions, D1-D12),
`docs/references/webview2-pitfalls.md` (F6 input). Owner standing orders: motion
plentiful and premium; customization maxed (layered, never cut); every UX dimension
to the extreme; release only when all three modules wear v3 (D4).

Dev loop: Mac `bun run dev` + mock desktop (`bridge/mock-desktop.ts`) for all design
iteration; `bun test` (banned-colour + unit) green at every phase gate. C# host
changes and real-host verification batch at the end on Windows (F6/F8). Files ≤500
lines; i18n via resx source (`scripts/dev/upsert-strings.py` on Windows — new strings
are staged in the generated TS with a `// PENDING-RESX` marker and reconciled in F8,
so the TS header rule is honoured).

> **Status (2026-07-08 checkpoint)**: F1-F5 built and owner-iterated (~50 local
> commits; scope grew mid-flight by owner decree: welcome gate, arrow penance,
> keyline unification, keymap-by-page, DEV menu, dash-free copy — all recorded
> as ADR-0013 amendments). F6's WEB-side belts are DONE
> (`lib/webview-hardening.ts` + checklist ticks in webview2-pitfalls.md);
> the host-side audit batches into F8. Remaining: D10 gesture remainder +
> zone-drag DOM fill, dark/zh regression evidence, F7 codex review, F8 Windows
> pass. Current truth lives in `docs/STATE.md`.

## Phases

- **F1 — Type + tokens (the two P1 roots).**
  Bundle Inter VF (Latin subset) + HarmonyOS Sans SC 400/500 (GB2312 subset) under
  `src/assets/fonts/`; `@font-face` block + preload + metric overrides +
  `document.fonts.ready` first-frame gate; `--font-os-mirror` token for tile labels +
  taskbar. Rebuild `index.css` light-first in OKLCH: cool/true-white neutral ramp
  (bg/raised/raised-hov/chip/t1-t3/hair), `--coral-ink`, neutral matte
  `--canvas-stage`, two-tier elevation (`--elev-soft`/`--elev-stage`), ladder v3
  (12/13/13-500/15/19/26, weights {400,500}), tracking rules, dark re-derived.
  Gate: component gallery screenshots both themes; contrast ≥4.5:1 t1/t2; gates green.

- **F2 — Primitive craft.**
  macOS-true `Segmented` (inset track + sliding white thumb, spring damping ~44,
  max-w 360); `Chip` press state + reserved-weight selection (no reflow) + wash+ink+
  border selected treatment; CTA `working` shimmer + label crossfade + synced ✓ pop;
  `pop` on ALL menus/popovers (per-anchor origin); motion tokens single-source
  (`lib/motion.ts` helpers + reduced-motion wrapper — no inline curves anywhere);
  toggle spring de-bounced; scrollbar + hairline device-pixel alignment.
  Gate: gallery interaction pass (motion visibly premium, reduced-motion degrades).

- **F3 — Icons module.**
  Curated shape row (苹果/纯圆/三星/方块/水滴/无) + 「更多形状」fold (7 more, all
  Chinese names, i18n staged); axis summary strips as label:value; latency-gated
  restyle cue (kill instant 45% dim); consent/done/restore ceremony components
  (shared, wire the stranded strings); first-run auto 原样→美化 wave (skippable,
  mirror-only); preview-anchor status line; per-icon ⋯ one-shot coach; preset cards
  no mid-word wrap; canvas stage reframe (elevated card, matte letterbox).
  Gate: full-module screenshots dark+light, regular+compact; ceremony E2E in mock.

- **F4 — Wallpaper module.**
  Clarity-first hero + IA reorder (zones demoted below clarity); gesture unification
  (drag=pan, tool-state crosshair create + Alt+drag, Space=compare only, pass-through
  on focused buttons); zone-drag DOM approximate fill (frost/tint + title wash 1:1,
  reconcile on true frame ≤150ms); zone list humanized (style swatch, no 7×12.5);
  snap pulse kept; compact ✕.
  Gate: drag feel check (no lagging frost in mock), screenshots ×4.

- **F5 — Settings + shell.**
  Inset list rows (label left / control right / hairline), macOS density, dedupe
  identity vs About, fonts attribution line (HarmonyOS obligation), segmented usage,
  rail label 11px→caption token, title bar polish; string unify 新图标自动美化.
  Gate: screenshots ×2 themes; density judged against macOS System Settings.

- **F6 — WebView2 hardening (Windows-side audit + patch).**
  Work `docs/references/webview2-pitfalls.md` 🔴 list against the existing host —
  AUDIT first (virtual host, kiosk settings, DPI, ProcessFailed, UDF, white-flash,
  occlusion flag, SharedBuffer reuse), patch gaps, document what was already in
  place. Front-end-only items (overscroll-behavior, hairline alignment, reduced-
  motion, preload) land in F1-F5 directly.

- **F7 — Adversarial review.**
  codex cross-vendor review via /multi-ai over the full diff (visual claims verified
  against screenshots, WYSIWYG law, gesture edge cases, a11y), fix or dispose.

- **F8 — Windows verify (owner-present).**
  resx reconciliation (upsert-strings + regenerate TS, drop PENDING markers, delete
  dead strings); `dotnet test`; real-host run: fonts render (no YaHei fallback), IME
  per-field, DPI 125/150, dual theme, publish.ps1 size check; then the standing
  owner-supervised live gates (unchanged).

## Testing law

Every phase: `bun test` + lint + banned-colour green; new behaviour gets unit tests
(hero states, ceremony gating, gesture state machine, motion reduced-variants);
screenshots are evidence, both themes, committed under
`docs/plans/evidence/2026-07-v3/`.
