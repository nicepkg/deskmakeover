import { describe, expect, test } from 'bun:test'
import { BASE_CONFIGS, DEFAULT_PRESET_ID, PRESET_TYPE_OVERRIDES } from '../src/lib/icons-assemble'
import { parseIconLookPayload } from '../src/lib/icon-look'

// Locks the Preset Collection v3 factory lineup (owner-curated hand-tuned
// exports, 2026-07-16): the nine-preset lineup with the 方圆/squircle default.
// The v2 chief-designer lineup (spectrum/stationery/glass/pebble/ink/white/
// ascast) retired wholesale — git remembers. The retired #65470D dark-brown
// folder-board guard carries over (it survives only as a user swatch).

const RETIRED_FOLDER_BOARD = '#65470D'
const EXPECTED_PRESETS = [
  'squircle', 'porthole', 'pixel', 'creek', 'scrapbook', 'gleam', 'diecut', 'blueprint', 'glaze',
]

describe('preset collection v3 — factory lineup guards', () => {
  test('ships exactly the nine owner-curated presets, in card order, squircle default', () => {
    expect(Object.keys(BASE_CONFIGS)).toEqual(EXPECTED_PRESETS) // key order = card order
    expect(DEFAULT_PRESET_ID).toBe('squircle')
    expect(BASE_CONFIGS[DEFAULT_PRESET_ID]).toBeDefined()
  })

  test('every recipe passes the ONE validator (the same gate imports run through)', () => {
    for (const [id, config] of Object.entries(BASE_CONFIGS)) {
      const parsed = parseIconLookPayload(
        JSON.stringify({ config, typeOverrides: PRESET_TYPE_OVERRIDES[id] ?? {} }),
      )
      expect(parsed, `preset ${id} must validate`).not.toBeNull()
    }
  })

  test('no preset ships a uniform shortcut shape (the toggle stays a user opt-in)', () => {
    for (const [id, cfg] of Object.entries(BASE_CONFIGS)) {
      expect(cfg.shortcutShape, `preset ${id} must ship shortcutShape=null`).toBeNull()
    }
  })

  test('diecut and blueprint deliberately ship no type overrides', () => {
    expect(PRESET_TYPE_OVERRIDES.diecut).toBeUndefined()
    expect(PRESET_TYPE_OVERRIDES.blueprint).toBeUndefined()
  })

  test('no preset or type override reintroduces the retired #65470D folder board', () => {
    const plateColors: (string | null | undefined)[] = []
    for (const cfg of Object.values(BASE_CONFIGS)) plateColors.push(cfg.plateColor)
    for (const overrides of Object.values(PRESET_TYPE_OVERRIDES)) {
      for (const ov of Object.values(overrides)) plateColors.push(ov.patch?.plateColor)
    }
    const offenders = plateColors.filter(
      (c) => typeof c === 'string' && c.toUpperCase() === RETIRED_FOLDER_BOARD,
    )
    expect(offenders).toEqual([])
  })
})
