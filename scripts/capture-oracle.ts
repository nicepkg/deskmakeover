#!/usr/bin/env bun
// M0b oracle-corpus harness (ADR-0019 §Parity). Captures the FROZEN TypeScript
// icon compositor's output as the permanent parity oracle, and re-verifies a
// re-render against the committed goldens. This exact harness is the TS side of
// the M5 tri-target differential (TS vs Rust-WASM vs Rust-native), so the dump
// is machine-comparable (per-cell rasterHash + deep-equal stage JSON), not
// human-tuned. Two modes:
//   bun scripts/capture-oracle.ts --capture           write goldens
//   bun scripts/capture-oracle.ts --verify [--sample N]  re-render + compare
// See testdata/icons/README.md for the layout and the full/CI commands.

import { cpus, arch as osArch, platform as osPlatform, totalmem } from 'node:os'
import { join, resolve } from 'node:path'
import { renderTile } from '@/icon-compositor/compose'
import type { ComposeDiagnostics } from '@/icon-compositor/compose'
import { tileStyleKey } from '@/icon-compositor/icon-renderer'
import { setNativeArrowRaster } from '@/icon-compositor/marks'
import type { ConfigDto, IconKindBucket } from '@/bridge/types'
import { encodeRgbaPng, rasterHash } from './oracle/png-codec'
import { decodePng } from './oracle/png-codec'
import {
  decodeSource,
  loadSources,
  lookOf,
  optsOf,
  PRESET_IDS,
  readSourceMetas,
  resolveLook,
} from './oracle/desktop-session'
import type { OracleSource } from './oracle/desktop-session'
import { dumpSource } from './oracle/stage-dump'
import { selectTierBSources, styleCells } from './oracle/style-matrix'
import {
  CORPUS_SCHEMA_VERSION,
  deterministicSample,
  jsonDiff,
  pixelsEqual,
  readBytes,
  readJson,
  setHash,
  sha256Hex,
  writeFile,
  writeJson,
} from './oracle/manifest'

const REPO_ROOT = resolve(import.meta.dir, '..')
const TESTDATA = join(REPO_ROOT, 'testdata/icons')
const MASTER = 256
const ARROW_ASSET = join(REPO_ROOT, 'public/win-native-arrow.png')

/** Load the real Win11 shortcut-arrow badge exactly as the app's worker does at
 *  boot (native size, no resize), so shortcut/Keep goldens match the app bake
 *  instead of drawClassicArrow's vector fallback. Both modes MUST call this. */
function initNativeArrow(): Uint8Array {
  const bytes = readBytes(ARROW_ASSET)
  setNativeArrowRaster(decodePng(bytes))
  return bytes
}

interface RenderOptsRec {
  fieldSeed: string | null
  kindBucket: IconKindBucket | null
}

interface CellRecord {
  path: string
  sourceId: string
  styleKey: string
  config: ConfigDto
  isShortcut: boolean
  showOriginal: boolean
  opts: RenderOptsRec | null
  lane: string
  fieldLane: string | null
  passThrough: boolean
  rasterHash: string
}

/** Render one cell and return its raster + the executed lane diagnostics. */
function renderCell(
  source: OracleSource,
  config: ConfigDto,
  isShortcut: boolean,
  showOriginal: boolean,
  opts: RenderOptsRec | null,
) {
  const diag = {} as ComposeDiagnostics
  const raster = renderTile(source.raster, config, isShortcut, showOriginal, MASTER, opts ?? undefined, diag)
  return { raster, diag }
}

function cellRecord(path: string, source: OracleSource, config: ConfigDto, isShortcut: boolean, showOriginal: boolean, opts: RenderOptsRec | null): { rec: CellRecord; png: Uint8Array } {
  const { raster, diag } = renderCell(source, config, isShortcut, showOriginal, opts)
  return {
    rec: {
      path,
      sourceId: source.id,
      styleKey: tileStyleKey(config, isShortcut, showOriginal, MASTER),
      config,
      isShortcut,
      showOriginal,
      opts,
      lane: diag.lane,
      fieldLane: diag.fieldLane ?? null,
      passThrough: diag.passThrough ?? false,
      rasterHash: rasterHash(raster),
    },
    png: encodeRgbaPng(raster),
  }
}

