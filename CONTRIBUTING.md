# Contributing to DeskMakeover

Thanks for your interest. This guide gets you from a fresh clone to a mergeable pull request.

## Ways to contribute

You don't need a Windows box for most of it:

- **React UI** — the whole web half runs in a browser against a mock backend (`bun run dev`), on any OS.
- **Rust core** — `dm-icon-core` / `dm-icon-codec` / `dm-operations` are portable and unit-tested; only `dm-windows` needs Windows.
- **Docs & localization** — the README pair (EN / zh-CN) and `docs/` always welcome fixes; edit both READMEs in the same PR so they stay in lockstep.
- **Windows compatibility testing** — real-machine reports (version, apply result, reboot persistence, restore result) are gold for a pre-1.0 desktop app.

Check the [good first issues](https://github.com/nicepkg/deskmakeover/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22), and for anything large, open an issue to discuss before writing a big PR.

## Getting set up

You need [**Bun**](https://bun.sh) ≥ 1.1 and the Rust toolchain pinned in
[`rust-toolchain.toml`](rust-toolchain.toml) (installed automatically by `rustup` — including
the `wasm32-unknown-unknown` target). **Bun is the only JS toolchain — never use `npm` or
`node`.**

```bash
bun install          # JS deps
bun run dev          # web UI against a mock backend — any OS, browser + hot reload
bun test             # web tests
bun run tauri:dev    # full desktop app (Windows) — compiles the Rust host, opens the window
```

Most of the UI can be built and tested in the browser + mock loop without a Windows box; native
host work needs Windows. The full runbook is [`docs/development.md`](docs/development.md).

## Before you open a PR

Run the same gates CI runs, and make sure they are green:

```bash
bun run lint                 # oxlint
bunx tsc -b                  # type-check
bun test                     # web tests
cargo test --workspace       # Rust tests (dm-windows is Windows-only)
bun run check:bindings       # the generated bridge is in sync
```

If you changed the bridge contract (`dm-contracts`), regenerate the bindings with
`bun run gen:bindings` and commit the result.

## House rules

These are enforced in review (and some in tests), so following them up front saves a round-trip:

- **Extreme DRY.** Three strikes and you extract. No copy-paste survival, no `utils.ts` dumping
  ground — related code lives together.
- **Files ≤ 500 lines.** Long files get split. Names are documentation: `kebab-case` filenames,
  self-explanatory directories and identifiers.
- **Warm coral `#FF6F5E` is the only UI accent.** Blue/violet are banned and grep/test-gated
  (`tests/banned-colors.test.ts`); the only exemptions are OS-authentic depictions and the
  celebration confetti.
- **No dashes in user-facing copy** — em/en dashes and hyphenated compounds read as AI text. This
  applies to UI strings, not code or docs.
- **WYSIWYG.** Preview pixels must equal applied pixels. Don't fork the render path.
- **Reversibility is sacred.** Anything that writes to the user's system snapshots first, previews
  before applying, and stays restorable. Privileged actions go through the whitelisted
  `dm-elevated` helper, never inline.
- **A bug fix ships a regression test** that reproduces the failure first (red), then passes
  (green). This is required even for CRUD-shaped changes.
- **Test-first for substantial logic** (core algorithms, security, public APIs). Scaffolding and
  throwaway spikes don't need it.

## Commits & pull requests

- Use [Conventional Commits](https://www.conventionalcommits.org/): `feat:`, `fix:`, `docs:`,
  `refactor:`, `test:`, `ci:`, `chore:` (with optional scopes like `feat(icons):`).
- Keep a PR focused on one concern. Describe **what** changed and **why**, and paste the output
  of the gates above as evidence.
- Link the issue it closes. If it's a bug fix, point at the regression test.

## Reporting bugs

Open a [bug report](https://github.com/nicepkg/deskmakeover/issues/new/choose). In-app, the
**Settings → 反馈问题** button copies a full diagnostic report to your clipboard and opens a
pre-filled issue — please paste that report in.

## Security

Do not open a public issue for a vulnerability. Follow [`SECURITY.md`](SECURITY.md).

By contributing, you agree that your contributions are licensed under the project's
[MIT License](LICENSE).
