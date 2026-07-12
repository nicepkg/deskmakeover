//! Mac dev-host ICON adapters (M6-WIRE B3): the `#[cfg(not(windows))]` side of the icon
//! composition root. A shared in-memory icon desktop backs the reader + applier, so an apply is
//! VISIBLE to the next scan/read — the Mac-Tauri E2E exercises the real command → `IconOps` →
//! `TxnDriver` → port pipeline end to end; only the shell/registry syscalls are faked.
//!
//! Icon source extraction has no cross-platform Rust core (unlike the wallpaper decoder), so the
//! dev host SYNTHESIZES distinct 256px stand-in sources rather than depending on the gitignored
//! real-icon pack — a fresh clone runs the pipeline with zero setup. The real Windows extraction
//! is `[WINDOWS-VERIFY]`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use dm_domain::{
    ApplyAssets, DecodedImage, DesktopItem, DesktopScanner, ExplorerRefresher, Fingerprint,
    IconApplier, IconSourceExtractor, ItemId, ItemKind, ItemState, ItemStateReader, ItemTarget,
    OverlayControl, OverlayOutcome, OverlayStyle, PortError, PortResult, RestoreAnchor,
};
use image::ImageEncoder;

/// The rendered source size the compositor bakes from (spec 06 §2).
const ICON_PX: u32 = 256;

/// One fake desktop item's static definition. `hue` is `0xRRGGBB` for its synthesized source.
struct DevIcon {
    id: &'static str,
    name: &'static str,
    kind: ItemKind,
    hue: u32,
}

/// The default fake desktop: a system item, the Recycle Bin (two visual states), apps, a folder,
/// and loose files — enough kinds that the E2E covers shortcut/system/folder/file/recycle-bin +
/// the paired-empty path.
const DEV_ICONS: &[DevIcon] = &[
    DevIcon { id: "bin", name: "回收站", kind: ItemKind::RecycleBin, hue: 0x3F_B6A8 },
    DevIcon { id: "thispc", name: "此电脑", kind: ItemKind::System, hue: 0x5B_8DEF },
    DevIcon { id: "edge", name: "Edge", kind: ItemKind::Shortcut, hue: 0x2A_9DF4 },
    DevIcon { id: "code", name: "VS Code", kind: ItemKind::Shortcut, hue: 0x2C_7BD6 },
    DevIcon { id: "store", name: "Minecraft", kind: ItemKind::AppxShortcut, hue: 0x6A_A84F },
    DevIcon { id: "docs", name: "工作", kind: ItemKind::Folder, hue: 0xE0_A030 },
    DevIcon { id: "report", name: "季度报告.docx", kind: ItemKind::RegularFile, hue: 0x3B_78C7 },
    DevIcon { id: "notes", name: "会议纪要.txt", kind: ItemKind::RegularFile, hue: 0x8A_8A8A },
];

/// The deterministic virtual-desktop path for a dev item (the styleable surface's key).
fn dev_path(id: &str) -> String {
    format!("C:/Users/Dev/Desktop/{id}")
}

fn def_of(id: &str) -> Option<&'static DevIcon> {
    DEV_ICONS.iter().find(|d| d.id == id)
}

/// A ready-to-style [`DesktopItem`] from a dev definition.
fn dev_item(def: &DevIcon) -> DesktopItem {
    DesktopItem {
        id: ItemId::from_raw(def.id),
        name: def.name.into(),
        path: dev_path(def.id),
        kind: def.kind,
        icon: None,
        state: ItemState::Ready,
        requires_explicit_consent: false,
        status_message: None,
    }
}

/// The shared virtual icon desktop: each item's current styleable-surface bytes. Seeded with an
/// "original" marker so a fresh scan can fingerprint + capture the true original; an apply
/// overwrites it, a restore reverts it — exactly what makes the real transaction + CAS + restore
/// path testable on Mac.
pub struct DevIconDesktop {
    state: Mutex<HashMap<String, Vec<u8>>>,
}

