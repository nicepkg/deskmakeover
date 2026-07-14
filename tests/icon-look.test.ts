import { describe, expect, test } from 'bun:test'
import type { ConfigDto } from '../src/bridge/types'
import { normalizeIconLook, parseIconLook, serializeIconLook } from '../src/lib/icon-look'
import { ICON_LOOK_VERSION, migrateIconLook, migrateWallpaperZoneEnums } from '../src/lib/preset-migrations'
import { parseRecipe } from '../src/lib/icons-assemble'
import { DEFAULT_KIND_POLICY } from '../src/lib/kind-policy'

// The ONE serializer/parser/validator (spec 09 §1) + the migration chain
// (spec 09 §3). parseRecipe (host styleJson) and package import share these.

const config: ConfigDto = {
  shape: 'Apple',
  subject: 'Original',
  monoStyle: 'Tonal',
  plateBand: 'Vivid',
  plateFallback: 'derived',
  shortcutShape: null,
  tint: '#FF6F5E',
  distinction: 'Mark',
  markStyle: 'Halo',
  markColor: null,
  plateColor: null,
  size: 'Mid',
  filter: 'None',
}

const recipe = {
  config,
  kindPolicy: { ...DEFAULT_KIND_POLICY },
  typeOverrides: { Folder: { source: 'custom' as const, patch: { shape: 'Folder' as const, plateColor: '#EAD6A8' } } },
}

describe('serialize/parse round-trip', () => {
  test('a serialized recipe carries the current version and round-trips losslessly', () => {
    const json = serializeIconLook(recipe)
    expect(JSON.parse(json).v).toBe(ICON_LOOK_VERSION)
    const back = parseIconLook(json)
    expect(back).not.toBeNull()
    expect(back!.config).toEqual(config)
    expect(back!.kindPolicy).toEqual(recipe.kindPolicy)
    expect(back!.typeOverrides).toEqual(recipe.typeOverrides)
  })

  test('parseRecipe (host styleJson) delegates to the same parser', () => {
    expect(parseRecipe(serializeIconLook(recipe))).toEqual(parseIconLook(serializeIconLook(recipe)))
  })

  test('a legacy version-less styleJson (v0) migrates forward', () => {
    const legacy = JSON.stringify({ config, kindPolicy: recipe.kindPolicy, typeOverrides: {} })
    const back = parseIconLook(legacy)
    expect(back).not.toBeNull()
    expect(back!.config).toEqual(config)
  })

  test('a pre-ADR-0018 legacy config (missing two-axis fields) is backfilled, not dropped', () => {
    const old = {
      shape: 'Circle', subject: 'Mono', tint: '#3FB6A8', distinction: 'None',
      markStyle: 'Arc', size: 'Mid', filter: 'None',
    }
    const back = parseIconLook(JSON.stringify({ config: old }))
    expect(back).not.toBeNull()
    expect(back!.config.monoStyle).toBe('Tonal')
    expect(back!.config.plateBand).toBe('Vivid')
    expect(back!.config.plateFallback).toBe('derived')
    expect(back!.config.plateColor).toBeNull()
    expect(back!.config.shortcutShape).toBeNull()
  })
})

describe('fail-closed behavior (spec 09 §3)', () => {
  test('a NEWER payload version parses null (never guess at unknown fields)', () => {
    const json = JSON.stringify({ v: ICON_LOOK_VERSION + 1, config })
    expect(parseIconLook(json)).toBeNull()
  })

  test('an unknown enum value in a KNOWN field rejects the payload', () => {
    const bad = { ...config, shape: 'Hexagon' }
    expect(parseIconLook(JSON.stringify({ v: 1, config: bad }))).toBeNull()
  })

  test('an invalid hex rejects; junk JSON rejects; junk shapes reject', () => {
    expect(parseIconLook(JSON.stringify({ v: 1, config: { ...config, tint: 'red' } }))).toBeNull()
    expect(parseIconLook('not json {{')).toBeNull()
    expect(parseIconLook(JSON.stringify({ v: 1, config: 42 }))).toBeNull()
    expect(parseIconLook(null)).toBeNull()
  })

  test('a known patch key with an invalid value rejects; unknown patch keys drop', () => {
    const withBadPatch = {
      v: 1,
      config,
      typeOverrides: { File: { source: 'custom', patch: { shape: 'NotAShape' } } },
    }
    expect(parseIconLook(JSON.stringify(withBadPatch))).toBeNull()
    const withForeignKey = {
      v: 1,
      config,
      typeOverrides: { File: { source: 'custom', patch: { shape: 'Tile', futureKnob: 9 } } },
    }
    const back = parseIconLook(JSON.stringify(withForeignKey))
    expect(back!.typeOverrides.File!.patch).toEqual({ shape: 'Tile' })
  })

  test('unknown buckets drop (additive tolerance); junk kindPolicy rejects', () => {
    const foreignBucket = { v: 1, config, typeOverrides: { Widget: { source: 'custom', patch: { shape: 'Tile' } } } }
    expect(parseIconLook(JSON.stringify(foreignBucket))!.typeOverrides).toEqual({})
    expect(parseIconLook(JSON.stringify({ v: 1, config, kindPolicy: { App: 'yes' } }))).toBeNull()
  })
})

describe('normalizeIconLook (the import validator)', () => {
  test('kindPolicy is optional (a preset payload) and preserved when present', () => {
    expect(normalizeIconLook({ config })!.kindPolicy).toBeUndefined()
    expect(normalizeIconLook({ config, kindPolicy: { App: false } })!.kindPolicy).toEqual({ App: false } as never)
  })
})

describe('migration chain mechanics', () => {
  test('idempotent: migrating an already-current payload is the identity', () => {
    const raw = { v: ICON_LOOK_VERSION, config }
    expect(migrateIconLook(raw, ICON_LOOK_VERSION)).toEqual(raw)
  })

  test('unknown/negative versions return null', () => {
    expect(migrateIconLook({}, -1)).toBeNull()
    expect(migrateIconLook({}, ICON_LOOK_VERSION + 5)).toBeNull()
  })
})

describe('wallpaper enum migrations (graduated MATERIAL_MIGRATION)', () => {
  test('retired finishes and Tab map to their heirs', () => {
    const zone = { material: 'Glaze', titleStyle: 'Tab' }
    expect(migrateWallpaperZoneEnums(zone)).toBe(true)
    expect(zone).toEqual({ material: 'Fluted', titleStyle: 'Chip' })
  })
  test('current values pass untouched', () => {
    const zone = { material: 'Frost', titleStyle: 'Etched' }
    expect(migrateWallpaperZoneEnums(zone)).toBe(false)
    expect(zone).toEqual({ material: 'Frost', titleStyle: 'Etched' })
  })
})
