# reviews/ — point-in-time panel + audit records (NOT living truth)

This folder holds **dated, immutable** review artifacts: expert-panel records, adversarial-audit
verdicts, and their raw capture. They are the forensic record of a review at a moment in time — they
are **not** the source of truth (the living truth is `docs/specs/` + `docs/decisions/`), and they are
not implementation plans (those live in `docs/plans/`).

- `YYYY-MM-DD-*-panel.md` — expert / design panel records with owner dispositions.
- `YYYY-MM-DD-*-audit*.md` — audit runs. A `*-audit-raw.md` is the UNVERIFIED raw capture; the
  matching consolidated `*-audit.md` / `*-audit-fix-run.md` is the reviewed ledger. Both are kept so
  the raw claims stay separable from the verified findings.
- `evidence/` — point-in-time evidence bundles referenced by a specific review.

Nothing here should be read as current truth; each file is anchored to its date.
