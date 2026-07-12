//! Windows icon source extraction ([WINDOWS-VERIFY]).
//!
//! Extracts an item's normalized 256px source(s) for the compositor via the shell —
//! `IShellItemImageFactory::GetImage` (`SIIGBF_RESIZETOFIT`, 256×256) for files/folders/`.lnk`s,
//! the package logo for `AppxShortcut`, the per-user CLSID `DefaultIcon` for `System`/`RecycleBin`
//! (the Recycle Bin additionally extracting its empty-state icon) — then re-encodes to a
//! straight-alpha RGBA PNG the way the wallpaper decoder does.
//!
//! The extraction body is deferred to the Windows batch (it needs a real shell + a live desktop to
//! verify against, and cannot be exercised on the Mac host). The TYPE + composition are wired now so
//! the swap to a live implementation is a single method body — the icon host references this exactly
//! as it references the other `[WINDOWS-VERIFY]` shell adapters.

use std::sync::Arc;

use dm_domain::{DecodedImage, DesktopItem, IconSourceExtractor, PortError, PortResult};

use crate::com::StaExecutor;

/// Extracts 256px icon sources on the STA apartment (oracle: the C# shell icon extraction).
pub struct WindowsIconSourceExtractor {
    // Retained for the live implementation: `IShellItemImageFactory` COM must run on the STA thread
    // the executor owns, the same apartment the scanner/applier use.
    _exec: Arc<StaExecutor>,
}

impl WindowsIconSourceExtractor {
    pub fn new(exec: Arc<StaExecutor>) -> Self {
        Self { _exec: exec }
    }
}

impl IconSourceExtractor for WindowsIconSourceExtractor {
    fn extract(&self, item: &DesktopItem) -> PortResult<Vec<DecodedImage>> {
        // [WINDOWS-VERIFY] real IShellItemImageFactory / package-logo / CLSID-DefaultIcon extraction
        // (see the module docs + handoff §8). Wired but unimplemented so the icon host composition is
        // complete and the app boots; a scan surfaces this until the Windows batch fills the body.
        let _ = item;
        Err(PortError::Unsupported(
            "Windows icon source extraction is [WINDOWS-VERIFY] (IShellItemImageFactory, Windows batch)"
                .into(),
        ))
    }
}
