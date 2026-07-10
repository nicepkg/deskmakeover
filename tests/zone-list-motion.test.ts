import { describe, expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

// Regression guard for the wallpaper zone list's ACTIVE WASH motion (owner ask
// 2026-07-10: "点击分区时 active 高亮的背景要丝滑上下移动").
//
// Root cause of the earlier break (frame-verified via CDP): the wash was a
// per-row `motion.div` with `layoutId="zoneListActiveWash"`. On selection change
// motion projected it from the target row UP toward the previous row, but the
// target row is `overflow-hidden`, so every frame above the row's top edge was
// CLIPPED — the highlight vanished from the old row and re-grew in place instead
// of gliding between rows. The fix hoists ONE wash into the non-clipping list
// container and slides it with `y = activeIndex * ROW_H` (same proven pattern as
// the segmented thumb, components/common/segmented.tsx).
//
// These assertions fail loudly if anyone reintroduces the clipped-projection.

const SRC = readFileSync(
  join(import.meta.dir, '..', 'src', 'components', 'panels', 'wallpaper-zone-list.tsx'),
  'utf8',
)

describe('zone list active wash', () => {
  test('does not use a layoutId projection (it gets clipped by the rows)', () => {
    // Match the JSX prop usage only — the ⛔ comment mentions the word on purpose.
    expect(SRC).not.toContain('layoutId=')
  })

  test('slides ONE container-level wash by translate (y = index * ROW_H)', () => {
    expect(SRC).toContain('activeIndex * ROW_H')
  })
})
