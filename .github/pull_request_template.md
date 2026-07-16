<!-- Thanks for contributing! Keep the PR focused on one concern. -->

## What & why

<!-- What does this change, and why? Link the issue it closes (e.g. Closes #123). -->

## Evidence

<!-- Paste the output of the gates you ran. A bug fix must include a regression test. -->

```
bun run lint
bunx tsc -b
bun test
cargo test --workspace   # (Windows)
bun run check:bindings
```

## Checklist

- [ ] Follows the house rules in [CONTRIBUTING.md](../CONTRIBUTING.md) (DRY, files ≤ 500 lines,
      coral-only accent, no dashes in user-facing copy)
- [ ] Tests pass locally; a bug fix ships a regression test
- [ ] If the bridge contract changed, `bun run gen:bindings` was run and committed
- [ ] User-facing changes are noted in `CHANGELOG.md` (Unreleased)
