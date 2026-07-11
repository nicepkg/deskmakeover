//! Mac dev-host wallpaper adapters (M6-WIRE A5): the `#[cfg(not(windows))]` side of
//! the composition root. A shared in-memory "desktop" backs both the topology and the
//! applier, so an apply is VISIBLE to the next `getScreens` — the Mac-Tauri E2E
//! exercises the real command → ops → port pipeline end to end; only the final COM
//! syscall is faked. Mirrors the browser mock's default scenario: a landscape
//! primary + a portrait secondary, each showing a real repo wallpaper.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use dm_domain::{
    MonitorInfo, MonitorRect, MonitorTopology, MonitorWallpaper, PortResult, WallpaperApplier,
    WallpaperPosition, WallpaperSnapshot, WallpaperTopology,
};

const PRIMARY: &str = "\\\\?\\DISPLAY#DEV#0";
const PORTRAIT: &str = "\\\\?\\DISPLAY#DEV#1";

/// Repo wallpapers double as the dev desktop's images (same files the browser mock
/// serves over vite). Resolved from the crate dir so cwd never matters.
fn repo_wallpaper(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../public/real-icons/wallpapers")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

/// The shared fake desktop: monitor → current wallpaper path ("" = solid colour).
pub struct DevDesktop {
    walls: Mutex<HashMap<String, String>>,
}

impl DevDesktop {
    pub fn new() -> Arc<Self> {
        let mut walls = HashMap::new();
        walls.insert(PRIMARY.to_string(), repo_wallpaper("wallpaper-default.jpg"));
        walls.insert(PORTRAIT.to_string(), repo_wallpaper("wallpaper-dark.jpg"));
        Arc::new(Self { walls: Mutex::new(walls) })
    }

    fn wall_of(&self, id: &str) -> Option<String> {
        self.walls.lock().unwrap().get(id).filter(|p| !p.is_empty()).cloned()
    }
}

pub struct DevMonitorTopology(pub Arc<DevDesktop>);

impl MonitorTopology for DevMonitorTopology {
    fn enumerate(&self) -> PortResult<WallpaperTopology> {
        let monitor = |id: &str, name: &str, bounds: MonitorRect| {
            let source_path = self.0.wall_of(id);
            MonitorInfo {
                monitor_id: id.to_string(),
                name: name.to_string(),
                bounds,
                has_readable_source: source_path.is_some(),
                source_path,
                slideshow_active: false,
            }
        };
        Ok(WallpaperTopology {
            monitors: vec![
                monitor(PRIMARY, "Display 1", MonitorRect { x: 0, y: 0, w: 1920, h: 1080 }),
                monitor(PORTRAIT, "Display 2", MonitorRect { x: 1920, y: 0, w: 1080, h: 1920 }),
            ],
            position: WallpaperPosition::Fill,
        })
    }
}

pub struct DevWallpaperApplier(pub Arc<DevDesktop>);

impl WallpaperApplier for DevWallpaperApplier {
    fn capture(&self) -> PortResult<WallpaperSnapshot> {
        let walls = self.0.walls.lock().unwrap();
        let mut monitors: Vec<_> = walls
            .iter()
            .map(|(id, path)| MonitorWallpaper {
                monitor_id: id.clone(),
                image: (!path.is_empty()).then(|| path.clone()),
            })
            .collect();
        monitors.sort_by(|a, b| a.monitor_id.cmp(&b.monitor_id));
        Ok(WallpaperSnapshot {
            background_color: 0x0010_1010,
            position: 4, // raw DWPOS_FILL
            slideshow_active: false,
            monitors,
        })
    }

    fn set(&self, monitor_id: &str, image_path: &str) -> PortResult<()> {
        self.0.walls.lock().unwrap().insert(monitor_id.to_string(), image_path.to_string());
        Ok(())
    }

    fn restore(&self, snapshot: &WallpaperSnapshot) -> PortResult<()> {
        let mut walls = self.0.walls.lock().unwrap();
        for m in &snapshot.monitors {
            walls.insert(m.monitor_id.clone(), m.image.clone().unwrap_or_default());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_is_visible_to_the_next_enumerate() {
        let desk = DevDesktop::new();
        let topo = DevMonitorTopology(desk.clone());
        let applier = DevWallpaperApplier(desk);
        applier.set(PRIMARY, "/tmp/baked.png").unwrap();
        let t = topo.enumerate().unwrap();
        let primary = t.monitors.iter().find(|m| m.monitor_id == PRIMARY).unwrap();
        assert_eq!(primary.source_path.as_deref(), Some("/tmp/baked.png"));
    }

    #[test]
    fn capture_then_restore_round_trips_the_desktop() {
        let desk = DevDesktop::new();
        let topo = DevMonitorTopology(desk.clone());
        let applier = DevWallpaperApplier(desk);
        let before = applier.capture().unwrap();
        applier.set(PRIMARY, "/tmp/baked.png").unwrap();
        applier.set(PORTRAIT, "").unwrap(); // solid colour
        applier.restore(&before).unwrap();
        let t = topo.enumerate().unwrap();
        let by_id = |id: &str| t.monitors.iter().find(|m| m.monitor_id == id).unwrap().clone();
        assert!(by_id(PRIMARY).source_path.unwrap().ends_with("wallpaper-default.jpg"));
        assert!(by_id(PORTRAIT).source_path.unwrap().ends_with("wallpaper-dark.jpg"));
    }

    #[test]
    fn dev_wallpaper_files_exist_in_the_repo() {
        // The topology hands these paths to the REAL decoder; a moved/renamed asset
        // must fail here, not at app runtime.
        for m in DevMonitorTopology(DevDesktop::new()).enumerate().unwrap().monitors {
            let p = m.source_path.expect("dev monitors always start with a wallpaper");
            assert!(std::path::Path::new(&p).exists(), "missing dev wallpaper: {p}");
        }
    }
}
