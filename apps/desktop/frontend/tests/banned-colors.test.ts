import { describe, expect, test } from 'bun:test'
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { hexToHsv, normalizeHex } from '../src/lib/color'

// Owner rule (spec 02): warm coral is the only accent — blue/violet UI reads as
// AI slop and is permanently banned. This test walks every shipped source file
// and rejects blue/violet hexes and Tailwind blue-family utility classes.
//
// Exclusions: the 调色盘 hue spectrum (a full rainbow by definition) and the
// debug gallery's simulated wallpaper swatches (user content, not UI accent).

const ROOT = join(import.meta.dir, '..', 'src')
const EXCLUDED_PATHS = [join('components', 'debug')]
const SPECTRUM_HEXES = new Set(['#FF0000', '#FFFF00', '#00FF00', '#00FFFF', '#0000FF', '#FF00FF'])
// OS-authentic colours inside the desktop MIRROR (decorative taskbar replica) — these
// depict Windows itself (accent chips + the Start/logo tile), not our UI accent.
// Reviewed exception, keep the list tiny.
const OS_MIRROR_HEXES = new Set(['#4CC2FF', '#5B8DEF', '#0067C0'])
// The decorative taskbar strip renders the SIMULATED Windows desktop chrome (spec 02
// rule 5: app chrome and simulated OS are different layers) — its OS-neutral greys
// depict Windows itself, not our app chrome, so this ONE file is exempt from the
// cool-gray ban. The canvas app-chrome (mirror pills) now wears warm glass tokens and
// is fully enforced (finding 15 migration complete).
const COOL_GRAY_EXEMPT_PATHS = [
  join('components', 'canvas', 'taskbar-strip.tsx'),
  join('components', 'canvas', 'taskbar-icons.tsx'),
]
// The simulated taskbar's pinned-app art (own vector drawings of the OS layer —
// flag/folder/browser orb/store/music) NEEDS OS-authentic hues incl. blues.
// One file, reviewed with the owner's mockup 2026-07-09; app chrome stays banned.
const OS_MIRROR_HEX_EXEMPT_PATHS = [join('components', 'canvas', 'taskbar-icons.tsx')]
// The celebration confetti is multicolour BY DESIGN (owner decree 2026-07-10:
// App-Store-subscription-success 礼花) — a full festive palette, not a UI accent.
// One reviewed file; the coral-only rule still governs every real surface.
const CELEBRATION_HEX_EXEMPT_PATHS = [join('components', 'common', 'confetti.tsx')]

const BANNED_CLASS = /\b(?:bg|text|border|ring|from|via|to|stroke|fill|outline|decoration|shadow)-(?:blue|indigo|violet|purple|sky|cyan|fuchsia)-\d{2,3}\b/g
// Cool/neutral Tailwind grey families are banned in app chrome — the product palette
// is warm (t1/t2/t3 tokens). Use the tokens, never a stock grey scale.
const BANNED_GRAY_CLASS = /\b(?:bg|text|border|ring|from|via|to|stroke|fill|outline|decoration|divide|placeholder|shadow)-(?:slate|gray|zinc|neutral|stone|cool-?gray)-\d{2,3}\b/g
const HEX = /#[0-9a-fA-F]{3}\b|#[0-9a-fA-F]{6}\b/g

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const full = join(dir, name)
    if (statSync(full).isDirectory()) return walk(full)
    return /\.(tsx?|css)$/.test(name) ? [full] : []
  })
}

function isBannedHue(hex: string): boolean {
  const normalized = normalizeHex(hex)
  if (!normalized || SPECTRUM_HEXES.has(normalized) || OS_MIRROR_HEXES.has(normalized)) return false
  const { h, s, v } = hexToHsv(normalized)
  // Blue → violet band, saturated enough to read as an accent.
  return h >= 195 && h <= 290 && s > 0.28 && v > 0.25
}

describe('banned colours (owner rule: coral only, never blue/violet)', () => {
  const files = walk(ROOT).filter((f) => !EXCLUDED_PATHS.some((p) => f.includes(p)))

  test('scans a real file set', () => {
    expect(files.length).toBeGreaterThan(5)
  })

  for (const file of files) {
    test(file.slice(ROOT.length + 1), () => {
      const text = readFileSync(file, 'utf8')
      const classHits = text.match(BANNED_CLASS) ?? []
      expect(classHits).toEqual([])

      const graySafe = COOL_GRAY_EXEMPT_PATHS.some((p) => file.includes(p))
      const grayHits = graySafe ? [] : (text.match(BANNED_GRAY_CLASS) ?? [])
      expect(grayHits).toEqual([])

      const hexSafe =
        OS_MIRROR_HEX_EXEMPT_PATHS.some((p) => file.includes(p)) ||
        CELEBRATION_HEX_EXEMPT_PATHS.some((p) => file.includes(p))
      const hexHits = hexSafe ? [] : (text.match(HEX) ?? []).filter(isBannedHue)
      expect(hexHits).toEqual([])
    })
  }
})
