# Icon parity oracle corpus (ADR-0019 M0b)

Golden renders + stage dumps from the **frozen** TypeScript icon compositor
(`apps/desktop/frontend/src/icon-compositor`). This is the permanent parity
oracle for the Rust port and the **TS side of the M5 tri-target differential**
(TS vs Rust-WASM vs Rust-native). Regenerated only with a reviewed `--bless`
(re-run `--capture`); never hand-edit a golden.

## Layout
- `manifest.json` — source sha256 + whole-set hash, decode/encode spec, counts, parity note.
- `sources/profiles/<id>.json` — per-source stage profile (classification, own-background verdict + anchor rect, corner-symmetry, rim band + majority hex, dominant colour, foreground bbox, matchesShape/maxScaleInside). Config-independent.
- `sources/masks/<id>.png` — subject mask (8-bit grayscale, 0 bg / 255 subject).
- `tier-a/masters/<id>.png` + `tier-a/cells.json` — full desktop under the factory-default (spectrum) look; pixels depend on the whole source set.
- `tier-b/<id>/<group>-<name>.png` + `tier-b/cells.json` — style matrix on ~24 representative sources across every compositor axis (shapes, subjects, plate stops, marks, filters, shortcut badge, 7 presets).
- `tier-c/<preset>.json` — one cross-icon hue-spread session per look: every item's decode seed + resolved fieldSeed (what the Rust RenderSession must reproduce).
- `perf-baseline.json` — informational warm timings; **excluded from --verify**.

## Commands (run from `apps/desktop/frontend`)
- Capture / re-bless: `bun scripts/capture-oracle.ts --capture`
- Full verify (CI-nightly / manual): `bun scripts/capture-oracle.ts --verify`
- Fast CI smoke (also `tests/oracle-corpus.test.ts`): `bun scripts/capture-oracle.ts --verify --sample 12`

## Parity contract
Byte-comparison is on **decoded RGBA pixels** (platform-independent), not PNG
container bytes. Each cell's `rasterHash` (sha256 of decoded RGBA) is the
canonical anchor; the PNG is storage. `--verify` re-renders each cell, checks
the rasterHash + executed lane against the manifest, and decodes the committed
golden to confirm identical pixels.
