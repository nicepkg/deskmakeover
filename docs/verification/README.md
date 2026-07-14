# verification/ — owner-supervised live-run checklists

This folder holds the **owner-supervised** verification runbooks: the checklists a human runs to
confirm the irreversible, real-desktop operations (icon bake, wallpaper apply, resident audit, calm
writes) actually work. These **verification runs** are never auto-triggered — a human clicks,
watches, and confirms. (This is about the verification runbook, not a blanket product rule: the
shipped **resident auto-format** feature does auto-apply, but only after opt-in, on new icons, via
batched propose + timeout, and always undoable — spec 07 · ADR-0020/0022.)

- `owner-supervised-live-runs.md` — the live-run checklist. **Note:** its concrete steps predate the
  Tauri/Rust replatform and must be rebuilt for the Tauri stack + extended with the calm W3 cert-lab
  matrix (the file carries a banner saying so). The live-run GATE itself stands; only the steps are stale.

Authoritative project state: `docs/STATE.md`; the ship tracker: `docs/ship-readiness.md`.
