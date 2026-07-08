// The 8 icon-category builders + the randomized axes (spec 06 §5 distribution).
// Each builder returns { layers, opts } consumed by raster.renderIcon.

import { SIZE, SEED } from './constants.mjs'
import { pick, range } from './prng.mjs'
import { hsl, shade, palette } from './color.mjs'
import {
  layer,
  solid,
  linear,
  radial,
  pattern,
  grain,
  plateCover,
  coverRRect,
  coverRect,
  coverCircle,
  coverRing,
  coverEllipse,
  coverPoly,
  union,
} from './raster.mjs'
import { glyphCover, GLYPHS, letterCover, hanziCover } from './glyphs.mjs'

function bgField(rng, pal) {
  switch (pick(rng, ['solid', 'solid', 'linear', 'radial', 'noise', 'pattern'])) {
    case 'linear':
      return linear(shade(pal.plate, 1.12), shade(pal.plate, 0.78), rng() < 0.5 ? 1 : 0.4, 0.9)
    case 'radial':
      return radial(shade(pal.plate, 1.18), shade(pal.plate, 0.7), range(rng, 96, 160), range(rng, 96, 160), 200)
    case 'noise':
      return grain(solid(pal.plate), 0.12, (SEED ^ (rng() * 1e6)) | 0)
    case 'pattern':
      return pattern(pal.plate, shade(pal.plate, 0.86), 16 + Math.floor(rng() * 18))
    default:
      return solid(pal.plate)
  }
}
function baseAxes(rng, over1 = {}) {
  return {
    srcRes: rng() < 0.14 ? 32 : rng() < 0.12 ? 48 : SIZE,
    edge: pick(rng, ['aa', 'aa', 'aa', 'hard', 'glow', 'semi']),
    safe: range(rng, 0.6, 1.0),
    ss: 2,
    ...over1,
  }
}

function buildFlat(rng) {
  const pal = palette(rng)
  const shape = pick(rng, ['squircle', 'rrect', 'rrect', 'circle', 'superellipse', 'square'])
  const ax = baseAxes(rng)
  const plate = SIZE * range(rng, 0.82, 0.94)
  const layers = [layer(plateCover(shape, 128, 128, plate), bgField(rng, pal))]
  const g = pick(rng, GLYPHS)
  layers.push(layer(glyphCover(g, 128, 128, plate * 0.5 * ax.safe), solid(pal.ink)))
  return { layers, opts: { ...ax, glow: pal.plate } }
}

function buildSkeuo(rng) {
  const pal = palette(rng)
  const ax = baseAxes(rng, { edge: pick(rng, ['aa', 'aa', 'semi']) })
  // Pre-baked rounded corners on a transparent square canvas = the double-rounding trap.
  const plate = SIZE * range(rng, 0.86, 0.96)
  const r = plate * range(rng, 0.16, 0.26)
  const layers = [
    layer(coverRRect(128, 128, plate, plate, r), linear(shade(pal.plate, 1.22), shade(pal.plate, 0.72), 0.5, 0.9)),
    // glossy top sheen
    layer(coverEllipse(128, 128 - plate * 0.24, plate * 0.44, plate * 0.26), solid({ r: 255, g: 255, b: 255, a: 64 })),
    // bevel base
    layer(coverRRect(128, 128 + plate * 0.36, plate * 0.9, plate * 0.22, r * 0.6), solid({ r: 0, g: 0, b: 0, a: 40 })),
  ]
  layers.push(layer(glyphCover(pick(rng, GLYPHS), 128, 128, plate * 0.46 * ax.safe), solid(pal.ink)))
  if (rng() < 0.4) addBadge(layers, rng, pick(rng, ['tl', 'tr', 'bl', 'br']), pal)
  return { layers, opts: ax }
}

