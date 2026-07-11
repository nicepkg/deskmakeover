#!/usr/bin/env bun
// Anchor assertions for the M6 byte-parity certificate — the safety net that
// makes the differential mean something.
//
// Before hardening (M6 kernel-speed plan Phase 0, Codex audit), the gate accepted
// ANY non-empty corpus and passed whenever the present cells all matched their
// goldens, without ever checking the setHash, the cell/source counts, the per-lane
// population, the file lengths, or the 389,808,128-byte compared total. A truncated,
// stale, or re-based corpus sailed through: a 1-cell corpus that matched read as a
// full green. This module pins every one of those to the certified anchor (M5.12
// all-real corpus, ADR-0019) and throws on any drift, so no fast-kernel work can
// land while silently weakening what "0/389,808,128" means.
//
// The chain of trust: setHash pins the source PNG pack (recomputed here exactly as
// scripts/capture-oracle.ts --verify does); the deterministic decode + frozen TS
// oracle turn that pack into the decoded sources and goldens, whose content digests
// are pinned too; the counts / lengths / histograms pin the shape. Any deliberate
// re-anchor updates the constants below in the same commit.

import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, resolve } from 'node:path'

import { setHash, sha256Hex } from '../../../scripts/oracle/manifest'

const REPO_ROOT = resolve(import.meta.dir, '../../..')

// ── The certified anchor ────────────────────────────────────────────────────
/** sha256 over `id:sha256(pngBytes)` lines in id order — pins the exact 124-source
 *  pack the desktop-global tiers depend on (scripts/oracle/manifest.ts setHash). */
export const SET_HASH = '8a6c19ee69235d95092cf6b593a89b6c334690d1a3b8c71baf7376bb65b773df'
/** sha256 of public/win-native-arrow.png — the badge the shortcut goldens carry. */
export const ARROW_PNG_SHA256 = '2432a8e026a6f409d94512f19bc116f4df49fa21fea1fc1c7a3248159eb2c4a0'

export const SOURCE_COUNT = 124
export const TIER_A = 124
export const TIER_B = 1363
export const CELL_COUNT = TIER_A + TIER_B // 1487
export const MASTER = 256
export const CELL_BYTES = MASTER * MASTER * 4 // 262,144
export const TOTAL_BYTES = CELL_COUNT * CELL_BYTES // 389,808,128
export const ARROW_W = 100
export const ARROW_H = 100
export const ARROW_BYTES = ARROW_W * ARROW_H * 4 // 40,000

// Content digests (setHash-style: sha256 over sorted `stem:sha256(bytes)` lines) —
// catch a stale-but-right-shape corpus that length/count checks alone would miss.
// Deterministic across platforms by the ADR-0019 doctrine; a drift here is either a
// deliberate re-anchor (update the constant) or a determinism regression (a real bug).
export const GOLDENS_DIGEST = 'f761909af6b8ae66dad749586d6c1445a74209312f820821bf2197288f9b9721'
export const SOURCES_DIGEST = '3721e7a9926f416ccf2492f52813e44e2790b0babdc76892095c88cf4df0077b'
export const ARROW_RGBA_SHA256 = 'f99be6764e19c9c975abefc55501602f2f92191687387c5b5195226140d59f1b'

// The FULL ordered render manifest — sha256 of cells.jsonl itself, i.e. the exact
// per-cell source->config->flags->lane->fieldLane list, in order. The lane/field
// histograms above pin only the MARGINALS: swap a board cell for a second alarm cell
// and every count/histogram/byte-total is unchanged, so a shape-specific coverage
// loss slips through. This pins the specs themselves, so any drift in what the corpus
// renders is red before a byte is diffed. (Codex Phase-0 audit #1.)
export const CELLS_MANIFEST_SHA256 = '2c9b7f129fb7a5421a98e39242bf9319bd3785cc43b63ec469833e337ada12b5'

// Per-lane cell population — a lane that gains/loses cells (or a renamed/added lane)
// flips this red even if every present cell still matches its golden. Sums to 1487.
export const LANE_HISTOGRAM: Readonly<Record<string, number>> = {
  'bare-white': 134,
  'derived-field': 1193,
  'inscribe-white': 1,
  'layered-mono': 29,
  original: 58,
  'passthrough-match': 4,
  'passthrough-none': 29,
  'plate-detect': 34,
  stretch: 5,
}
// Per-field-lane population (fieldLane, null → '(none)'). Sums to 1487.
export const FIELD_HISTOGRAM: Readonly<Record<string, number>> = {
  '(none)': 294,
  'derived-bare-shadow': 855,
  'full-square': 38,
  'own-board': 221,
  'user-plate-bare': 67,
  'user-plate-board': 12,
}

// ── assertions ──────────────────────────────────────────────────────────────

class AnchorError extends Error {}

function fail(msg: string): never {
  throw new AnchorError(`ANCHOR DRIFT — ${msg}`)
}

function eq(actual: unknown, expected: unknown, what: string): void {
  if (actual !== expected) fail(`${what}: got ${JSON.stringify(actual)}, expected ${JSON.stringify(expected)}`)
}

function assertHistogram(actual: Record<string, number>, expected: Readonly<Record<string, number>>, what: string): void {
  const aKeys = Object.keys(actual).sort().join(',')
  const eKeys = Object.keys(expected).sort().join(',')
  if (aKeys !== eKeys) fail(`${what} keys: {${aKeys}} != {${eKeys}}`)
  for (const k of Object.keys(expected)) eq(actual[k], expected[k], `${what}[${k}] count`)
}