// ---- capture ----------------------------------------------------------------

export function capture(): void {
  const arrowBytes = initNativeArrow()
  const sources = loadSources(REPO_ROOT)
  const sourceEntries = sources.map((s) => ({ id: s.id, sha256: sha256Hex(s.bytes) }))
  const set = setHash(sourceEntries)

  // Source-level stage dumps (config-independent, shared by every tier).
  let sourceMasks = 0
  const sourceManifest = sources.map((s) => {
    const { profile, maskPng } = dumpSource(s.raster, s.seed)
    writeJson(join(TESTDATA, `sources/profiles/${s.id}.json`), profile)
    writeFile(join(TESTDATA, `sources/masks/${s.id}.png`), maskPng)
    sourceMasks++
    return {
      id: s.id,
      file: s.file,
      sourceUrl: s.sourceUrl,
      kind: s.kind,
      bucket: s.bucket,
      isShortcut: s.isShortcut,
      label: s.label,
      sha256: sha256Hex(s.bytes),
      rasterHash: rasterHash(s.raster),
      seed: s.seed,
    }
  })

  // Tier A — full desktop, default look (spectrum) 256px masters.
  const tierA = resolveLook(sources, lookOf('spectrum'))
  const tierACells: CellRecord[] = []
  for (const cell of tierA.cells) {
    const path = `tier-a/masters/${cell.source.id}.png`
    const { rec, png } = cellRecord(path, cell.source, cell.config, cell.source.isShortcut, cell.showOriginal, optsOf(cell))
    writeFile(join(TESTDATA, path), png)
    tierACells.push(rec)
  }
  writeJson(join(TESTDATA, 'tier-a/cells.json'), { look: 'spectrum', set, cells: tierACells })

  // Tier B — style matrix on the representative subset.
  const profileOf = (s: OracleSource) => {
    const p = dumpSource(s.raster, s.seed).profile
    return { kind: p.kind, cornerSymmetric: p.cornerSymmetric, seed: p.seed, matchesCircle: p.matchesCircle, bucket: s.bucket }
  }
  const picks = selectTierBSources(sources, profileOf)
  const cells = styleCells()
  const tierBCells: CellRecord[] = []
  for (const { source } of picks) {
    for (const c of cells) {
      const path = `tier-b/${source.id}/${c.group}-${c.name}.png`
      const { rec, png } = cellRecord(path, source, c.config, c.isShortcut, c.showOriginal, null)
      writeFile(join(TESTDATA, path), png)
      tierBCells.push(rec)
    }
  }
  writeJson(join(TESTDATA, 'tier-b/cells.json'), {
    set,
    selection: picks.map((p) => p.pick),
    cellsPerSource: cells.length,
    cells: tierBCells,
  })

  // Tier C — one hue-spread session per look (decode seed + resolved fieldSeed).
  for (const id of PRESET_IDS) {
    const look = lookOf(id)
    const resolved = resolveLook(sources, look)
    writeJson(join(TESTDATA, `tier-c/${id}.json`), {
      look: id,
      config: look.config,
      typeOverrides: look.typeOverrides,
      set,
      items: resolved.cells.map((c) => ({
        id: c.source.id,
        bucket: c.source.bucket,
        seed: c.source.seed,
        fieldSeed: c.fieldSeed,
        poolMember: c.poolMember,
      })),
    })
  }

  // Perf baselines (informational, EXCLUDED from --verify): median of 5 warm
  // single-threaded full-set renders at 48px and 256px.
  writeJson(join(TESTDATA, 'perf-baseline.json'), perfBaseline(tierA))

  // Manifest + README.
  const totalPng = sourceMasks + tierACells.length + tierBCells.length
  writeJson(join(TESTDATA, 'manifest.json'), {
    schemaVersion: CORPUS_SCHEMA_VERSION,
    description: 'Parity oracle corpus for the frozen TS icon compositor (ADR-0019 M0b).',
    parity: {
      note: 'Byte-comparison in --verify is on DECODED RGBA pixels (platform-independent), not PNG container bytes. The per-cell rasterHash is sha256 of decoded RGBA and is the canonical parity anchor for the M5 tri-target differential.',
      decode: 'Goldens: pure node:zlib inflate + PNG un-filter (colortype 6 / 8-bit / no interlace); straight alpha, no colour management. Sources (real pack): oracle/source-decode handles PNG colortype 0/2/3/4/6 incl. 4-bit indexed + PLTE/tRNS (one .webp via macOS sips) and normalizes to 256² with a premultiplied bilinear resize (the app resizes every source to MASTER before renderTile).',
      encode: 'Deterministic: Paeth per-scanline filter + zlib deflate level 9. Proven by capture-twice byte-diff.',
      masterSize: MASTER,
      shortcutArrow: { asset: 'public/win-native-arrow.png', sha256: sha256Hex(arrowBytes), note: 'Loaded at native 100² and composited by drawClassicArrow exactly as the app worker does at boot; shortcut/Keep goldens carry the real badge, not the vector fallback. The Rust port must composite the same asset.' },
    },
    sourceDir: 'public/real-icons',
    setHash: set,
    sources: sourceManifest,
    tiers: {
      a: { dir: 'tier-a', look: 'spectrum', masters: tierACells.length, note: 'Full desktop under the factory-default look; pixels depend on the full source set (setHash pins it).' },
      b: { dir: 'tier-b', sources: picks.length, cellsPerSource: cells.length, cells: tierBCells.length },
      c: { dir: 'tier-c', sessions: PRESET_IDS.length, note: 'Cross-icon hue spread is desktop-global — one session JSON per look, listing every item decode seed + resolved fieldSeed.' },
    },
    counts: {
      sources: sources.length,
      sourceProfiles: sources.length,
      sourceMasks,
      tierAMasters: tierACells.length,
      tierBSources: picks.length,
      tierBCells: tierBCells.length,
      tierCSessions: PRESET_IDS.length,
      totalPng,
    },
    verify: {
      full: 'bun scripts/capture-oracle.ts --verify',
      ciSample: 'bun scripts/capture-oracle.ts --verify --sample 12',
    },
  })
  writeFile(join(TESTDATA, 'README.md'), readme())

  console.log(`captured: ${sources.length} sources, ${tierACells.length} tier-A masters, ${tierBCells.length} tier-B cells (${picks.length} sources × ${cells.length}), ${PRESET_IDS.length} tier-C sessions, ${sourceMasks} masks`)
  console.log(`total PNG: ${totalPng}  setHash: ${set.slice(0, 16)}`)
}

