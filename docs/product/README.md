# product/ — product-studio design docs (NOT the capability source of truth)

This folder holds product-design artifacts from the design phase (a `/product-studio`-style output):
finalized design specs, owner-curation records, and executed design notes. **The living capability
truth is `docs/specs/`** — when a doc here disagrees with a spec/ADR, the spec/ADR wins. Each doc's
own Status header states where it sits in its lifecycle:

- **`two-axis-colour-spec.md`** — Finalized normative design for the 主体×底板 colour model (ratified
  into ADR-0018 + spec 06). Reference it, but spec 06 is the living truth.
- **`preset-collection-v2.md`** — ✅ SHIPPED + ACCEPTED historical curation record (the seven presets
  landed; see journal).
- **`multi-screen-wallpaper.md`** — ✅ EXECUTED historical design record (multi-monitor + switcher
  shipped; Windows runtime `[WINDOWS-VERIFY]`).

Dated design reviews / panel records live in `docs/reviews/`; the engineering specs live in
`docs/specs/`.
