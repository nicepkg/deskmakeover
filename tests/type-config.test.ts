import { describe, expect, test } from 'bun:test'
import type { ConfigDto, TypeOverrides } from '../src/bridge/types'
import { resolveTypeConfig, typeAssertsShape, typeHasFixedPlate, typeIsCustom, typeOverridesEqual } from '../src/lib/type-config'
import { kindBucket } from '../src/lib/kind-policy'

// The per-type resolve chain (ADR-0017 D2): pure merge, sparse overrides,
// pool-exit predicate, preset-matching equality.

const BASE: ConfigDto = {
  shape: 'Apple', subject: 'Original', plateBand: 'Vivid', plateFallback: 'derived', shortcutShape: null, monoStyle: 'Tonal',
  tint: '#FF6F5E', distinction: 'None', markStyle: 'Shadow', markColor: null,
  plateColor: null, size: 'Mid', filter: 'None',
}

const LADDER: TypeOverrides = {
  Folder: { source: 'custom', patch: { shape: 'Bookmark' } },
  App: { source: 'custom', patch: { shape: 'Circle', subject: 'Mono' } },
  File: { source: 'global' },
}

describe('resolveTypeConfig', () => {
  test('followers and bucketless icons take the base untouched', () => {
    expect(resolveTypeConfig(BASE, { File: { source: 'global' } }, 'App')).toBe(BASE) // no entry → base identity
    expect(resolveTypeConfig(BASE, LADDER, 'File')).toBe(BASE) // explicit global → base
    expect(resolveTypeConfig(BASE, LADDER, null)).toBe(BASE)
    expect(resolveTypeConfig(BASE, undefined, 'Folder')).toBe(BASE)
  })

  test('custom patches merge sparsely over the base', () => {
    const app = resolveTypeConfig(BASE, LADDER, 'App')
    expect(app.shape).toBe('Circle')
    expect(app.subject).toBe('Mono')
    expect(app.tint).toBe(BASE.tint) // untouched keys inherit
    expect(app.filter).toBe(BASE.filter) // filter is not even patchable
    const folder = resolveTypeConfig(BASE, LADDER, 'Folder')
    expect(folder.shape).toBe('Bookmark')
    expect(folder.subject).toBe('Original')
  })

  test('plateColor patch pins the plate and exits the hue-spread pool', () => {
    const pinned: TypeOverrides = { File: { source: 'custom', patch: { plateColor: '#DDE6F2' } } }
    expect(resolveTypeConfig(BASE, pinned, 'File').plateColor).toBe('#DDE6F2')
    expect(typeHasFixedPlate(pinned, 'File')).toBe(true)
    expect(typeHasFixedPlate(pinned, 'App')).toBe(false)
    expect(typeHasFixedPlate(LADDER, 'App')).toBe(false) // demotion is not a pin
    expect(typeHasFixedPlate(pinned, null)).toBe(false)
  })

  test('typeIsCustom: only live non-empty patches count', () => {
    expect(typeIsCustom(LADDER, 'Folder')).toBe(true)
    expect(typeIsCustom(LADDER, 'File')).toBe(false) // source global
    expect(typeIsCustom({ File: { source: 'global' } }, 'App')).toBe(false) // absent
    expect(typeIsCustom({ App: { source: 'custom', patch: {} } }, 'App')).toBe(false) // empty patch
  })

  test('typeOverridesEqual: ladder equality for preset matching', () => {
    expect(typeOverridesEqual(LADDER, structuredClone(LADDER))).toBe(true)
    // global-source and absent entries are the same thing.
    expect(typeOverridesEqual({ File: { source: 'global' } }, {})).toBe(true)
    expect(typeOverridesEqual(undefined, {})).toBe(true)
    expect(typeOverridesEqual(LADDER, {})).toBe(false)
    const other = structuredClone(LADDER)
    other.App!.patch!.subject = 'BlackWhite'
    expect(typeOverridesEqual(LADDER, other)).toBe(false)
  })
})

describe('kindBucket (ADR-0017 taxonomy)', () => {
  test('bare executables are PROGRAMS, never documents', () => {
    expect(kindBucket('ExecutableFile')).toBe('App')
    expect(kindBucket('RegularFile')).toBe('File')
  })

  test('bare shortcut kinds (unresolved / web / appx targets) bucket to App', () => {
    // Kind carries TARGET semantics (owner 2026-07-16): a folder/file shortcut
    // arrives kind=Folder/RegularFile from the scan. These are the launcher and
    // fallback kinds — they stay App.
    expect(kindBucket('Shortcut')).toBe('App')
    expect(kindBucket('UrlShortcut')).toBe('App')
    expect(kindBucket('AppxShortcut')).toBe('App')
  })

  test('system virtual items merged into App (owner 2026-07-16)', () => {
    expect(kindBucket('RecycleBin')).toBe('App')
    expect(kindBucket('SystemIcon')).toBe('App')
  })
})

describe('typeAssertsShape', () => {
  test('true only for a custom entry whose patch asserts shape', () => {
    expect(typeAssertsShape(LADDER, 'Folder')).toBe(true) // custom + shape
    expect(typeAssertsShape(LADDER, 'App')).toBe(true)
    expect(typeAssertsShape(LADDER, 'File')).toBe(false) // global follower
    expect(typeAssertsShape({ File: { source: 'custom', patch: { tint: '#333333' } } }, 'File')).toBe(false) // custom, no shape
    expect(typeAssertsShape(undefined, 'Folder')).toBe(false)
    expect(typeAssertsShape(LADDER, null)).toBe(false) // bucketless
  })
})
