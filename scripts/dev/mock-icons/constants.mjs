// Shared config + numeric clamps for the mock-icon generator (spec 06 §5).
// SEED is read once from argv here so every module sees the same value.

export const SIZE = 256
export const SEED = Number(process.argv[2]) || 0x9e3779b9
export const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v)
export const clampByte = (v) => (v < 0 ? 0 : v > 255 ? 255 : Math.round(v))
