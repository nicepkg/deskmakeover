//! Native headless master bake — the webview-less render path (spec 07 §15).
//!
//! The resident process has no JS host, so background rendering can ONLY call the native
//! `dm-icon-core` kernel; this is the thin adapter from "a 256px source PNG + a resolved core
//! `Config`" to "the base64 master PNG the packaging path already consumes"
//! ([`super::package::BufferedMaster`]). The foreground webview bake and this path converge on
//! `package_masters` → `TxnDriver::apply`, so the two pipelines cannot drift beyond the kernel
//! they share.

use base64::Engine;
use dm_icon_core::batch::{render_icons_par, IconJob};
use dm_icon_core::compose::{ComposeDiagnostics, RenderOpts};
use dm_icon_core::config::Config;
use dm_icon_core::raster::Raster;
use dm_icon_core::render_session::RenderSession;

use crate::error::{OperationError, Result};

/// The master edge every bake targets (spec 06 §2).
const MASTER_PX: usize = 256;

/// Decodes a source PNG into the kernel's RGBA raster. Any decodable size is accepted — the
/// kernel normalizes; the extractor advertises real dimensions honestly.
pub fn raster_from_png(png: &[u8]) -> Result<Raster> {
    let img = image::load_from_memory(png)
        .map_err(|e| OperationError::InvalidPayload(format!("source png decode: {e}")))?
        .into_rgba8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mut raster = Raster::new(w, h);
    raster.data.copy_from_slice(img.as_raw());
    Ok(raster)
}

/// FNV-1a over the source bytes — the advisory content hash `RenderSession::register` keys its
/// profile/fact caches by (native derives the real key from the raster, so collisions are safe).
pub fn source_hash(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Bakes one 256px master from a source PNG under `config`, returning the base64 PNG the
/// packaging path consumes. `field_seed` carries the hue-spread allocation (spec 07 §5: new
/// icons allocate against pinned existing seeds; `None` lets the kernel derive).
pub fn bake_master_png(
    session: &mut RenderSession,
    id: &str,
    source_png: &[u8],
    config: &Config,
    is_shortcut: bool,
    field_seed: Option<u32>,
) -> Result<String> {
    let raster = raster_from_png(source_png)?;
    session.register(id, source_hash(source_png), raster);
    session.set_look(config.clone());
    let mut diag = ComposeDiagnostics::default();
    let opts = RenderOpts { field_seed };
    let out = session
        .render(id, is_shortcut, false, MASTER_PX, &opts, &mut diag)
        .ok_or_else(|| OperationError::InvalidPayload(format!("native render produced nothing for {id}")))?;
    encode_master_png(&out)
}

/// Deterministically PNG-encodes a rendered master and returns it as base64 (the form the packaging
/// path consumes). Shared by the serial and batch bake paths so the encoding can never drift.
fn encode_master_png(tile: &Raster) -> Result<String> {
    let mut png = Vec::new();
    {
        use image::ImageEncoder;
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(
                &tile.data,
                tile.width as u32,
                tile.height as u32,
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|e| OperationError::InvalidPayload(format!("master png encode: {e}")))?;
    }
    Ok(base64::engine::general_purpose::STANDARD.encode(png))
}

/// One master-bake request for the parallel batch path, borrowing the caller's frozen inputs (source
/// bytes + config are immutable for the batch's lifetime).
pub struct BakeJob<'a> {
    pub source_png: &'a [u8],
    pub config: &'a Config,
    pub is_shortcut: bool,
}

