import { describe, expect, test } from 'bun:test'
import type { PresetReadEntryDto } from '../src/bridge/types'
import { toImportCandidate } from '../src/stores/preset-library'
import { serializeIconLook } from '../src/lib/icon-look'
import { BASE_CONFIGS } from '../src/lib/icons-assemble'

// Import candidates (spec 09 §5): package entries pass the ONE validator before
// the preview sheet; junk payloads become per-entry failures, never crashes.

const goodPayload = serializeIconLook({ config: BASE_CONFIGS.spectrum, typeOverrides: {} })

const readEntry = (over: Partial<PresetReadEntryDto['entry'] & object> = {}, thumb: string | null = null): PresetReadEntryDto => ({
  entry: {
    id: 'aaaaaaaa-1111',
    presetType: 'icon',
    schemaVersion: 1,
    meta: { name: '暖阳陶土', author: 'nicepkg', description: null, createdAt: null },
    payloadJson: goodPayload,
    hasThumb: thumb !== null,
    ...over,
  },
  thumbPngBase64: thumb,
  error: null,
})

describe('toImportCandidate', () => {
  test('a valid entry produces a recipe + a re-serialized validated payload', () => {
    const c = toImportCandidate(readEntry())
    expect(c.error).toBeNull()
    expect(c.recipe!.config.shape).toBe('Apple')
    expect(c.save!.meta.name).toBe('暖阳陶土')
    // The saved payload is OUR serialization of the validated recipe — junk
    // fields from the pack can never reach the library.
    expect(JSON.parse(c.save!.payloadJson).config.shape).toBe('Apple')
    expect(JSON.parse(c.save!.payloadJson).kindPolicy).toBeUndefined()
  })

  test('a structurally failed entry passes its reason through', () => {
    const c = toImportCandidate({ entry: null, thumbPngBase64: null, error: 'needs newer app' })
    expect(c.recipe).toBeNull()
    expect(c.save).toBeNull()
    expect(c.error).toBe('needs newer app')
  })

  test('an unknown enum in the payload fails validation per-entry', () => {
    const bad = JSON.stringify({ config: { ...BASE_CONFIGS.spectrum, shape: 'Hexagon' }, typeOverrides: {} })
    const c = toImportCandidate(readEntry({ payloadJson: bad }))
    expect(c.recipe).toBeNull()
    expect(c.save).toBeNull()
    expect(c.error).not.toBeNull()
    expect(c.name).toBe('暖阳陶土') // the preview still shows WHO failed
  })

  test('junk payload JSON fails per-entry, never throws', () => {
    const c = toImportCandidate(readEntry({ payloadJson: '{{{' }))
    expect(c.recipe).toBeNull()
    expect(c.error).not.toBeNull()
  })

  test('an opt-in-exported kindPolicy round-trips through import (owner #4, codex F1)', () => {
    const policy = { App: true, Folder: false, File: true }
    const withPolicy = serializeIconLook({ config: BASE_CONFIGS.ink, typeOverrides: {}, kindPolicy: policy })
    const c = toImportCandidate(readEntry({ payloadJson: withPolicy }))
    expect(c.recipe!.config.shape).toBe('Circle')
    // The opted-in participation survives into the recipe AND the saved payload
    // (a bundled backup restores participation; a style-only preset would omit it).
    expect(c.recipe!.kindPolicy).toEqual(policy)
    expect(JSON.parse(c.save!.payloadJson).kindPolicy).toEqual(policy)
  })

  test('a style-only payload carries NO kindPolicy (community preset stays orthogonal)', () => {
    const styleOnly = serializeIconLook({ config: BASE_CONFIGS.ink, typeOverrides: {} })
    const c = toImportCandidate(readEntry({ payloadJson: styleOnly }))
    expect(c.recipe!.kindPolicy).toBeUndefined()
    expect(JSON.parse(c.save!.payloadJson).kindPolicy).toBeUndefined()
  })
})
