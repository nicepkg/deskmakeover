import { afterAll, describe, expect, test } from 'bun:test'
import { verify } from '../scripts/capture-oracle'
import { setNativeArrowRaster } from '../src/icon-compositor/marks'

// verify() installs the real shortcut-arrow badge into the compositor's
// module-global; reset it so this file never poisons the fallback-arrow
// assertions in icon-compositor.test.ts regardless of run order.
afterAll(() => setNativeArrowRaster(null))

// CI smoke for the frozen-compositor parity oracle (ADR-0019 M0b). Re-renders a
// deterministic sample of the committed goldens (by sorted path hash) and
// deep-compares the sampled stage dumps + the whole-set hash, catching BOTH
// accidental compositor drift and a stale/edited golden. The full sweep
// (`bun scripts/capture-oracle.ts --verify`, ~1152 cells) is the manual /
// CI-nightly command documented in testdata/icons/README.md.

describe('oracle corpus', () => {
  test('verify --sample 12 matches the committed goldens', () => {
    expect(verify(12)).toBe(0)
  })
})