/// Batch-bakes 256px masters across rayon (M6 kernel-speed Phase 3, WIRED). Decodes each source, then
/// renders every icon IN PARALLEL and PNG-encodes it. The result is BYTE-IDENTICAL to calling
/// `bake_master_png` serially per job and is returned in INPUT order — `render_icons_par` guarantees
/// both, and every icon is a pure function of its own `(source, config, size, flags)` plus the boot-once
/// `NATIVE_ARROW`, so which thread renders which icon never changes a byte (batch.rs §byte-safety). The
/// session's profile/fact cache is not used here (a batch is distinct sources — nothing to reuse), but
/// the per-worker `MaskCache` keeps Phase 1's shape-mask sharing; the caches are pure memos, so dropping
/// them is byte-neutral. Field seed is None (the version-switch and resident batch paths never set one).
/// A per-job decode/encode fault fails ONLY that job (its slot in the returned vec is `Err`).
///
/// Contract: `NATIVE_ARROW` must not be rewritten while this runs — set the arrow once at startup
/// (batch.rs §Downstream integration contract).
pub fn bake_masters_par(jobs: &[BakeJob]) -> Vec<Result<String>> {
    // Decode every source up front — the parallel render needs all rasters in hand, and a decode fault
    // must stay attached to its own job. Carry the error as a String so the (owned) result can be
    // reassembled without holding a borrow of the decoded set.
    let decoded: Vec<std::result::Result<Raster, String>> =
        jobs.iter().map(|j| raster_from_png(j.source_png).map_err(|e| e.to_string())).collect();

    // Build render jobs for the successfully-decoded sources, in input order.
    let icon_jobs: Vec<IconJob> = decoded
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            r.as_ref().ok().map(|raster| IconJob {
                source: raster,
                config: jobs[i].config,
                is_shortcut: jobs[i].is_shortcut,
                show_original: false,
                size: MASTER_PX,
                opts: RenderOpts { field_seed: None },
            })
        })
        .collect();

    // Render all decoded icons in parallel (input-ordered), then encode each. `tiles` aligns 1:1 with
    // the Ok entries of `decoded` in order, so a single forward cursor reattaches each tile to its job.
    let tiles = render_icons_par(&icon_jobs);
    let mut tiles = tiles.into_iter();
    decoded
        .iter()
        .map(|r| match r {
            Ok(_) => {
                let tile = tiles.next().expect("exactly one rendered tile per decoded source");
                encode_master_png(&tile)
            }
            Err(msg) => Err(OperationError::InvalidPayload(msg.clone())),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dm_icon_core::config::{
        Band, Distinction, FilterStyle, MarkStyle, MonoStyle, PlateFallback, Subject,
    };
    use dm_icon_core::shapes::IconShape;

    fn source_png(r: u8, g: u8, b: u8) -> Vec<u8> {
        use image::ImageEncoder;
        let img = image::RgbaImage::from_pixel(256, 256, image::Rgba([r, g, b, 255]));
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&img, 256, 256, image::ExtendedColorType::Rgba8)
            .unwrap();
        png
    }

    fn config() -> Config {
        Config {
            shape: IconShape::Circle,
            subject: Subject::Original,
            tint: 0xFF6F5E,
            mono_style: MonoStyle::Tonal,
            plate_band: Band::Vivid,
            shortcut_shape: None,
            distinction: Distinction::None,
            mark_style: MarkStyle::Glass,
            mark_color: None,
            filter: FilterStyle::None,
            plate_color: None,
            plate_fallback: PlateFallback::Derived,
        }
    }

    #[test]
    fn bakes_a_256px_master_that_differs_from_the_raw_source() {
        let mut session = RenderSession::new();
        let src = source_png(40, 90, 200);
        let b64 = bake_master_png(&mut session, "item-1", &src, &config(), false, None).unwrap();
        let bytes = base64::engine::general_purpose::STANDARD.decode(b64).unwrap();
        let img = image::load_from_memory(&bytes).unwrap();
        assert_eq!((img.width(), img.height()), (256, 256));
        assert_ne!(bytes, src, "the styled master is not the raw source");
    }

    #[test]
    fn distinct_sources_bake_through_one_session_without_aliasing() {
        let mut session = RenderSession::new();
        let a = bake_master_png(&mut session, "a", &source_png(200, 30, 30), &config(), true, None)
            .unwrap();
        let b = bake_master_png(&mut session, "b", &source_png(30, 200, 30), &config(), true, None)
            .unwrap();
        assert_ne!(a, b, "two different sources render two different masters");
    }

    #[test]
    fn junk_source_bytes_error_cleanly() {
        let mut session = RenderSession::new();
        assert!(bake_master_png(&mut session, "x", b"not a png", &config(), false, None).is_err());
    }

    #[test]
    fn bake_masters_par_is_byte_identical_to_serial_bake() {
        // The byte-safety proof for the M6 Phase 3 wiring (codex R2 C-7): the parallel batch must
        // produce EXACTLY the same masters, in the same order, as the serial session bake — the kernel
        // is a pure function and the caches are memos, so parallelism only changes which thread renders
        // which icon. A cache that were NOT a pure memo would fail here.
        let cfg = config();
        let srcs = [source_png(40, 90, 200), source_png(200, 30, 30), source_png(30, 200, 60)];
        let shortcut = [false, true, false];

        let mut session = RenderSession::new();
        let serial: Vec<String> = (0..srcs.len())
            .map(|i| bake_master_png(&mut session, &format!("i{i}"), &srcs[i], &cfg, shortcut[i], None).unwrap())
            .collect();

        let jobs: Vec<BakeJob> = (0..srcs.len())
            .map(|i| BakeJob { source_png: &srcs[i], config: &cfg, is_shortcut: shortcut[i] })
            .collect();
        let batch: Vec<String> = bake_masters_par(&jobs).into_iter().map(|r| r.unwrap()).collect();

        assert_eq!(serial, batch, "rayon batch bake must be byte-identical to the serial session bake");
    }

    #[test]
    fn bake_masters_par_isolates_a_per_job_decode_fault() {
        // A junk source fails ONLY its own slot; the neighbours still bake, in order.
        let cfg = config();
        let good_a = source_png(10, 20, 30);
        let good_b = source_png(200, 100, 50);
        let junk: &[u8] = b"not a png";
        let jobs = vec![
            BakeJob { source_png: &good_a, config: &cfg, is_shortcut: false },
            BakeJob { source_png: junk, config: &cfg, is_shortcut: false },
            BakeJob { source_png: &good_b, config: &cfg, is_shortcut: true },
        ];
        let out = bake_masters_par(&jobs);
        assert!(out[0].is_ok(), "job 0 (good) bakes");
        assert!(out[1].is_err(), "job 1 (junk) fails only its own slot");
        assert!(out[2].is_ok(), "job 2 (good) still bakes after the fault");

        // The surviving jobs match a serial bake of just those two.
        let mut session = RenderSession::new();
        assert_eq!(out[0].as_ref().unwrap(), &bake_master_png(&mut session, "a", &good_a, &cfg, false, None).unwrap());
        assert_eq!(out[2].as_ref().unwrap(), &bake_master_png(&mut session, "b", &good_b, &cfg, true, None).unwrap());
    }
}