function buildPhoto(rng) {
  // Two distinct mid-light hues keep the photo-fill category visibly gradient-y; the
  // pure-black / pure-white degenerates arrive through the palette axis in the other
  // categories, so photos don't need to (and shouldn't) collapse to a flat black plate.
  const h1 = rng() * 360
  const c0 = hsl(h1, range(rng, 0.55, 0.92), range(rng, 0.4, 0.62))
  const c1 = hsl((h1 + range(rng, 40, 190)) % 360, range(rng, 0.55, 0.92), range(rng, 0.3, 0.55))
  const ax = baseAxes(rng, { ss: 2, edge: pick(rng, ['aa', 'hard']) })
  const pre = rng() < 0.5
  const plate = SIZE * (pre ? 0.94 : 1.0)
  const cover = pre ? coverRRect(128, 128, plate, plate, plate * 0.2) : coverRect(128, 128, SIZE, SIZE)
  const gradient = rng() < 0.5 ? linear(c0, c1, range(rng, 0.3, 1), range(rng, 0.3, 1)) : radial(c0, c1, range(rng, 70, 186), range(rng, 60, 160), range(rng, 180, 260))
  const layers = [layer(cover, grain(gradient, range(rng, 0.05, 0.14), (SEED + rng() * 1e6) | 0))]
  // soft vignette
  layers.push(layer(cover, radial({ r: 0, g: 0, b: 0, a: 0 }, { r: 0, g: 0, b: 0, a: 60 }, 128, 128, 210)))
  if (rng() < 0.5) layers.push(layer(glyphCover(pick(rng, GLYPHS), 128, 128, plate * 0.4), solid({ r: 255, g: 255, b: 255, a: 210 })))
  return { layers, opts: ax }
}

function buildBadged(rng, corner) {
  const base = buildFlat(rng)
  const pal = palette(rng)
  addBadge(base.layers, rng, corner, pal)
  return base
}
function addBadge(layers, rng, corner, pal) {
  const off = 128 * range(rng, 0.52, 0.62)
  const cx = corner.includes('l') ? 128 - off : 128 + off
  const cy = corner.includes('t') ? 128 - off : 128 + off
  const rad = SIZE * range(rng, 0.11, 0.15)
  layers.push(layer(coverCircle(cx, cy, rad + 3), solid({ r: 255, g: 255, b: 255, a: 235 })))
  layers.push(layer(coverCircle(cx, cy, rad), solid(pal.badge)))
  const mark = pick(rng, ['dot', 'plus', 'check'])
  const t = rad * 0.28
  if (mark === 'dot') layers.push(layer(coverCircle(cx, cy, rad * 0.4), solid({ r: 255, g: 255, b: 255 })))
  else if (mark === 'plus') layers.push(layer(union(coverRect(cx, cy, rad, t), coverRect(cx, cy, t, rad)), solid({ r: 255, g: 255, b: 255 })))
  else
    layers.push(
      layer(
        union(coverPoly([[cx - rad * 0.5, cy], [cx - rad * 0.15, cy + rad * 0.4], [cx - rad * 0.28, cy + rad * 0.5], [cx - rad * 0.5, cy + rad * 0.2]]), coverPoly([[cx - rad * 0.2, cy + rad * 0.4], [cx + rad * 0.5, cy - rad * 0.4], [cx + rad * 0.62, cy - rad * 0.24], [cx - rad * 0.12, cy + rad * 0.52]])),
        solid({ r: 255, g: 255, b: 255 }),
      ),
    )
}

function buildTransparent(rng) {
  const pal = palette(rng)
  const ax = baseAxes(rng, { edge: pick(rng, ['glow', 'glow', 'hard', 'semi', 'aa']), srcRes: rng() < 0.12 ? 32 : SIZE })
  const kind = pick(rng, ['hex', 'tri', 'diamond', 'ring', 'blob'])
  const s = SIZE * range(rng, 0.62, 0.84)
  const h = s / 2
  const fill = rng() < 0.5 ? solid(pal.plate) : linear(shade(pal.plate, 1.15), shade(pal.plate, 0.75), 0.6, 0.8)
  let cover
  if (kind === 'hex') {
    const pts = []
    for (let i = 0; i < 6; i++) pts.push([128 + h * Math.cos((i / 6) * 2 * Math.PI + 0.5), 128 + h * Math.sin((i / 6) * 2 * Math.PI + 0.5)])
    cover = coverPoly(pts)
  } else if (kind === 'tri') {
    cover = coverPoly([[128, 128 - h], [128 + h, 128 + h * 0.8], [128 - h, 128 + h * 0.8]])
  } else if (kind === 'diamond') {
    cover = coverPoly([[128, 128 - h], [128 + h, 128], [128, 128 + h], [128 - h, 128]])
  } else if (kind === 'ring') {
    cover = coverRing(128, 128, h * 0.7, s * 0.26)
  } else {
    const pts = []
    const lobes = 5 + Math.floor(rng() * 3)
    for (let i = 0; i < lobes * 2; i++) {
      const rr = i % 2 === 0 ? h : h * range(rng, 0.55, 0.8)
      pts.push([128 + rr * Math.cos((i / (lobes * 2)) * 2 * Math.PI), 128 + rr * Math.sin((i / (lobes * 2)) * 2 * Math.PI)])
    }
    cover = coverPoly(pts)
  }
  const layers = [layer(cover, fill)]
  if (kind !== 'ring' && rng() < 0.5) layers.push(layer(glyphCover(pick(rng, GLYPHS), 128, 128, s * 0.42), solid(pal.ink)))
  return { layers, opts: { ...ax, glow: pal.plate } }
}

