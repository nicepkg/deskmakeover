//! Wallpaper host glue (M6-WIRE A6): assembles the THIN `wallpaper.*` command results
//! from the ports (owner ruling D1 — no looks, no grids, no reconcile here; the
//! frontend store owns those) and serves decoded sources over the `dmwallpaper://`
//! custom protocol so pixel buffers never ride the JSON bridge.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use dm_contracts::{
    MonitorBounds, ScreenInfoDto, ScreenOrientation, WallpaperPosition, WallpaperResultDto,
    WallpaperScreensDto, WallpaperSourceDto,
};
use dm_domain::{DecodedImage, ImageDecoder, MonitorTopology, Orientation, WallpaperApplier};
use dm_operations::{SnapshotStore, WallpaperOps};

/// One monitor's decoded current wallpaper, cached for the protocol handler. `rev`
/// bumps whenever the underlying path changes, cache-busting the webview's image.
struct CachedSource {
    rev: u64,
    path: String,
    image: DecodedImage,
}

pub struct WallpaperHost {
    topology: Arc<dyn MonitorTopology + Send + Sync>,
    applier: Arc<dyn WallpaperApplier + Send + Sync>,
    decoder: Arc<dyn ImageDecoder + Send + Sync>,
    snapshot: SnapshotStore,
    baked_dir: PathBuf,
    /// keyed by the sanitized monitor id (the token in the protocol URL path).
    cache: Mutex<HashMap<String, CachedSource>>,
    /// Serializes apply/restore. Two concurrent FIRST applies would otherwise race
    /// the snapshot-once check-then-act: the loser captures the already-styled
    /// desktop and overwrites the true original — the exact data-loss trap the
    /// snapshot exists to prevent.
    mutate: Mutex<()>,
}

impl WallpaperHost {
    pub fn new(
        topology: Arc<dyn MonitorTopology + Send + Sync>,
        applier: Arc<dyn WallpaperApplier + Send + Sync>,
        decoder: Arc<dyn ImageDecoder + Send + Sync>,
        data_dir: &std::path::Path,
    ) -> Self {
        Self {
            topology,
            applier,
            decoder,
            snapshot: SnapshotStore::new(data_dir.join("wallpaper-snapshot.json")),
            baked_dir: data_dir.join("baked-wallpapers"),
            cache: Mutex::new(HashMap::new()),
            mutate: Mutex::new(()),
        }
    }

    /// `wallpaper.getScreens`: raw screens + globals. Decodes each monitor's current
    /// source (cached by path) and hands the webview a protocol URL + true dims.
    pub fn screens(&self) -> Result<WallpaperScreensDto, String> {
        let topo = self.topology.enumerate().map_err(|e| e.to_string())?;
        let span_active = topo.span_active();
        let mut screens = Vec::with_capacity(topo.monitors.len());
        for m in &topo.monitors {
            let source = match &m.source_path {
                Some(path) => self.decoded_source(&m.monitor_id, path),
                None => None,
            };
            screens.push(ScreenInfoDto {
                monitor_id: m.monitor_id.clone(),
                name: m.name.clone(),
                bounds: MonitorBounds { x: m.bounds.x, y: m.bounds.y, w: m.bounds.w, h: m.bounds.h },
                orientation: match m.bounds.orientation() {
                    Orientation::Portrait => ScreenOrientation::Portrait,
                    Orientation::Landscape => ScreenOrientation::Landscape,
                },
                source,
                slideshow_active: m.slideshow_active,
                has_readable_source: m.has_readable_source,
            });
        }
        // The durable snapshot flag rides getScreens so a COLD START surfaces the
        // whole-desktop restore affordance. A corrupt snapshot reads as no-backup
        // here (a restore of it would fail anyway; hiding the affordance is safer
        // than offering a broken one — the corruption still fails closed in apply).
        let has_backup = matches!(self.snapshot.load(), Ok(Some(_)));
        Ok(WallpaperScreensDto {
            screens,
            position: map_position(topo.position),
            span_active,
            has_backup,
        })
    }

