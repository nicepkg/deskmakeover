use std::fmt::Debug;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::JournalError;

pub trait WriterLease: Debug {}

#[derive(Debug, Default)]
pub(crate) struct MemoryLeaseState {
    next_lease_id: u64,
    active_lease_id: Option<u64>,
}

#[derive(Debug)]
pub struct MemoryWriterLease {
    owner_id: u64,
    lease_id: u64,
    state: Arc<Mutex<MemoryLeaseState>>,
}

impl WriterLease for MemoryWriterLease {}

impl Drop for MemoryWriterLease {
    fn drop(&mut self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if state.active_lease_id == Some(self.lease_id) {
            state.active_lease_id = None;
        }
    }
}

pub(crate) fn new_owner() -> (u64, Arc<Mutex<MemoryLeaseState>>) {
    static NEXT_OWNER_ID: AtomicU64 = AtomicU64::new(1);
    (
        NEXT_OWNER_ID.fetch_add(1, Ordering::Relaxed),
        Arc::new(Mutex::new(MemoryLeaseState::default())),
    )
}

pub(crate) fn acquire(
    owner_id: u64,
    state: &Arc<Mutex<MemoryLeaseState>>,
) -> Result<MemoryWriterLease, JournalError> {
    let mut locked = state
        .lock()
        .map_err(|_| JournalError("memory writer lease state poisoned".into()))?;
    if locked.active_lease_id.is_some() {
        return Err(JournalError(
            "writer lease already held for this journal".into(),
        ));
    }
    locked.next_lease_id += 1;
    let lease_id = locked.next_lease_id;
    locked.active_lease_id = Some(lease_id);
    drop(locked);
    Ok(MemoryWriterLease {
        owner_id,
        lease_id,
        state: Arc::clone(state),
    })
}

pub(crate) fn validate(
    owner_id: u64,
    state: &Arc<Mutex<MemoryLeaseState>>,
    lease: &MemoryWriterLease,
) -> Result<(), JournalError> {
    if lease.owner_id != owner_id || !Arc::ptr_eq(&lease.state, state) {
        return Err(JournalError(
            "writer lease belongs to a different journal".into(),
        ));
    }
    let locked = state
        .lock()
        .map_err(|_| JournalError("memory writer lease state poisoned".into()))?;
    if locked.active_lease_id != Some(lease.lease_id) {
        return Err(JournalError("writer lease is no longer active".into()));
    }
    Ok(())
}
