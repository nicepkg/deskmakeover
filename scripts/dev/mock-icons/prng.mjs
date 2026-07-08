// Seeded PRNG + pickers — deterministic pack generation.

export function mulberry32(seed) {
  let a = seed >>> 0
  return () => {
    a = (a + 0x6d2b79f5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}
export const pick = (rng, arr) => arr[Math.floor(rng() * arr.length)]
export const range = (rng, lo, hi) => lo + rng() * (hi - lo)
