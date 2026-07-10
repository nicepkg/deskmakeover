//! The STA executor: one dedicated apartment-threaded worker that runs closures on behalf of the
//! rest of the process. This is the message-passing actor mandated by ADR-0019 Amendment 1 —
//! every closure creates, uses, and releases its COM interface pointers entirely on this one
//! thread, and only owned (`Send`) values cross back to the caller through a oneshot reply.
//!
//! ADR-0019 also mandates a **message loop**: a single-threaded apartment must pump its message
//! queue, or any COM call that marshals across apartments (a shell server behind
//! `IDesktopWallpaper`, the cross-process `IFolderView2` desktop-layout chain) deadlocks. The
//! previous implementation blocked on an `mpsc::recv`, never pumping, so it was correct only for
//! purely in-apartment interfaces (`IShellLinkW`/`IPersistFile`). This worker runs a real
//! `GetMessageW` pump and delivers jobs AS posted thread messages (`WM_STA_JOB`), so one loop is
//! both the message pump and the job dispatcher: in-apartment calls are unaffected and any
//! marshaling interface now has a pumping thread to marshal onto.
//!
//! [WINDOWS-VERIFY] the whole module is Windows-only (COM apartment + Win32 message queue); there
//! is no host-testable logic to unit-test on macOS.

use std::sync::mpsc;
use std::thread::JoinHandle;

use dm_domain::{PortError, PortResult};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PeekMessageW, PostThreadMessageW, TranslateMessage, MSG,
    PM_NOREMOVE, WM_APP, WM_QUIT,
};

use crate::com::apartment::Apartment;

/// A unit of work to run on the STA thread. It captures its own reply channel.
type Job = Box<dyn FnOnce() + Send + 'static>;

/// Private thread message carrying a `Box<Job>` raw pointer in its `lParam`.
const WM_STA_JOB: u32 = WM_APP + 1;

/// A handle to the dedicated STA worker thread. Dropping it posts `WM_QUIT` and joins the thread
/// (which then runs `CoUninitialize`).
pub struct StaExecutor {
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

impl StaExecutor {
    /// Spawns the STA thread and blocks until it has entered its apartment and created its message
    /// queue (or failed to). [WINDOWS-VERIFY] runtime.
    pub fn spawn() -> PortResult<Self> {
        // The ready channel carries the worker's thread id (needed to post jobs) or an init error.
        let (ready_tx, ready_rx) = mpsc::channel::<Result<u32, String>>();

        let thread = std::thread::Builder::new()
            .name("dm-windows-sta".to_string())
            .spawn(move || match Apartment::enter_sta() {
                Ok(apartment) => {
                    // Force the thread message queue into existence BEFORE publishing our id, so a
                    // caller's PostThreadMessageW can never race ahead of the queue.
                    let mut probe = MSG::default();
                    // SAFETY: a PM_NOREMOVE peek only creates the queue; it removes nothing.
                    let _ = unsafe { PeekMessageW(&mut probe, None, 0, 0, PM_NOREMOVE) };
                    // SAFETY: reads this thread's own id.
                    let id = unsafe { GetCurrentThreadId() };
                    let _ = ready_tx.send(Ok(id));
                    pump(apartment);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e.to_string()));
                }
            })
            .map_err(|e| PortError::Com(format!("failed to spawn STA thread: {e}")))?;

        match ready_rx.recv() {
            Ok(Ok(thread_id)) => Ok(Self { thread_id, thread: Some(thread) }),
            Ok(Err(e)) => Err(PortError::Com(format!("STA init failed: {e}"))),
            Err(e) => Err(PortError::Com(format!("STA thread never reported ready: {e}"))),
        }
    }

    /// Runs `work` on the STA thread and returns its result. `work` and `T` are `Send`; any COM
    /// interface pointers `work` creates stay on the STA thread and are dropped there.
    pub fn run<T, F>(&self, work: F) -> PortResult<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let (reply_tx, reply_rx) = mpsc::channel::<T>();
        let job: Job = Box::new(move || {
            // If the caller dropped the receiver, the send fails and the result is discarded.
            let _ = reply_tx.send(work());
        });
        // The job travels to the STA thread as the lParam of a posted message. `Job` is a fat
        // pointer, so box it once more to get a thin pointer that fits an isize lParam.
        let raw = Box::into_raw(Box::new(job));
        // SAFETY: posts WM_STA_JOB to the STA thread, which reconstructs and runs the Box<Job>.
        let posted =
            unsafe { PostThreadMessageW(self.thread_id, WM_STA_JOB, WPARAM(0), LPARAM(raw as isize)) };
        if let Err(e) = posted {
            // The message was never delivered, so we still own the box — reclaim it, don't leak.
            // SAFETY: `raw` came from Box::into_raw above and was not handed off.
            drop(unsafe { Box::from_raw(raw) });
            return Err(PortError::Com(format!("STA thread is gone: {e}")));
        }
        reply_rx
            .recv()
            .map_err(|_| PortError::Com("STA job dropped before replying".to_string()))
    }
}

impl Drop for StaExecutor {
    fn drop(&mut self) {
        // SAFETY: WM_QUIT makes the worker's GetMessageW return 0, ending the pump so the thread
        // can run CoUninitialize and exit.
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The STA message pump: dispatch queued window messages (so cross-apartment COM can marshal onto
/// this thread) and run any job posted as `WM_STA_JOB`. `_apartment` outlives the loop, so
/// `CoUninitialize` runs on this same thread after the last message.
fn pump(_apartment: Apartment) {
    let mut msg = MSG::default();
    loop {
        // SAFETY: standard STA message loop. GetMessageW returns >0 for a message, 0 for WM_QUIT,
        // and -1 on error; exit on 0 or -1.
        let got = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if got.0 <= 0 {
            break;
        }
        if msg.message == WM_STA_JOB {
            // SAFETY: the lParam is the Box<Job> raw pointer posted in `run`; reclaim and run it.
            let job = unsafe { Box::from_raw(msg.lParam.0 as *mut Job) };
            job();
        } else {
            // SAFETY: dispatch non-job messages so marshaled cross-apartment COM calls complete.
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}
