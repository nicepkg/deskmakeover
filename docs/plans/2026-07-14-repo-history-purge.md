# Plan — repo history purge (shrink the clone) · 2026-07-14

**Goal**: a fresh `git clone` of this repo transfers a small pack, not the 123 MB it grew to,
so maintainers are not deterred by a slow clone.

**Status**: Track 1 DONE (forward removal, commit `32951c5`). **Track 2 PENDING** — the git-history
rewrite that actually shrinks the clone has NOT run; it is gated (see Preconditions).

## Why

A fresh clone pulled the ~123 MB pack. Root cause (measured 2026-07-14): ~70 MB of stale
`docs/**/evidence` screenshots + ~23 MB of retired C# (`legacy/` + pre-Amendment-1 `src/DeskMakeover.*`
nested paths) still living in history. The Rust port has been the sole production engine since the
M6 flip, and no living spec/ADR depends on the evidence pixels, so both are dead weight in every clone.

## Track 1 — forward removal (DONE, commit `32951c5`)

Removed from the working tree + reconciled all referencing docs (grep-verified: zero dangling
`legacy/` folder refs in living docs; adjective "legacy" preserved). See ADR-0019 Amendment 2 and
the journal "Repo slim-down 2026-07-14" entry. This frees disk but does NOT shrink the clone — the
bytes are still in history up to `32951c5^`.

## Track 2 — history rewrite (PENDING — the actual clone shrink)

### Preconditions (ALL must hold before running)

1. **The neighbor icon session is DONE** and has committed + pushed everything. (At Track-1 time it
   had uncommitted work in `crates/dm-resident/` + `src-tauri/`.)
2. **Every worktree/clone is clean** (`git status` shows nothing uncommitted anywhere).
3. **Owner has explicitly OK'd the force-push.** History rewrite changes every commit SHA from the
   first C#/evidence commit onward → force-push → all clones must re-clone or `git reset --hard`.

### Purge paths (fed to `git filter-repo --invert-paths --paths-from-file`)

KEEP `docs/plans/evidence/2026-07-calm/` (active calm-module evidence). Remove:

```
legacy/
docs/plans/evidence/2026-07-icons-v2/
docs/plans/evidence/2026-07-parity/
docs/plans/evidence/2026-07-v3/
docs/plans/evidence/2026-07-settings-i18n-shapes-icon/
docs/plans/evidence/2026-07-tauri/
docs/reviews/evidence/
src/DeskMakeover.App/
src/DeskMakeover.Core/
src/DeskMakeover.IconRendering/
src/DeskMakeover.Operations/
src/DeskMakeover.Shell/
src/DeskMakeover.Web/
tests/DeskMakeover.App.Tests/
tests/DeskMakeover.Core.Tests/
tests/DeskMakeover.E2E/
tests/DeskMakeover.IconRendering.Tests/
tests/DeskMakeover.Operations.Tests/
tests/DeskMakeover.Shell.Tests/
```

### Procedure

```bash
cd <repo>
# 0. Safety net — full backup + tag the last full-C# commit BEFORE the rewrite.
git bundle create ../deskmakeover-pre-purge-$(git rev-parse --short HEAD).bundle --all
git tag -f last-dotnet 32951c5^          # final .NET state, retrievable from the bundle

# 1. Purge from ALL history (write the list above to purge-paths.txt first).
git filter-repo --invert-paths --paths-from-file purge-paths.txt --force

# 2. Reclaim.
git reflog expire --expire=now --all && git gc --prune=now --aggressive

# 3. Verify (expect pack ~35-40 MB, down from ~123 MB).
git count-objects -vH | grep -E 'size-pack|count' ; du -sh .git

# 4. filter-repo drops 'origin' by design. Re-add + force-push.
git remote add origin <URL>
git push --force --all origin
git push --force --tags origin           # includes last-dotnet

# 5. Every other clone: re-clone, or `git fetch && git reset --hard origin/main`
#    (local commits must be re-applied — parent SHAs changed).
```

### Expected result

Pack **~123 MB → ~35-40 MB** (evidence ~70 MB + C# ~23 MB reclaimed; fonts 17 MB + testdata/icons
30 MB + normal code churn remain). Fonts stay as-is — the owner decided NOT to subset them (they are
runtime assets and 17 MB is acceptable).

### Recoverability

The rewrite makes the removed bytes unretrievable from `git log`. The `last-dotnet` tag + the
pre-purge bundle hold the final C# state if a future parity re-check ever needs the original.