function buildLetter(rng, latin) {
  const pal = palette(rng)
  const shape = pick(rng, ['squircle', 'rrect', 'circle'])
  const ax = baseAxes(rng, { ss: 3 })
  const plate = SIZE * range(rng, 0.84, 0.94)
  const layers = [layer(plateCover(shape, 128, 128, plate), bgField(rng, pal))]
  if (latin) layers.push(layer(letterCover(pick(rng, ['A', 'H', 'E', 'F', 'T', 'L', 'U', 'I', 'O']), 128, 128, plate * 0.5 * ax.safe), solid(pal.ink)))
  else layers.push(layer(hanziCover(rng, 128, 128, plate * 0.52 * ax.safe), solid(pal.ink)))
  return { layers, opts: ax }
}

function buildFolder(rng) {
  const family = pick(rng, [42, 45, 205, 150, 20, 0])
  const isGray = family === 0
  const body = isGray ? { r: 150, g: 156, b: 164 } : hsl(family, 0.62, 0.55)
  const front = isGray ? { r: 176, g: 182, b: 190 } : hsl(family, 0.66, 0.66)
  const ax = baseAxes(rng, { edge: pick(rng, ['aa', 'aa', 'semi']), srcRes: rng() < 0.12 ? 32 : SIZE })
  const w = SIZE * 0.82
  const layers = [
    // back panel + tab
    layer(coverRRect(128, 150, w, SIZE * 0.5, 14), solid(shade(body, 0.9))),
    layer(coverRRect(128 - w * 0.24, 96, w * 0.42, 26, 10), solid(shade(body, 0.9))),
    // front flap
    layer(coverRRect(128, 158, w, SIZE * 0.44, 16), solid(front)),
  ]
  if (rng() < 0.45) addBadge(layers, rng, 'br', palette(rng))
  return { layers, opts: ax }
}

function buildDocument(rng) {
  const pal = palette(rng)
  const ax = baseAxes(rng, { edge: 'aa', srcRes: SIZE })
  const w = SIZE * 0.62
  const hh = SIZE * 0.8
  const left = 128 - w / 2
  const paper = { r: 250, g: 250, b: 252 }
  const fold = SIZE * 0.16
  const layers = [
    layer(coverPoly([[left, 44], [left + w - fold, 44], [left + w, 44 + fold], [left + w, 44 + hh], [left, 44 + hh]]), solid(paper)),
    // dog-ear
    layer(coverPoly([[left + w - fold, 44], [left + w, 44 + fold], [left + w - fold, 44 + fold]]), solid(shade(paper, 0.82))),
    // header band (type colour)
    layer(coverRect(128, 44 + hh - 26, w, 34), solid(pal.badge)),
  ]
  // text lines
  for (let i = 0; i < 3; i++) layers.push(layer(coverRRect(128, 90 + i * 26, w * 0.7, 9, 4), solid({ r: 200, g: 202, b: 208 })))
  return { layers, opts: ax }
}

export function build(cat, rng, i) {
  if (cat === 'flat') return buildFlat(rng)
  if (cat === 'skeuo') return buildSkeuo(rng)
  if (cat === 'photo') return buildPhoto(rng)
  if (cat === 'badged') return buildBadged(rng, ['tl', 'tr', 'bl', 'br'][i % 4])
  if (cat === 'transparent') return buildTransparent(rng)
  if (cat === 'letter') return buildLetter(rng, i % 2 === 0)
  if (cat === 'folder') return buildFolder(rng)
  return buildDocument(rng)
}
// kind per category; the mock bridge (spec 06 §5) maps these onto item taxonomy.
export function kindFor(rng, cat) {
  if (cat === 'folder') return 'folder'
  if (cat === 'document') return 'file'
  if (cat === 'transparent') return pick(rng, ['url', 'lnk', 'lnk'])
  return pick(rng, ['lnk', 'lnk', 'lnk', 'exe', 'exe', 'url', 'uwp'])
}
