//! An RAII single-threaded-apartment (STA) guard, ported from `DeskMakeover.Shell/StaThread.cs`.
//! Shell COM (`IFolderView2`, `IDesktopWallpaper`, `IShellLink`) requires STA; on an MTA thread
//! these calls silently misbehave, so entering STA is correctness, not convenience.

use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

/// Owns this thread's COM initialization. `CoUninitialize` runs on drop, on the same thread — so
/// this must live for exactly the lifetime of the STA thread that created it.
pub struct Apartment {
    _not_send: std::marker::PhantomData<*const ()>,
}

impl Apartment {
    /// Initializes the current thread as an STA. `S_FALSE` (already initialized on this thread) is
    /// treated as success, matching COM's contract.
    ///
    /// [WINDOWS-VERIFY] runtime: exercised only on the owner's Windows box.
    pub fn enter_sta() -> windows::core::Result<Self> {
        // SAFETY: `CoInitializeEx` is the documented entry point for COM on this thread; it is
        // paired with the `CoUninitialize` in `Drop`, and the guard is `!Send` so the pairing
        // cannot be split across threads.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        // `ok()` maps S_OK/S_FALSE to Ok and real failures to Err.
        hr.ok()?;
        Ok(Self { _not_send: std::marker::PhantomData })
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: balances the `CoInitializeEx` in `enter_sta`, on the same thread.
        unsafe { CoUninitialize() };
    }
}