impl DevIconDesktop {
    pub fn new() -> Arc<Self> {
        let mut state = HashMap::new();
        for it in DEV_ICONS {
            state.insert(dev_path(it.id), format!("original:{}", it.id).into_bytes());
        }
        Arc::new(Self { state: Mutex::new(state) })
    }

    fn bytes(&self, path: &str) -> Option<Vec<u8>> {
        self.state.lock().unwrap().get(path).cloned()
    }

    fn put(&self, path: &str, bytes: Vec<u8>) {
        self.state.lock().unwrap().insert(path.to_string(), bytes);
    }
}

/// Enumerates the fixed fake desktop (positions are the host's synthetic layout, [WV] on Windows).
pub struct DevDesktopScanner;

impl DesktopScanner for DevDesktopScanner {
    fn scan(&self) -> PortResult<Vec<DesktopItem>> {
        Ok(DEV_ICONS.iter().map(dev_item).collect())
    }
}

/// Synthesizes distinct 256px stand-in sources: `[0]` the item's hue, and for the Recycle Bin a
/// greyed `[1]` empty-state so the paired-asset path is exercised.
pub struct DevIconSourceExtractor;

impl IconSourceExtractor for DevIconSourceExtractor {
    fn extract(&self, item: &DesktopItem) -> PortResult<Vec<DecodedImage>> {
        let def = def_of(item.id.as_str())
            .ok_or_else(|| PortError::NotFound(format!("no dev icon for {}", item.id.as_str())))?;
        let mut sources = vec![synth_source(def.hue)];
        if def.kind == ItemKind::RecycleBin {
            sources.push(synth_source(0xB0_B0B0)); // the empty bin reads greyed
        }
        Ok(sources)
    }
}

/// A 256px straight-alpha RGBA source: a solid rounded field in `hue` on a transparent bed, so the
/// compositor has real silhouette + colour to style.
fn synth_source(hue: u32) -> DecodedImage {
    let (r, g, b) = (((hue >> 16) & 0xff) as u8, ((hue >> 8) & 0xff) as u8, (hue & 0xff) as u8);
    let margin = 26i32;
    let img = image::RgbaImage::from_fn(ICON_PX, ICON_PX, |x, y| {
        let (x, y) = (x as i32, y as i32);
        let n = ICON_PX as i32;
        let inside = x >= margin && x < n - margin && y >= margin && y < n - margin;
        if inside {
            image::Rgba([r, g, b, 255])
        } else {
            image::Rgba([0, 0, 0, 0])
        }
    });
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(&img, ICON_PX, ICON_PX, image::ExtendedColorType::Rgba8)
        .expect("in-memory PNG encode cannot fail");
    DecodedImage { width: ICON_PX, height: ICON_PX, png }
}

/// Reads the virtual desktop's current styleable-surface state (the CAS + restore-anchor source).
pub struct DevIconReader(pub Arc<DevIconDesktop>);

impl ItemStateReader for DevIconReader {
    fn read_fingerprint(&self, target: &ItemTarget) -> PortResult<Fingerprint> {
        self.0
            .bytes(&target.path)
            .map(|b| Fingerprint::of_bytes(&b))
            .ok_or_else(|| PortError::NotFound(target.path.clone()))
    }

    fn capture_anchor(&self, target: &ItemTarget) -> PortResult<RestoreAnchor> {
        self.0
            .bytes(&target.path)
            .map(|bytes| RestoreAnchor::FileBytes { bytes })
            .ok_or_else(|| PortError::NotFound(target.path.clone()))
    }
}

/// Applies a style by writing deterministic styled bytes derived from the asset set, so the
/// promised fingerprint matches the read-back (the driver's non-tautological verify, P1-4);
/// restores by replaying the captured original bytes.
pub struct DevIconApplier(pub Arc<DevIconDesktop>);

