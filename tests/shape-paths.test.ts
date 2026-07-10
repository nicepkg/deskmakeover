import { describe, expect, test } from 'bun:test'
import type { IconShape } from '../src/bridge/types'
import { clipPathFor } from '../src/lib/shape-paths'

// Every IconShape must yield a usable, distinct chrome clip-path (spec 02 §Geometry).
const SHAPES: IconShape[] = [
  'Apple', 'Circle', 'Samsung', 'None', 'Bookmark',
  'Lemon', 'Tile', 'Teardrop', 'Diamond', 'Flower', 'Pebble',
]

describe('clipPathFor', () => {
  for (const shape of SHAPES) {
    test(`${shape} → a non-empty clip-path string`, () => {
      const value = clipPathFor(shape)
      expect(typeof value).toBe('string')
      expect(value.trim().length).toBeGreaterThan(0)
    })
  }

  test('distinct shapes do not all collapse to one string', () => {
    const values = SHAPES.map(clipPathFor)
    expect(new Set(values).size).toBeGreaterThan(1)
  })

  test('every shape is geometrically distinct', () => {
    const values = SHAPES.map(clipPathFor)
    expect(new Set(values).size).toBe(SHAPES.length)
  })
})
