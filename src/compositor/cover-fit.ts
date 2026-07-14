import type { Sprite } from 'pixi.js'

// Shared cover-fit geometry (extracted from renderer.ts so the wallpaper source
// sprite AND the Liquid-Glass backdrop sprite land on the SAME pixels). A glass
// zone refracts the wallpaper behind it; if its backdrop were stretched while
// the displayed wallpaper is cover-fit, the refracted rim would not line up with
// the crisp wallpaper just outside the glass. One helper = one alignment.

/** Cover-fit a sprite over the screen: fill it while PRESERVING the source's
 *  aspect ratio, centred, letting the caller's mask/stage clip the overflow
 *  (Windows "Fill" position). `srcW/srcH` are the REAL source image dims. */
export function coverFit(sprite: Sprite, srcW: number, srcH: number, scrW: number, scrH: number): void {
  const texA = srcW > 0 && srcH > 0 ? srcW / srcH : scrW / scrH
  const scrA = scrW / scrH
  if (texA >= scrA) {
    // source is wider than the screen → match height, overflow + centre width
    sprite.height = scrH
    sprite.width = scrH * texA
    sprite.x = (scrW - sprite.width) / 2
    sprite.y = 0
  } else {
    // source is taller → match width, overflow + centre height
    sprite.width = scrW
    sprite.height = scrW / texA
    sprite.x = 0
    sprite.y = (scrH - sprite.height) / 2
  }
}
