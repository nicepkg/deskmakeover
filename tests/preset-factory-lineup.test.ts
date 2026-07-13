import { describe, expect, test } from 'bun:test'
import { BASE_CONFIGS, DEFAULT_PRESET_ID, PRESET_TYPE_OVERRIDES } from '../src/lib/icons-assemble'

// Locks the Preset Collection v2 factory-lineup owner decisions
// (docs/STATE.md §Open owner decisions, resolved 2026-07-13): the seven-preset lineup with the
// spectrum default, the shortcut-mark None decree, and the retirement of the #65470D dark-brown
// folder board. These were "owed regression guards" — closing them here rather than as prose.

const RETIRED_FOLDER_BOARD = '#65470D'
const EXPECTED_PRESETS = ['ascast', 'glass', 'ink', 'pebble', 'spectrum', 'stationery', 'white']

describe('preset collection v2 — factory lineup guards', () => {
  test('ships exactly the seven curated presets with the spectrum default', () => {
    expect(Object.keys(BASE_CONFIGS).sort()).toEqual([...EXPECTED_PRESETS].sort())
    expect(DEFAULT_PRESET_ID).toBe('spectrum')
    expect(BASE_CONFIGS[DEFAULT_PRESET_ID]).toBeDefined()
  })

  test('no preset carries a shortcut mark (owner decree: presets ship None)', () => {
    // Owner decree 2026-07-07 + the durable rule "presets never carry a shortcut mark" win over
    // the panel's badge-ON suggestion (STATE §Open owner decisions). Every base config leaves the
    // shortcut shape off; resolveTypeConfig cannot re-add it (shortcutShape is not a patch key).
    for (const [id, cfg] of Object.entries(BASE_CONFIGS)) {
      expect(cfg.shortcutShape, `preset ${id} must ship shortcutShape=null`).toBeNull()
    }
  })

  test('no preset or type override reintroduces the retired #65470D folder board', () => {
    // #65470D 深金板 全线退役 (docs/product/preset-collection-v2.md). It survives ONLY as a
    // user-selectable swatch (icon-axis-options.ts TYPE_PLATE_SWATCHES) — never a factory default.
    const plateColors: (string | null | undefined)[] = []
    for (const cfg of Object.values(BASE_CONFIGS)) plateColors.push(cfg.plateColor)
    for (const overrides of Object.values(PRESET_TYPE_OVERRIDES)) {
      for (const ov of Object.values(overrides)) plateColors.push(ov.patch.plateColor)
    }
    const offenders = plateColors.filter(
      (c) => typeof c === 'string' && c.toUpperCase() === RETIRED_FOLDER_BOARD,
    )
    expect(offenders).toEqual([])
  })
})