    /// `wallpaper.applyBaked` — the ops layer owns snapshot-once + materialization.
    pub fn apply_baked(&self, monitor_id: &str, png_base64: &str) -> Result<WallpaperResultDto, String> {
        let _serialized = self.mutate.lock().unwrap();
        let ops = WallpaperOps::new(&*self.applier, &self.snapshot, &self.baked_dir);
        let outcome = ops.apply_baked(monitor_id, png_base64).map_err(|e| e.to_string())?;
        Ok(WallpaperResultDto { ok: true, toast: None, has_backup: outcome.has_backup })
    }

    /// `wallpaper.restore` — `monitor_id == "all"` reverts the whole desktop.
    pub fn restore(&self, monitor_id: &str) -> Result<WallpaperResultDto, String> {
        let _serialized = self.mutate.lock().unwrap();
        let ops = WallpaperOps::new(&*self.applier, &self.snapshot, &self.baked_dir);
        let outcome = ops.restore(monitor_id).map_err(|e| e.to_string())?;
        Ok(WallpaperResultDto { ok: true, toast: None, has_backup: outcome.has_backup })
    }

    /// Protocol lookup: the PNG bytes for `dmwallpaper://…/<key>?rev=N`.
    pub fn png_for(&self, key: &str) -> Option<Vec<u8>> {
        self.cache.lock().unwrap().get(key).map(|c| c.image.png.clone())
    }

    /// Decode-with-cache: re-decodes only when the monitor's source PATH changes
    /// (baked files are content-hashed, so a re-apply always changes the path).
    fn decoded_source(&self, monitor_id: &str, path: &str) -> Option<WallpaperSourceDto> {
        let key = sanitize(monitor_id);
        let mut cache = self.cache.lock().unwrap();
        if let Some(hit) = cache.get(&key) {
            if hit.path == path {
                return Some(source_dto(&key, hit));
            }
        }
        match self.decoder.decode(path) {
            Ok(image) => {
                let rev = cache.get(&key).map_or(1, |c| c.rev + 1);
                let entry = CachedSource { rev, path: path.to_string(), image };
                let dto = source_dto(&key, &entry);
                cache.insert(key, entry);
                Some(dto)
            }
            // An undecodable current wallpaper degrades to "no readable source" for
            // THIS screen (frontend shows the import CTA) instead of failing the
            // whole getScreens.
            Err(e) => {
                log::warn!("wallpaper decode failed for {monitor_id} ({path}): {e}");
                None
            }
        }
    }
}

fn source_dto(key: &str, cached: &CachedSource) -> WallpaperSourceDto {
    WallpaperSourceDto {
        url: protocol_url(key, cached.rev),
        width: cached.image.width,
        height: cached.image.height,
    }
}

/// The platform-correct custom-protocol URL (what tauri's convertFileSrc would build).
/// [WINDOWS-VERIFY] the `http://dmwallpaper.localhost` WebView2 form on the real box.
fn protocol_url(key: &str, rev: u64) -> String {
    if cfg!(windows) {
        format!("http://dmwallpaper.localhost/{key}?rev={rev}")
    } else {
        format!("dmwallpaper://localhost/{key}?rev={rev}")
    }
}