function median(xs: number[]): number {
  const s = [...xs].sort((a, b) => a - b)
  return s[Math.floor(s.length / 2)]
}

function perfBaseline(tierA: ReturnType<typeof resolveLook>): unknown {
  const timeFullSet = (size: number): number => {
    const t0 = performance.now()
    for (const c of tierA.cells) renderTile(c.source.raster, c.config, c.source.isShortcut, c.showOriginal, size, optsOf(c))
    return performance.now() - t0
  }
  const warm = (size: number) => {
    timeFullSet(size) // warm caches/JIT
    return median([0, 1, 2, 3, 4].map(() => timeFullSet(size)))
  }
  const cpu = cpus()[0]
  return {
    note: 'Informational only — EXCLUDED from --verify. Median of 5 warm single-threaded full-set renders.',
    machine: { platform: osPlatform(), arch: osArch(), cpu: cpu?.model ?? 'unknown', cores: cpus().length, memGB: Math.round(totalmem() / 2 ** 30), bun: Bun.version },
    fullSetItems: tierA.cells.length,
    medianMs: { '48px': Math.round(warm(48) * 100) / 100, '256px': Math.round(warm(MASTER) * 100) / 100 },
  }
}

// ---- verify -----------------------------------------------------------------

export function verify(sampleN: number | null): number {
  initNativeArrow()
  const errors: string[] = []

  // Source population must match (Tier A/C pixels depend on the whole set) —
  // hashed from bytes only, no decode. Full mode decodes everything; sample
  // mode decodes just the sources it touches, so the CI smoke stays snappy.
  const metas = readSourceMetas(REPO_ROOT)
  const currentSet = setHash(metas.map((m) => ({ id: m.id, sha256: sha256Hex(m.bytes) })))
  const manifest = readJson<{ setHash: string; counts: Record<string, number> }>(join(TESTDATA, 'manifest.json'))
  if (currentSet !== manifest.setHash) errors.push(`setHash drift: source pack changed (${currentSet.slice(0, 16)} != ${manifest.setHash.slice(0, 16)})`)

  const metaById = new Map(metas.map((m) => [m.id, m]))
  const decoded = new Map<string, OracleSource>()
  const srcOf = (id: string): OracleSource | null => {
    let s = decoded.get(id)
    if (!s) {
      const meta = metaById.get(id)
      if (!meta) return null
      s = decodeSource(meta)
      decoded.set(id, s)
    }
    return s
  }

  // Gather cell records from both tiers.
  const tierA = readJson<{ cells: CellRecord[] }>(join(TESTDATA, 'tier-a/cells.json'))
  const tierB = readJson<{ cells: CellRecord[] }>(join(TESTDATA, 'tier-b/cells.json'))
  let allCells = [...tierA.cells, ...tierB.cells]
  const fullCellCount = allCells.length
  if (sampleN !== null) allCells = deterministicSample(allCells, (c) => c.path, sampleN)

  // Re-render each cell: rasterHash must match the manifest, and the committed
  // golden PNG must decode to the identical pixels.
  for (const c of allCells) {
    const source = srcOf(c.sourceId)
    if (!source) {
      errors.push(`${c.path}: source ${c.sourceId} missing`)
      continue
    }
    const { raster, diag } = renderCell(source, c.config, c.isShortcut, c.showOriginal, c.opts)
    const h = rasterHash(raster)
    if (h !== c.rasterHash) errors.push(`${c.path}: rasterHash ${h.slice(0, 12)} != ${c.rasterHash.slice(0, 12)} (compositor or manifest drift)`)
    if ((diag.lane ?? null) !== c.lane || (diag.fieldLane ?? null) !== c.fieldLane) {
      errors.push(`${c.path}: lane ${diag.lane}/${diag.fieldLane} != ${c.lane}/${c.fieldLane}`)
    }
    try {
      const golden = decodePng(readBytes(join(TESTDATA, c.path)))
      const diffAt = pixelsEqual(golden.data, raster.data)
      if (diffAt !== -1) errors.push(`${c.path}: committed golden differs from re-render at byte ${diffAt} (stale/edited golden)`)
    } catch (e) {
      errors.push(`${c.path}: committed golden unreadable (${(e as Error).message})`)
    }
  }

  // Stage profiles + Tier C sessions: deep-equal (sampled by id in sample mode,
  // full otherwise). Full mode also validates the Tier C hue-spread sessions,
  // which need every source decoded.
  const profileMetas = sampleN === null ? metas : deterministicSample(metas, (m) => m.id, sampleN)
  for (const m of profileMetas) {
    const s = srcOf(m.id)!
    const { profile } = dumpSource(s.raster, s.seed)
    const committed = readJson<unknown>(join(TESTDATA, `sources/profiles/${s.id}.json`))
    const d = jsonDiff(profile, committed)
    if (d) errors.push(`sources/profiles/${s.id}.json: ${d}`)
  }
  if (sampleN === null) {
    const sources = metas.map((m) => srcOf(m.id)!)
    for (const id of PRESET_IDS) {
      const look = lookOf(id)
      const resolved = resolveLook(sources, look)
      const items = resolved.cells.map((c) => ({ id: c.source.id, bucket: c.source.bucket, seed: c.source.seed, fieldSeed: c.fieldSeed, poolMember: c.poolMember }))
      const committed = readJson<{ items: unknown }>(join(TESTDATA, `tier-c/${id}.json`))
      const d = jsonDiff(items, committed.items)
      if (d) errors.push(`tier-c/${id}.json: ${d}`)
    }
  }

  const scope = sampleN === null ? `full (${fullCellCount} cells)` : `sample ${sampleN}/${fullCellCount} cells`
  if (errors.length) {
    console.error(`oracle verify FAILED (${scope}): ${errors.length} diffs`)
    for (const e of errors.slice(0, 40)) console.error(`  ${e}`)
    if (errors.length > 40) console.error(`  … +${errors.length - 40} more`)
    return 1
  }
  console.log(`oracle verify OK (${scope}); setHash ${currentSet.slice(0, 16)}`)
  return 0
}

