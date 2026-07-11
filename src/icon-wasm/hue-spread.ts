// M6 single-truth cutover — the cross-icon hue-spread pre-pass, ported
// byte-for-byte from the frozen `icon-compositor/hue-spread.ts` so the store's
// seed-selection survives the pixel-oracle deletion. It runs on the MAIN thread
// to pick each icon's adjusted plate seed (handed to the Rust kernel as
// `RenderOpts.fieldSeed`); it is NOT part of the certified tile kernel. Colour
// math comes from the shared `color-math.ts` survivor — zero frozen imports.
//
// Cross-icon hue spread (ADR-0016 D1; designer acceptance item 3). A desktop
// piles same-hue apps (the blue pile: Outlook/Skype/Twitter/OneDrive…); their
// derived plates would be indistinguishable. This pass rotates COLLIDING plate
// hues apart — deterministically (sorted inputs, no randomness, id-cached by
// the caller) so the bake reproduces the preview pixel-for-pixel.
//
// Two hard rules:
//  - identical artwork (same artKey) keeps identical plates — three .docx
//    files SHOULD look the same; only DISTINCT apps sharing a hue spread;
//  - rotation is capped so a brand hue stays recognizably itself (a Twitter
//    plate may lean cyan or indigo, never green). Within an 8-deep blue pile
//    the cap wins over the ideal gap — richer chroma + artwork carry the rest.

import type { Rgba } from './color-math'
import { fromRgbInt, rotateSeedHue, toOkLab } from './color-math'
import { hexToInt } from './config-abi'

/** Target minimum hue gap between DISTINCT plates (radians ≈ 12°) — the
 *  designer asked 25-30° but the brand cap binds first on dense piles; 12°
 *  is what the ±18° windows can guarantee for a 4-deep pile. */
const MIN_GAP = (12 * Math.PI) / 180
/** Max rotation either way (≈ 18°) — brand hue stays itself. */
const ROTATION_CAP = (18 * Math.PI) / 180
/** Forward/backward relaxation rounds (converges fast at these scales). */
const RELAX_ROUNDS = 4

export interface SpreadEntry {
  id: string
  /** Artwork identity (source URL); identical art → identical plate. */
  artKey: string
  /** The icon's derived seed colour (hex), or null for the no-hue tail. */
  seed: string | null
}

const TAU = Math.PI * 2

function hueOf(seed: Rgba): number {
  const lab = toOkLab(seed.r, seed.g, seed.b)
  return Math.atan2(lab.B, lab.A)
}

function toHex(c: Rgba): string {
  const h = (v: number) => v.toString(16).padStart(2, '0')
  return `#${h(c.r)}${h(c.g)}${h(c.b)}`.toUpperCase()
}

/**
 * The spread result: id → adjusted seed hex. Ids with a null seed are absent
 * (the engine's own fallback handles them). Pure and deterministic.
 */
export function computeHueSpread(entries: SpreadEntry[]): Map<string, string> {
  // One representative per artKey (first by sorted id), members follow it.
  const byArt = new Map<string, { artKey: string; seed: Rgba; hue: number; ids: string[] }>()
  for (const e of [...entries].sort((a, b) => (a.id < b.id ? -1 : 1))) {
    if (!e.seed) continue
    const rep = byArt.get(e.artKey)
    if (rep) {
      rep.ids.push(e.id)
    } else {
      const seed = fromRgbInt(hexToInt(e.seed))
      byArt.set(e.artKey, { artKey: e.artKey, seed, hue: hueOf(seed), ids: [e.id] })
    }
  }

  const reps = [...byArt.values()].sort((a, b) => a.hue - b.hue || (a.artKey < b.artKey ? -1 : 1))
  const result = new Map<string, string>()
  if (reps.length === 0) return result

  // Global min-gap relaxation around the hue circle: push neighbours apart in
  // forward/backward passes, each rep confined to its ±cap window. Cross-
  // cluster collisions are handled naturally (a per-cluster symmetric spread
  // can shove a cluster's edge INTO the next hue — the bug this replaces).
  const pos = reps.map((r) => r.hue)
  const lo = reps.map((r) => r.hue - ROTATION_CAP)
  const hi = reps.map((r) => r.hue + ROTATION_CAP)
  const n = reps.length
  for (let round = 0; round < RELAX_ROUNDS && n > 1; round++) {
    for (let i = 1; i < n; i++) {
      if (pos[i] - pos[i - 1] < MIN_GAP) {
        pos[i] = Math.min(hi[i], pos[i - 1] + MIN_GAP)
      }
    }
    for (let i = n - 2; i >= 0; i--) {
      if (pos[i + 1] - pos[i] < MIN_GAP) {
        pos[i] = Math.max(lo[i], pos[i + 1] - MIN_GAP)
      }
    }
    // Wrap seam: the circle's last→first neighbour pair (n>1 — a PAIR
    // straddling ±π is exactly the case that needs it, codex #7).
    if (n > 1) {
      const seam = pos[0] + TAU - pos[n - 1]
      if (seam < MIN_GAP) {
        pos[0] = Math.min(hi[0], pos[0] + (MIN_GAP - seam) / 2)
        pos[n - 1] = Math.max(lo[n - 1], pos[n - 1] - (MIN_GAP - seam) / 2)
      }
    }
  }

  reps.forEach((rep, i) => {
    const offset = pos[i] - rep.hue
    const hex = Math.abs(offset) < 1e-9 ? toHex(rep.seed) : toHex(rotateSeedHue(rep.seed, offset))
    for (const id of rep.ids) result.set(id, hex)
  })
  return result
}
