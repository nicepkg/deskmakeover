//! Filter-finish coverage through the public `apply_filter` dispatch (the finishes
//! themselves are private). Each finish had zero direct tests; these pin the
//! observable structure (gloss sheen gradient, pixel hard alpha, transparent-safety)
//! without asserting exact bytes the corpus already certifies.

use dm_icon_core::config::{FilterStyle, Subject};
use dm_icon_core::filters::apply_filter;
use dm_icon_core::raster::Raster;

fn solid(size: usize, rgba: [u8; 4]) -> Raster {
    let mut r = Raster::new(size, size);
    for p in r.data.chunks_exact_mut(4) {
        p.copy_from_slice(&rgba);
    }
    r
}

fn gradient(size: usize) -> Raster {
    let mut r = Raster::new(size, size);
    for y in 0..size {
        for x in 0..size {
            let i = (y * size + x) * 4;
            r.data[i] = (x * 255 / size) as u8;
            r.data[i + 1] = (y * 255 / size) as u8;
            r.data[i + 2] = 128;
            r.data[i + 3] = 255;
        }
    }
    r
}

fn px(tile: &Raster, x: usize, y: usize) -> [u8; 4] {
    let i = (y * tile.width + x) * 4;
    [tile.data[i], tile.data[i + 1], tile.data[i + 2], tile.data[i + 3]]
}

#[test]
fn gloss_lightens_the_top_and_deepens_the_bottom() {
    let size = 64;
    let mut tile = solid(size, [128, 128, 128, 255]);
    apply_filter(&mut tile, size, FilterStyle::Gloss, Subject::Original, 0x3366cc);
    let top = px(&tile, size / 2, 5);
    let bottom = px(&tile, size / 2, size - 4);
    assert!(top[0] > 128, "the top sheen must lighten the tile (got {})", top[0]);
    assert!(bottom[0] < 128, "the lower body must deepen (got {})", bottom[0]);
    assert_eq!(top[3], 255, "gloss keeps opaque pixels opaque");
}

#[test]
fn gloss_leaves_a_fully_transparent_tile_untouched() {
    let size = 32;
    let mut tile = Raster::new(size, size);
    apply_filter(&mut tile, size, FilterStyle::Gloss, Subject::Original, 0x3366cc);
    assert!(tile.data.iter().all(|&b| b == 0), "no opaque pixel → gloss is a no-op");
}

#[test]
fn pixelate_produces_hard_alpha_only() {
    let size = 48;
    let mut tile = gradient(size);
    apply_filter(&mut tile, size, FilterStyle::Pixel, Subject::Original, 0x3366cc);
    assert!(
        tile.data.chunks_exact(4).all(|p| p[3] == 0 || p[3] == 255),
        "pixelate writes hard alpha (0 or 255) at every pixel"
    );
    // Nearest-neighbour block expand: the top-left cell is a uniform block.
    assert_eq!(px(&tile, 0, 0), px(&tile, 1, 1), "a pixel cell is a solid block");
}

#[test]
fn glass_and_sticker_preserve_dimensions_and_never_panic() {
    let size = 64;
    for finish in [FilterStyle::Glass, FilterStyle::Sticker] {
        let mut tile = solid(size, [60, 150, 210, 255]);
        apply_filter(&mut tile, size, finish, Subject::Original, 0x3366cc);
        assert_eq!(tile.data.len(), size * size * 4, "{finish:?} must keep the tile size");
        assert!(tile.data.chunks_exact(4).any(|p| p[3] > 0), "{finish:?} must paint something");
    }
    // Mono glass takes the tinted-ramp branch (hue = Some) — exercise it too.
    let mut mono = solid(size, [60, 150, 210, 255]);
    apply_filter(&mut mono, size, FilterStyle::Glass, Subject::Mono, 0x33aa88);
    assert_eq!(mono.data.len(), size * size * 4);
}

#[test]
fn every_finish_is_safe_on_a_transparent_tile() {
    let size = 40;
    for finish in [FilterStyle::Gloss, FilterStyle::Glass, FilterStyle::Pixel, FilterStyle::Sticker] {
        let mut tile = Raster::new(size, size);
        apply_filter(&mut tile, size, finish, Subject::Original, 0x3366cc);
        assert_eq!(tile.data.len(), size * size * 4, "{finish:?} must not resize");
    }
}
