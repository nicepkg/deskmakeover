//! Wallpaper domain types: the multi-monitor topology (read), the pre-first-apply
//! restore snapshot (capture/restore), and decoded-image data. Pure data — no COM,
//! no I/O — so the port traits in [`crate::ports`] that speak them cross-check
//! cleanly for `x86_64-pc-windows-msvc` and are faked on the Mac host.
//!
//! Owner ruling D1 (2026-07-12): Rust is thin platform I/O — read screen info,
//! get/set wallpaper, capture/restore the snapshot. Reconcile, per-monitor draft
//! persistence, and `WallpaperStateDto` assembly are FRONTEND, so these types carry
//! NO look/zone/grid data.

use serde::{Deserialize, Serialize};

/// Virtual-desktop bounds of one monitor, in physical pixels (`GetMonitorRECT`).
/// `x`/`y` may be negative (a monitor left of / above the primary). Domain mirror
/// of the contract `MonitorBounds`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl MonitorRect {
    /// Portrait when strictly taller than wide; a square (or wider) is landscape.
    pub fn orientation(self) -> Orientation {
        if self.h > self.w {
            Orientation::Portrait
        } else {
            Orientation::Landscape
        }
    }
}

/// Screen orientation, derived from a [`MonitorRect`]'s aspect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Orientation {
    Portrait,
    Landscape,
}

/// GLOBAL wallpaper positioning (whole-desktop, not per-monitor). Domain mirror of
/// the contract `WallpaperPosition`; `Span` stretches ONE image across every
/// monitor, so per-screen isolation is undefined and the UI degrades to a unified
/// canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WallpaperPosition {
    Center,
    Tile,
    Stretch,
    Fit,
    Fill,
    Span,
}

/// One present monitor as the topology reports it: geometry + the CURRENT wallpaper
/// image path (pre-decode). The operations/command layer decodes `source_path` via
/// [`crate::ports::ImageDecoder`] to produce the dims the UI needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorInfo {
    /// Windows device path (`GetMonitorDevicePathAt`) — durable-ish, not permanent.
    pub monitor_id: String,
    pub name: String,
    pub bounds: MonitorRect,
    /// The current wallpaper image path (`GetWallpaper`), or `None` for a
    /// solid-colour desktop or an unreadable dynamic/video wallpaper.
    pub source_path: Option<String>,
    /// A running Windows slideshow on this monitor (rotation won't re-arm after apply).
    pub slideshow_active: bool,
    /// `GetWallpaper` returned a readable image path (`false` ⇒ dynamic/video
    /// wallpaper — distinct from a solid-colour desktop, which is readable-but-empty).
    pub has_readable_source: bool,
}

/// The whole multi-monitor topology: every present monitor plus the global
/// position. [`crate::ports::MonitorTopology::enumerate`] returns this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WallpaperTopology {
    pub monitors: Vec<MonitorInfo>,
    pub position: WallpaperPosition,
}

impl WallpaperTopology {
    /// `true` when the global position is `Span` (the UI degrades to a unified
    /// canvas). Detected here on the host rather than left for the frontend to guess.
    pub fn span_active(&self) -> bool {
        matches!(self.position, WallpaperPosition::Span)
    }
}

/// One present monitor's wallpaper in a restore snapshot: its device path and the
/// image it showed, or `None` when it showed the solid background colour (or a
/// slideshow frame). A `None` monitor is restored by clearing our image so the
/// restored background colour shows through. Detached monitors are NOT represented.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorWallpaper {
    pub monitor_id: String,
    pub image: Option<String>,
}

/// The full pre-first-apply restore snapshot: the global background colour +
/// position plus every present monitor's image. Byte-level restore needs the colour
/// and position, not just the images (P2-1), so a solid-colour or repositioned
/// desktop returns to exactly its prior state.
///
/// **This is the single highest-severity artifact in the feature.** It must be
/// captured BEFORE the first apply ever runs and persisted durably, or the first
/// apply permanently destroys the user's original desktop with no way back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WallpaperSnapshot {
    /// `COLORREF` (`0x00BBGGRR`) background colour, shown wherever no image covers
    /// the desktop.
    pub background_color: u32,
    /// The raw `DESKTOP_WALLPAPER_POSITION` enum value at capture — kept raw (not a
    /// [`WallpaperPosition`]) so restore re-applies the EXACT prior value (P2-1).
    pub position: i32,
    /// Whether a slideshow was active at capture. Restore is best-effort static.
    pub slideshow_active: bool,
    /// Per present monitor; detached monitors are omitted.
    pub monitors: Vec<MonitorWallpaper>,
}

/// A decoded wallpaper image: its true pixel dims + the re-encoded PNG bytes the
/// compositor renders from (served over the `dmwallpaper://<monitorId>?rev=N`
/// custom protocol). Produced by [`crate::ports::ImageDecoder`]. The dims are the
/// image's own, NOT the monitor bounds, so the compositor cover-crops to each
/// screen's aspect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orientation_is_portrait_only_when_taller_than_wide() {
        assert_eq!(MonitorRect { x: 0, y: 0, w: 1080, h: 1920 }.orientation(), Orientation::Portrait);
        assert_eq!(MonitorRect { x: 0, y: 0, w: 1920, h: 1080 }.orientation(), Orientation::Landscape);
        // A square is landscape (not strictly taller than wide).
        assert_eq!(MonitorRect { x: 0, y: 0, w: 1000, h: 1000 }.orientation(), Orientation::Landscape);
        // Negative origin (monitor left of primary) does not affect orientation.
        assert_eq!(MonitorRect { x: -1080, y: 0, w: 1080, h: 1920 }.orientation(), Orientation::Portrait);
    }

    #[test]
    fn span_active_only_for_span_position() {
        let base = |position| WallpaperTopology { monitors: vec![], position };
        assert!(base(WallpaperPosition::Span).span_active());
        for p in [
            WallpaperPosition::Center,
            WallpaperPosition::Tile,
            WallpaperPosition::Stretch,
            WallpaperPosition::Fit,
            WallpaperPosition::Fill,
        ] {
            assert!(!base(p).span_active());
        }
    }

    #[test]
    fn snapshot_round_trips_through_json() {
        let snap = WallpaperSnapshot {
            background_color: 0x00112233,
            position: 4,
            slideshow_active: false,
            monitors: vec![
                MonitorWallpaper { monitor_id: "m0".into(), image: Some("C:/a.jpg".into()) },
                MonitorWallpaper { monitor_id: "m1".into(), image: None },
            ],
        };
        let back: WallpaperSnapshot =
            serde_json::from_str(&serde_json::to_string(&snap).unwrap()).unwrap();
        assert_eq!(back, snap);
    }

    #[test]
    fn topology_round_trips_through_json() {
        let topo = WallpaperTopology {
            monitors: vec![MonitorInfo {
                monitor_id: "m0".into(),
                name: "primary".into(),
                bounds: MonitorRect { x: 0, y: 0, w: 3840, h: 2400 },
                source_path: Some("C:/wall.jpg".into()),
                slideshow_active: false,
                has_readable_source: true,
            }],
            position: WallpaperPosition::Fill,
        };
        let back: WallpaperTopology =
            serde_json::from_str(&serde_json::to_string(&topo).unwrap()).unwrap();
        assert_eq!(back, topo);
    }
}
