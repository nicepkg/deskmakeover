//! The STA executor: one dedicated apartment-threaded worker that runs closures on behalf of the
//! rest of the process. This is the message-passing actor mandated by ADR-0019 Amendment 1 —
//! every closure is created, uses, and releases its COM interface pointers entirely on this one
//! thread, and only owned (`Send`) values cross back to the caller through a oneshot reply.
//!
//! It generalizes `DeskMakeover.Shell/StaThread.Run<T>` from a spawn-per-call model to a single
//! long-lived thread (the resident process holds one for its whole life).

use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;

use dm_domain::{PortError, PortResult};

use crate::com::apartment::Apartment;

/// A unit of work to run on the STA thread. It captures its own reply channel.
type Job = Box<dyn FnOnce() + Send + 'static>;

/// A handle to the dedicated STA worker thread. Dropping it closes the channel and joins the
/// thread (which then runs `CoUninitialize`).
pub struct StaExecutor {
    sender: Option<Sender<Job>>,
    thread: Option<JoinHandle<()>>,
}

impl StaExecutor {
    /// Spawns the STA thread and blocks until it has entered its apartment (or failed to).
    ///
    /// [WINDOWS-VERIFY] runtime.
    pub fn spawn() -> PortResult<Self> {
        let (sender, receiver) = mpsc::channel::<Job>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

        let thread = std::thread::Builder::new()
            .name("dm-windows-sta".to_string())
            .spawn(move || match Apartment::enter_sta() {
                Ok(_apartment) => {
                    let _ = ready_tx.send(Ok(()));
                    // Run jobs until the sender is dropped; `_apartment` outlives the loop, so
                    // `CoUninitialize` runs on this same thread after the last job.
                    while let Ok(job) = receiver.recv() {
                        job();
                    }
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e.to_string()));
                }
            })
            .map_err(|e| PortError::Com(format!("failed to spawn STA thread: {e}")))?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self { sender: Some(sender), thread: Some(thread) }),
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
            // If the caller dropped the receiver, the send simply fails and the result is discarded.
            let _ = reply_tx.send(work());
        });
        self.sender
            .as_ref()
            .ok_or_else(|| PortError::Com("STA executor already shut down".to_string()))?
            .send(job)
            .map_err(|_| PortError::Com("STA thread is gone".to_string()))?;
        reply_rx
            .recv()
            .map_err(|_| PortError::Com("STA job dropped before replying".to_string()))
    }
}

impl Drop for StaExecutor {
    fn drop(&mut self) {
        // Close the channel so the worker's `recv` loop ends, then join it.
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