function readme(): string {
  return `# Icon parity oracle corpus (ADR-0019 M0b)

Golden renders + stage dumps from the **frozen** TypeScript icon compositor
(\`src/icon-compositor\`). This is the permanent parity
oracle for the Rust port and the **TS side of the M5 tri-target differential**
(TS vs Rust-WASM vs Rust-native). Regenerated only with a reviewed \`--bless\`
(re-run \`--capture\`); never hand-edit a golden.

## Layout
- \`manifest.json\` — source sha256 + whole-set hash, decode/encode spec, counts, parity note.
- \`sources/profiles/<id>.json\` — per-source stage profile (classification, own-background verdict + anchor rect, corner-symmetry, rim band + majority hex, dominant colour, foreground bbox, matchesShape/maxScaleInside). Config-independent.
- \`sources/masks/<id>.png\` — subject mask (8-bit grayscale, 0 bg / 255 subject).
- \`tier-a/masters/<id>.png\` + \`tier-a/cells.json\` — full desktop under the factory-default (spectrum) look; pixels depend on the whole source set.
- \`tier-b/<id>/<group>-<name>.png\` + \`tier-b/cells.json\` — style matrix on ~24 representative sources across every compositor axis (shapes, subjects, plate stops, marks, filters, shortcut badge, 7 presets).
- \`tier-c/<preset>.json\` — one cross-icon hue-spread session per look: every item's decode seed + resolved fieldSeed (what the Rust RenderSession must reproduce).
- \`perf-baseline.json\` — informational warm timings; **excluded from --verify**.

## Commands (run from the repo root)
- Capture / re-bless: \`bun scripts/capture-oracle.ts --capture\`
- Full verify (CI-nightly / manual): \`bun scripts/capture-oracle.ts --verify\`
- Fast CI smoke (also \`tests/oracle-corpus.test.ts\`): \`bun scripts/capture-oracle.ts --verify --sample 12\`

## Parity contract
Byte-comparison is on **decoded RGBA pixels** (platform-independent), not PNG
container bytes. Each cell's \`rasterHash\` (sha256 of decoded RGBA) is the
canonical anchor; the PNG is storage. \`--verify\` re-renders each cell, checks
the rasterHash + executed lane against the manifest, and decodes the committed
golden to confirm identical pixels.
`
}

// ---- main (only when run directly; importable for the CI test) --------------

if (import.meta.main) {
  const args = process.argv.slice(2)
  if (args.includes('--capture')) {
    capture()
  } else if (args.includes('--verify')) {
    const si = args.indexOf('--sample')
    const sampleN = si >= 0 ? Number(args[si + 1]) : null
    process.exit(verify(sampleN))
  } else {
    console.error('usage: capture-oracle.ts --capture | --verify [--sample N]')
    process.exit(2)
  }
}
