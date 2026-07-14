# Plan — repo history purge (shrink the clone) · 2026-07-14

**Goal**: a fresh `git clone` of this repo transfers a small pack, not the 123 MB it grew to,
so maintainers are not deterred by a slow clone.

**Status**: ✅ **DONE 2026-07-14.** Track 1 (forward removal) + Track 2 (history rewrite) both
executed. `git filter-repo` purged the 19 path prefixes below from all history; `main` was
force-pushed to origin. **Verified by a fresh GitHub clone: `.git` 146 MB → 54 MB** (pack
123 MB → 52 MB). A full pre-purge backup bundle was kept off-repo (see Recoverability).

## Why

A fresh clone pulled the ~123 MB pack. Root cause (measured 2026-07-14): ~70 MB of stale
`docs/**/evidence` screenshots + ~23 MB of retired C# (`legacy/` + pre-Amendment-1 `src/DeskMakeover.*`
nested paths) still living in history. The Rust port has been the sole production engine since the
M6 flip, and no living spec/ADR depends on the evidence pixels, so both are dead weight in every clone.

## Track 1 — forward removal (DONE)

Removed from the working tree + reconciled all referencing docs (grep-verified: zero dangling
`legacy/` folder refs in living docs; adjective "legacy" preserved). See ADR-0019 Amendment 2 and
the journal "Repo slim-down 2026-07-14" entry. (Original SHA `32951c5`, rewritten to `0126c20` by
Track 2.) Track 1 alone freed disk but did NOT shrink the clone; Track 2 (below) did.

## Track 2 — history rewrite (PENDING — the actual clone shrink)

### Preconditions (ALL held before running — verified 2026-07-14)

1. ✅ **The neighbor icon session was DONE** — its M7 work committed + pushed (commits survived the
   rewrite with new SHAs).
2. ✅ **Working tree clean**; the stale `.worktrees/review-*` review worktree was removed first
   (linked worktrees confuse filter-repo).
3. ✅ **Owner OK'd the force-push.** History rewrite changed every commit SHA from the first
   C#/evidence commit onward; any other clone must re-clone or `git fetch && git reset --hard origin/main`.

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

### Procedure (as executed 2026-07-14)

```bash
cd <repo>
# 0. Safety net — full backup of ALL pre-rewrite history (the sole archive; no remote tag, see Recoverability).
git bundle create ../deskmakeover-pre-purge-$(git rev-parse --short HEAD).bundle --all

# 0b. Remove the stale review worktree (linked worktrees confuse filter-repo).
git worktree remove .worktrees/review-* --force

# 1. Purge from ALL history (purge-paths.txt = the list above).
git filter-repo --invert-paths --paths-from-file purge-paths.txt --force

# 2. Reclaim.
git reflog expire --expire=now --all && git gc --prune=now --aggressive

# 3. Verify.
git count-objects -vH | grep -E 'size-pack|in-pack' ; du -sh .git

# 4. filter-repo drops 'origin' by design. Re-add + force-push main ONLY (no --all, no --tags —
#    the local backup branch stays local; a remote tag would drag the C# back into clones).
git remote add origin <URL>
git push --force origin main

# 5. Every other clone: re-clone, or `git fetch && git reset --hard origin/main`
#    (local commits must be re-applied — parent SHAs changed).
```

### Result (actual)

Pack **123 MB → 52 MB** (fresh-clone `.git` **146 MB → 54 MB**). Evidence PNGs (incompressible)
were the bulk of the reclaim; C# source compresses well, so it packed to only a few MB. The residual
~52 MB is dominated by **testdata/icons (~34 MB, the neighbor's parity corpus — untouched)** and
**fonts (~17 MB — owner kept, no subset)**. Those two are the current floor; nothing else is worth
purging without touching decisions the owner has made.

### Recoverability

The rewrite makes the removed bytes unretrievable from `git log`. The complete pre-rewrite history
(all refs, all C# + evidence) is preserved in a **backup bundle kept off-repo**:
`~/Documents/codes/deskmakeover-pre-purge-b632d4c.bundle` (~125 MB). Recover with
`git clone deskmakeover-pre-purge-b632d4c.bundle` if a future parity re-check ever needs the original.

No `last-dotnet` tag was pushed to origin **by design**: a tag on the remote pointing at the old C#
commits would keep those objects reachable, so every clone would pull them back — defeating the shrink.
The bundle is the archive; the owner can delete it once confident the C# is never needed again.