/** setHash-style content digest over every `*.rgba` in a corpus subdir. */
export function digestPixelDir(pixelsDir: string, sub: string): string {
  const files = readdirSync(join(pixelsDir, sub)).filter((f) => f.endsWith('.rgba')).sort()
  const lines = files.map((f) => `${f.replace(/\.rgba$/, '')}:${sha256Hex(new Uint8Array(readFileSync(join(pixelsDir, sub, f))))}`)
  return sha256Hex(lines.join('\n'))
}

/** Recompute the setHash from the REAL source loader's pack and pin the source pack
 *  + arrow — a changed / added / removed / reordered source PNG or arrow asset flips
 *  this red. The recompute enumerates `public/real-icons/manifest.json` (what
 *  scripts/oracle/desktop-session.ts readSourceMetas actually loads to build the
 *  corpus), NOT testdata/icons/manifest.json — the two are different files and could
 *  drift apart, leaving a testdata-only anchor blind to a changed real pack. (Codex
 *  Phase-0 audit #2.) */
export function assertSourcePack(): void {
  // The pinned anchor value + tier counts live in the corpus manifest.
  const corpus = JSON.parse(readFileSync(join(REPO_ROOT, 'testdata/icons/manifest.json'), 'utf8')) as {
    setHash: string
    counts: Record<string, number>
    parity: { shortcutArrow: { sha256: string } }
  }
  eq(corpus.setHash, SET_HASH, 'testdata/icons/manifest.json setHash')
  eq(corpus.parity.shortcutArrow.sha256, ARROW_PNG_SHA256, 'manifest arrow sha256')
  eq(corpus.counts.tierAMasters, TIER_A, 'manifest counts.tierAMasters')
  eq(corpus.counts.tierBCells, TIER_B, 'manifest counts.tierBCells')

  // Recompute from the loader manifest — the pack the corpus is actually built from.
  const pack = JSON.parse(readFileSync(join(REPO_ROOT, 'public/real-icons/manifest.json'), 'utf8')) as { id: string; file: string }[]
  eq(pack.length, SOURCE_COUNT, 'public/real-icons/manifest.json entry count')
  const entries = pack.map((e) => ({
    id: e.id,
    sha256: sha256Hex(new Uint8Array(readFileSync(join(REPO_ROOT, 'public/real-icons', e.file)))),
  }))
  eq(setHash(entries), SET_HASH, 'recomputed setHash from the public/real-icons loader pack')

  const arrowSha = sha256Hex(new Uint8Array(readFileSync(join(REPO_ROOT, 'public/win-native-arrow.png'))))
  eq(arrowSha, ARROW_PNG_SHA256, 'public/win-native-arrow.png sha256')
}

/** Pin the dumped corpus shape + content: counts, file lengths, lane/field
 *  histograms, and the goldens / decoded-sources / arrow content digests. */
export function assertCorpusShape(pixelsDir: string): void {
  const arrowMeta = JSON.parse(readFileSync(join(pixelsDir, 'arrow.json'), 'utf8')) as { width: number; height: number }
  eq(arrowMeta.width, ARROW_W, 'arrow.json width')
  eq(arrowMeta.height, ARROW_H, 'arrow.json height')
  eq(statSync(join(pixelsDir, 'arrow.rgba')).size, ARROW_BYTES, 'arrow.rgba length')

  const srcFiles = readdirSync(join(pixelsDir, 'sources')).filter((f) => f.endsWith('.rgba'))
  eq(srcFiles.length, SOURCE_COUNT, 'sources/*.rgba count')
  for (const f of srcFiles) eq(statSync(join(pixelsDir, 'sources', f)).size, CELL_BYTES, `source ${f} length`)

  const goldFiles = readdirSync(join(pixelsDir, 'expected')).filter((f) => f.endsWith('.rgba'))
  eq(goldFiles.length, CELL_COUNT, 'expected/*.rgba count')
  for (const f of goldFiles) eq(statSync(join(pixelsDir, 'expected', f)).size, CELL_BYTES, `golden ${f} length`)

  const lines = readFileSync(join(pixelsDir, 'cells.jsonl'), 'utf8').split('\n').filter((l) => l.length > 0)
  eq(lines.length, CELL_COUNT, 'cells.jsonl record count')
  const lane: Record<string, number> = {}
  const field: Record<string, number> = {}
  for (const l of lines) {
    const r = JSON.parse(l) as { lane: string; fieldLane: string | null }
    lane[r.lane] = (lane[r.lane] ?? 0) + 1
    const fk = r.fieldLane ?? '(none)'
    field[fk] = (field[fk] ?? 0) + 1
  }
  assertHistogram(lane, LANE_HISTOGRAM, 'lane')
  assertHistogram(field, FIELD_HISTOGRAM, 'fieldLane')

  // Pin the full ordered render manifest, not just its marginals (audit #1).
  eq(sha256Hex(new Uint8Array(readFileSync(join(pixelsDir, 'cells.jsonl')))), CELLS_MANIFEST_SHA256, 'cells.jsonl render-manifest digest')

  eq(digestPixelDir(pixelsDir, 'expected'), GOLDENS_DIGEST, 'goldens content digest')
  eq(digestPixelDir(pixelsDir, 'sources'), SOURCES_DIGEST, 'decoded-sources content digest')
  eq(sha256Hex(new Uint8Array(readFileSync(join(pixelsDir, 'arrow.rgba')))), ARROW_RGBA_SHA256, 'arrow.rgba content digest')
}

/** Full anchor gate: source pack + dumped corpus. Throws AnchorError on drift. */
export function assertCorpusAnchor(pixelsDir: string): void {
  assertSourcePack()
  assertCorpusShape(pixelsDir)
}