/// Same alphabet as the baked-file names: monitor device paths (`\\?\DISPLAY#…`)
/// carry URL/file-hostile characters; keep alphanumerics, map the rest to `_`.
fn sanitize(monitor_id: &str) -> String {
    monitor_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn map_position(p: dm_domain::WallpaperPosition) -> WallpaperPosition {
    use dm_domain::WallpaperPosition as D;
    match p {
        D::Center => WallpaperPosition::Center,
        D::Tile => WallpaperPosition::Tile,
        D::Stretch => WallpaperPosition::Stretch,
        D::Fit => WallpaperPosition::Fit,
        D::Fill => WallpaperPosition::Fill,
        D::Span => WallpaperPosition::Span,
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use dm_operations::RustImageDecoder;

    use super::*;
    use crate::devhost::{DevDesktop, DevMonitorTopology, DevWallpaperApplier};

    fn host(dir: &std::path::Path) -> WallpaperHost {
        let desk = DevDesktop::new();
        WallpaperHost::new(
            Arc::new(DevMonitorTopology(desk.clone())),
            Arc::new(DevWallpaperApplier(desk)),
            Arc::new(RustImageDecoder),
            dir,
        )
    }

    fn baked_png_base64() -> String {
        use base64::Engine;
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([10, 20, 30, 255]));
        let mut png = Vec::new();
        image::ImageEncoder::write_image(
            image::codecs::png::PngEncoder::new(&mut png),
            &img,
            2,
            2,
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();
        base64::engine::general_purpose::STANDARD.encode(png)
    }

    #[test]
    fn screens_reports_has_backup_across_the_snapshot_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        // Cold start: no snapshot yet → restore affordance stays hidden.
        assert!(!h.screens().unwrap().has_backup);
        let m0 = h.screens().unwrap().screens[0].monitor_id.clone();
        // Apply captures the pre-first-apply snapshot → getScreens now advertises it,
        // so a RESTART would still surface the whole-desktop restore (the fixed gap).
        h.apply_baked(&m0, &baked_png_base64()).unwrap();
        assert!(h.screens().unwrap().has_backup);
        // restore('all') consumes + clears the snapshot → back to no-backup.
        h.restore("all").unwrap();
        assert!(!h.screens().unwrap().has_backup);
    }

    #[test]
    fn screens_decodes_real_dev_wallpapers_with_protocol_urls() {
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        let dto = h.screens().unwrap();
        assert_eq!(dto.screens.len(), 2);
        assert!(!dto.span_active);
        assert!(!dto.has_backup, "a fresh host has no snapshot");
        for s in &dto.screens {
            let src = s.source.as_ref().expect("dev screens start with a wallpaper");
            assert!(src.url.starts_with("dmwallpaper://localhost/"), "{}", src.url);
            assert!(src.width > 0 && src.height > 0, "true decoded dims required");
        }
        // Portrait secondary is shaped by bounds, not by the image.
        assert!(matches!(dto.screens[1].orientation, ScreenOrientation::Portrait));
        // The protocol can serve every advertised URL.
        for s in &dto.screens {
            let key = s.source.as_ref().unwrap().url.split('/').next_back().unwrap();
            let key = key.split('?').next().unwrap();
            assert!(h.png_for(key).is_some(), "protocol miss for {key}");
        }
    }

    #[test]
    fn apply_bumps_rev_and_next_screens_serves_the_baked_image() {
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        let before = h.screens().unwrap();
        let m0 = before.screens[0].monitor_id.clone();
        let url_before = before.screens[0].source.as_ref().unwrap().url.clone();

        let res = h.apply_baked(&m0, &baked_png_base64()).unwrap();
        assert!(res.ok && res.has_backup);

        let after = h.screens().unwrap();
        let s0 = after.screens[0].source.as_ref().unwrap();
        assert_ne!(s0.url, url_before, "rev must bump when the path changes");
        assert_eq!((s0.width, s0.height), (2, 2), "now serving the baked 2x2 PNG");
    }

    #[test]
    fn restore_all_returns_the_original_and_drops_backup() {
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        let m0 = h.screens().unwrap().screens[0].monitor_id.clone();
        h.apply_baked(&m0, &baked_png_base64()).unwrap();

        let res = h.restore("all").unwrap();
        assert!(res.ok && !res.has_backup);
        let after = h.screens().unwrap();
        let s0 = after.screens[0].source.as_ref().unwrap();
        assert!(s0.width > 2, "original repo wallpaper is back, not the 2x2 bake");
        // A second restore has nothing to work from.
        assert!(h.restore("all").is_err());
    }

    #[test]
    fn cache_reuses_rev_for_an_unchanged_path() {
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        let a = h.screens().unwrap().screens[0].source.as_ref().unwrap().url.clone();
        let b = h.screens().unwrap().screens[0].source.as_ref().unwrap().url.clone();
        assert_eq!(a, b, "same path must keep the same rev URL");
    }
}