impl IconApplier for DevIconApplier {
    fn apply(&self, target: &ItemTarget, assets: &ApplyAssets) -> PortResult<Fingerprint> {
        let styled = dev_styled(assets);
        self.0.put(&target.path, styled.clone());
        Ok(Fingerprint::of_bytes(&styled))
    }

    fn restore(&self, target: &ItemTarget, anchor: &RestoreAnchor) -> PortResult<()> {
        match anchor {
            RestoreAnchor::FileBytes { bytes } => {
                self.0.put(&target.path, bytes.clone());
                Ok(())
            }
            other => Err(PortError::Unsupported(format!("dev host cannot restore {other:?}"))),
        }
    }
}

/// Deterministic styled bytes from the asset refs (primary + paired empty) — the styleable surface
/// an apply establishes, independent of the ICO bytes, so apply's promise and the read-back agree.
fn dev_styled(assets: &ApplyAssets) -> Vec<u8> {
    let mut s = format!("styled:{}", assets.primary.hash).into_bytes();
    if let Some(empty) = &assets.empty {
        s.extend_from_slice(format!(":empty:{}", empty.hash).as_bytes());
    }
    s
}

/// The dev overlay control: the machine-wide arrow verb always "succeeds" (no elevation on Mac).
pub struct DevOverlayControl;

impl OverlayControl for DevOverlayControl {
    fn apply(&self, _style: OverlayStyle, _ico_path: &str) -> PortResult<OverlayOutcome> {
        Ok(OverlayOutcome::Applied)
    }

    fn restore(&self) -> PortResult<OverlayOutcome> {
        Ok(OverlayOutcome::Applied)
    }
}

/// The dev Explorer refresher: a no-op (no shell to nudge).
pub struct DevExplorerRefresher;

impl ExplorerRefresher for DevExplorerRefresher {
    fn notify_icons_changed(&self) -> PortResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_enumerates_the_fixed_desktop_with_a_recycle_bin() {
        let items = DevDesktopScanner.scan().unwrap();
        assert_eq!(items.len(), DEV_ICONS.len());
        assert!(items.iter().any(|i| i.kind == ItemKind::RecycleBin));
        assert!(items.iter().all(|i| i.can_style()), "every dev item is ready to style");
    }

    #[test]
    fn extract_yields_a_256px_primary_and_a_paired_empty_for_the_bin() {
        let bin = dev_item(def_of("bin").unwrap());
        let sources = DevIconSourceExtractor.extract(&bin).unwrap();
        assert_eq!(sources.len(), 2, "the bin ships primary + empty");
        assert_eq!((sources[0].width, sources[0].height), (ICON_PX, ICON_PX));
        assert_eq!(&sources[0].png[0..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);

        let edge = dev_item(def_of("edge").unwrap());
        assert_eq!(DevIconSourceExtractor.extract(&edge).unwrap().len(), 1, "a shortcut ships one");
    }

    #[test]
    fn apply_is_visible_to_the_next_read_and_restore_reverts() {
        let desk = DevIconDesktop::new();
        let reader = DevIconReader(desk.clone());
        let applier = DevIconApplier(desk);
        let target = ItemTarget::new(ItemId::from_raw("edge"), ItemKind::Shortcut, dev_path("edge"));

        let original = reader.read_fingerprint(&target).unwrap();
        let anchor = reader.capture_anchor(&target).unwrap();

        let assets = ApplyAssets::single(dm_domain::AssetRef::new("hashX", "assets/hashX.ico"));
        let promised = applier.apply(&target, &assets).unwrap();
        // The applier's promise matches the live read-back (non-tautological verify).
        assert_eq!(promised, reader.read_fingerprint(&target).unwrap());
        assert_ne!(promised, original, "the styled surface differs from the original");

        applier.restore(&target, &anchor).unwrap();
        assert_eq!(reader.read_fingerprint(&target).unwrap(), original, "restore reverts exactly");
    }
}
