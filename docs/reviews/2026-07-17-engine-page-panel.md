# 2026-07-17 — Engine page panel (algorithm story: README + landing + /engine/)

Owner request: surface that the looks are backed by an in-house pixel pipeline —
mention on the landing page and GitHub README, a dedicated deep-dive page telling
the algorithm story (technical + design, including fallback/exception
engineering), mandatory motion, and optionally the WASM build running real-time
in the browser.

Three isolated seats (narrative, interaction/motion, engineering credibility)
briefed with a code-verified fact sheet from `crates/dm-icon-core` /
`crates/dm-icon-wasm` / ADR-0016/0018/0019. Feasibility pre-checked:
`dm-icon-wasm` builds clean for `wasm32-unknown-unknown` — 249 KB raw / 88 KB
gzip; the app's `src/icon-wasm/` loader + worker protocol is portable.

## Seat 1 — narrative

Two directions: **A "one icon's journey"** (scrollytelling following the real
pipeline, 7 sections: portrait / separate / rescue / invariant / color / finish /
guarantee) vs **B "determinism whitepaper"** (organized by verifiable
guarantees). Recommended A: the WASM demo makes the pipeline literally watchable,
and one storyline serves consumers and developers at once. Delivered zh copy
samples (hero, section heads, rescue paragraph) and a "never write this" list
(no AI-speak, no absolutes, no adjective stacks, no exclamation marks).

## Seat 2 — interaction/motion

Two directions: **A pure scrollytelling** (pre-rendered frames, low cost) vs
**B "living pipeline"** (scrollytelling skeleton + WASM playground finale).
Recommended B: proof beats animation — "the slider you're dragging runs the same
Rust code the desktop app ships". Module-level motion specs: linear scanline
(1.2 s — scanners don't ease), stepped BFS flood, three-beat rescue with
critically-damped settle, hue-wheel push-apart without bounce, count-up badges.
Reduced-motion static states per module; mobile downgrades (auto-play once,
128 px tile, preset carousel). Anti-list: no particles, no spring overshoot, no
scroll-jacking, no fake loading, no neon/glitch.

## Seat 3 — engineering credibility (read the repo, verified all facts)

- Most viral facts: provable preview=result byte parity (1,487-icon corpus);
  one ~12 k-line pure-Rust core compiled to native + 250 KB WASM (the web demo IS
  the shipping pipeline); perception-first OKLab rescue.
- Do NOT headline: Otsu / IoU / "we use OKLab" alone / checked_mul / squircle as
  original (Figma 1:1 port — crediting it adds trust, claiming it invites HN
  takedowns).
- README: insert a new H2 before the existing "Architecture and tech choices";
  draft delivered ("One pixel core, computed the same every time" + 5 receipt
  bullets) and adopted near-verbatim.
- Receipts pattern: every metric deep-links to the exact source line
  (`forbid(unsafe_code)` → `dm-icon-core/src/lib.rs:19`, parity cert →
  `tests/parity_determinism.rs`); unlinked numbers read as padding.
- Honesty edits: "purpose-built, fully deterministic pixel pipeline" over
  "original algorithm"; scope byte-parity precisely; never claim "zero unsafe"
  globally (the WASM FFI shim is unsafe by nature, overflow-checked).

## Owner dispositions (2026-07-17)

| Decision | Choice |
|----------|--------|
| Page direction | Journey scrollytelling + WASM playground finale |
| Wording | 自研确定性像素管线 / "purpose-built deterministic pixel pipeline"; credit Figma + standard methods; no "original", no "AI" |
| Playground sources | Built-in sample set + local user upload (privacy note) |
| Route + nav | `/engine/`, nav 引擎 / Engine |

Spec landed in `docs/specs/10-website.md` §"/engine/ — the pixel engine page".

## v2 redirection (owner, 2026-07-18)

The shipped v1 scenes were owner-rejected: hand-drawn sample icons ("小学生水平"),
flat 2D overlays that failed to visualise the process, copy both too cute
("拆开给你看" reads like an ad) and too obscure. Owner directives:
1. Real, complex icons only — "随便拿一个真实的图标都好过你自己画一个".
2. three.js / Lottie-grade motion: layered 3D decomposition ("打散"), user-drag
   interactivity on every model, sudden collapse→burst choreography.
3. Copy reframed as in-house-technology confidence (car-engine analogy): the
   page presents 自研像素计算引擎 capability, plain language for ordinary users.
4. "启用顶级 Subagent 首席设计师去写代码" — the three.js scene module
   (`components/engine/three/scenes3d.ts`) was authored by a chief-creative-
   technologist subagent against a typed contract; the orchestrator integrated,
   fixed framing/anchor clamping, and added fine beats per owner review.

Execution: real-icon cast from the app's fixture pack (folder/pics/bin/thispc/
camera/mail/maps/panel), oracle mask drives the CUT split, rescue layer = real
render diff, all five scenes drag-orbitable with double-click replay. Owner
verdict on the round: 「现在这个版本已经进步很大了，给予认可」 with follow-ups
(no visible canvas clipping, finer beats, full interactivity) applied same-day.
