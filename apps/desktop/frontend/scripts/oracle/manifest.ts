// Manifest + JSON helpers for the oracle corpus. Golden files carry NO
// timestamps (determinism); the machine-comparison surface is the per-cell
// rasterHash (sha256 of decoded RGBA — platform-independent) plus deep-equal
// stage JSON. Byte-comparison in --verify is on DECODED pixels, not PNG
// container bytes (zlib output can vary by build; pixels never do) — the PNG is
// the storage format, the pixels are the contract.

import { createHash } from 'node:crypto'
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname } from 'node:path'

export const CORPUS_SCHEMA_VERSION = 1

/** Recursively key-sorted JSON with a trailing newline — stable across runs. */
export function canonicalJson(value: unknown): string {
  return JSON.stringify(sortKeys(value), null, 2) + '\n'
}

function sortKeys(v: unknown): unknown {
  if (Array.isArray(v)) return v.map(sortKeys)
  if (v && typeof v === 'object') {
    const out: Record<string, unknown> = {}
    for (const k of Object.keys(v as Record<string, unknown>).sort()) out[k] = sortKeys((v as Record<string, unknown>)[k])
    return out
  }
  return v
}

export function sha256Hex(bytes: Uint8Array | string): string {
  return createHash('sha256').update(bytes).digest('hex')
}

/** Whole-set hash: sha256 over `id:sha` lines in id order — pins the exact
 *  source population the desktop-global tiers (A/C) depend on. */
export function setHash(entries: { id: string; sha256: string }[]): string {
  const lines = [...entries].sort((a, b) => (a.id < b.id ? -1 : 1)).map((e) => `${e.id}:${e.sha256}`).join('\n')
  return sha256Hex(lines)
}

export function writeFile(path: string, data: Uint8Array | string): void {
  mkdirSync(dirname(path), { recursive: true })
  writeFileSync(path, data)
}

export function writeJson(path: string, value: unknown): void {
  writeFile(path, canonicalJson(value))
}

export function readJson<T>(path: string): T {
  return JSON.parse(readFileSync(path, 'utf8')) as T
}

export function readBytes(path: string): Uint8Array {
  return new Uint8Array(readFileSync(path))
}

/** Deterministic sample: sort cells by sha256(stableKey) and take the first n.
 *  Content-independent (keyed on the golden path), so the sample never shifts
 *  when pixels change — the CI smoke always probes the same cells. */
export function deterministicSample<T>(items: T[], keyOf: (t: T) => string, n: number): T[] {
  if (n >= items.length) return items
  return [...items].sort((a, b) => (sha256Hex(keyOf(a)) < sha256Hex(keyOf(b)) ? -1 : 1)).slice(0, n)
}

/** First differing path between two JSON-able values, or null if deep-equal. */
export function jsonDiff(a: unknown, b: unknown, path = ''): string | null {
  if (a === b) return null
  if (typeof a !== typeof b || a === null || b === null) return `${path || '<root>'}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`
  if (Array.isArray(a) || Array.isArray(b)) {
    if (!Array.isArray(a) || !Array.isArray(b)) return `${path}: array/non-array mismatch`
    if (a.length !== b.length) return `${path}: length ${a.length} != ${b.length}`
    for (let i = 0; i < a.length; i++) {
      const d = jsonDiff(a[i], b[i], `${path}[${i}]`)
      if (d) return d
    }
    return null
  }
  if (typeof a === 'object') {
    const ka = Object.keys(a as object).sort()
    const kb = Object.keys(b as object).sort()
    if (ka.join(',') !== kb.join(',')) return `${path}: keys {${ka}} != {${kb}}`
    for (const k of ka) {
      const d = jsonDiff((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k], path ? `${path}.${k}` : k)
      if (d) return d
    }
    return null
  }
  return `${path || '<root>'}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`
}

/** Byte-equality of two pixel buffers, with the first differing index. */
export function pixelsEqual(a: Uint8Array | Uint8ClampedArray, b: Uint8Array | Uint8ClampedArray): number {
  if (a.length !== b.length) return -2
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return i
  return -1
}
