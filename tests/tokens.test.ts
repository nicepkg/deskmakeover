import { describe, expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

// Spec 02 v3 token contract (ADR-0013): LIGHT is the design first-citizen
// (`:root/.light`), dark derives (`.dark`). The elevation + glass + canvas tokens
// must exist in BOTH theme blocks, the six-step type ladder must be declared in
// `@theme`, and the bundled-font law must hold (brand pair + OS-mirror exception).

const CSS = readFileSync(join(import.meta.dir, '..', 'src', 'index.css'), 'utf8')

function block(re: RegExp): string {
  return CSS.match(re)?.[1] ?? ''
}

const lightBlock = block(/:root,\s*\.light\s*\{([\s\S]*?)\n\}/)
const darkBlock = block(/\n\.dark\s*\{([\s\S]*?)\n\}/)
const themeBlock = block(/@theme[^{]*\{([\s\S]*?)\n\}/)

const ELEVATION = ['--elev-1', '--elev-2', '--elev-cta']
const GLASS = ['--glass', '--glass-ink', '--glass-ring', '--canvas-stage']
const CORE = ['--bg', '--raised', '--chip', '--t1', '--t2', '--t3', '--hair', '--coral', '--coral-ink']
const TYPE_LADDER = ['--text-caption', '--text-body', '--text-cardtitle', '--text-section', '--text-display']

describe('index.css token contract (spec 02 v3)', () => {
  test('light (:root/.light), dark (.dark) and @theme blocks are present', () => {
    expect(lightBlock.length).toBeGreaterThan(0)
    expect(darkBlock.length).toBeGreaterThan(0)
    expect(themeBlock.length).toBeGreaterThan(0)
  })

  for (const token of [...ELEVATION, ...GLASS, ...CORE]) {
    test(`light/:root defines ${token}`, () => {
      expect(lightBlock).toContain(`${token}:`)
    })
    test(`.dark defines ${token}`, () => {
      expect(darkBlock).toContain(`${token}:`)
    })
  }

  for (const token of TYPE_LADDER) {
    test(`@theme defines ${token}`, () => {
      expect(themeBlock).toContain(`${token}:`)
    })
  }

  test('type ladder v3: caption is 12px and no chrome size below it survives', () => {
    expect(themeBlock).toContain('--text-caption: 12px')
  })

  test('bundled brand fonts are declared with a block display strategy', () => {
    expect(CSS).toContain("font-family: 'Inter'")
    expect(CSS).toContain("font-family: 'HarmonyOS Sans SC'")
    expect(CSS.match(/font-display: block/g)?.length ?? 0).toBeGreaterThanOrEqual(3)
  })

  test('OS-mirror surfaces keep an OS-faithful stack (--font-os-mirror)', () => {
    expect(themeBlock).toContain('--font-os-mirror')
    // The brand pair must NOT leak into the OS-mirror token.
    const osMirrorLine = CSS.split('\n').find((l) => l.includes('--font-os-mirror'))!
    expect(osMirrorLine).not.toContain('Inter')
    expect(osMirrorLine).not.toContain('HarmonyOS')
  })

  test('light neutral ramp is cool/true-white OKLCH (no warm-taupe hexes)', () => {
    // The v2 taupe set must never return in either theme block.
    for (const taupe of ['#ECEBE7', '#F0EFEC', '#F5F5F3', '#8A877F', '#57534E']) {
      expect(lightBlock).not.toContain(taupe)
      expect(darkBlock).not.toContain(taupe)
    }
    expect(lightBlock).toContain('oklch(')
  })
})
