// The before/after compare-sheet composer (icons.exportCompare) — the product's growth engine:
// a branded 1200×660 dark card whose shared screenshot is itself an advertisement. Ported from
// the frozen oracle `ComparisonImageExporter.cs`: same canvas size, same 4×2 grids, same warm
// coral accent (never blue/violet). Composition happens HERE in the webview (it owns the fonts,
// the CJK stack, and both image states — D1: Rust stays thin platform I/O and only saves bytes).

/** One tile on the sheet: the raw original source and the styled master this apply baked. */
export interface CompareTile {
  /** The original 256px source (a `dmicon://` URL the scan advertised). */
  originalUrl: string
  /** The styled master PNG, raw base64 (the same pixels the desktop received). */
  styledPng: string
}

/** The localized strings the card draws (resolved by the caller so this stays framework-free). */
export interface CompareText {
  productName: string
  tagline: string
  before: string
  after: string
  homepage: string
}

const WIDTH = 1200
const HEIGHT = 660
const COLUMNS = 4
const MAX_TILES = 8
const CELL = 108
const ICON = 84
const PANEL_TOP = 168
const PANEL_WIDTH = 480
const MARGIN = 60

// The oracle's exact palette: dark bed, warm neutrals, coral accent.
const BG = '#1a1a1c'
const PRIMARY = '#f4f4f2'
const SECONDARY = '#a8a7a1'
const ACCENT = '#ff6f5e'

/** The card's font stack — the platform UI face with a CJK fallback, matching the oracle's
 *  Segoe UI Variable + Microsoft YaHei pairing (the webview resolves whichever exists). */
const FACE =
  '-apple-system, "Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei UI", "PingFang SC", sans-serif'

/** Composes the sheet and returns it as raw base64 PNG (the bridge's payload shape). */
export async function composeCompareSheet(
  tiles: CompareTile[],
  text: CompareText,
): Promise<string> {
  const sample = tiles.slice(0, MAX_TILES)
  const [originals, styled] = await Promise.all([
    Promise.all(sample.map((t) => loadImage(t.originalUrl))),
    Promise.all(sample.map((t) => loadImage(`data:image/png;base64,${t.styledPng}`))),
  ])

  const canvas = document.createElement('canvas')
  canvas.width = WIDTH
  canvas.height = HEIGHT
  const ctx = canvas.getContext('2d')
  if (!ctx) throw new Error('compare sheet: no 2d context')

  ctx.fillStyle = BG
  ctx.fillRect(0, 0, WIDTH, HEIGHT)
  ctx.textBaseline = 'top'

  drawText(ctx, text.productName, 40, 600, PRIMARY, MARGIN, 44)
  drawText(ctx, text.tagline, 18, 400, SECONDARY, MARGIN + 2, 104)

  drawPanel(ctx, originals, text.before, MARGIN, SECONDARY)
  drawPanel(ctx, styled, text.after, WIDTH - MARGIN - PANEL_WIDTH, ACCENT)

  // Center arrow between the panels.
  drawText(ctx, '→', 44, 400, ACCENT, WIDTH / 2 - 16, PANEL_TOP + 150)

  drawText(ctx, text.homepage, 16, 400, SECONDARY, MARGIN, HEIGHT - 52)

  const url = canvas.toDataURL('image/png')
  return url.slice(url.indexOf(',') + 1)
}

function drawPanel(
  ctx: CanvasRenderingContext2D,
  images: (HTMLImageElement | null)[],
  label: string,
  x: number,
  labelColor: string,
): void {
  drawText(ctx, label, 20, 500, labelColor, x + 4, PANEL_TOP)
  const gridTop = PANEL_TOP + 44
  images.forEach((image, i) => {
    if (!image) return
    const col = i % COLUMNS
    const row = Math.floor(i / COLUMNS)
    const cx = x + col * CELL + (CELL - ICON) / 2
    const cy = gridTop + row * CELL + (CELL - ICON) / 2
    // Contain-fit: sources carry their REAL dimensions (a low-res-only resource is honest, not
    // an error), so a non-square image centers in its box instead of stretching.
    const scale = Math.min(ICON / image.width, ICON / image.height)
    const w = image.width * scale
    const h = image.height * scale
    ctx.drawImage(image, cx + (ICON - w) / 2, cy + (ICON - h) / 2, w, h)
  })
}

function drawText(
  ctx: CanvasRenderingContext2D,
  value: string,
  size: number,
  weight: number,
  color: string,
  x: number,
  y: number,
): void {
  ctx.font = `${weight} ${size}px ${FACE}`
  ctx.fillStyle = color
  ctx.fillText(value, x, y)
}

/** Loads one image; a single unloadable tile degrades to a blank cell, never fails the sheet. */
function loadImage(src: string): Promise<HTMLImageElement | null> {
  return new Promise((resolve) => {
    const img = new Image()
    img.onload = () => resolve(img)
    img.onerror = () => resolve(null)
    img.src = src
  })
}
