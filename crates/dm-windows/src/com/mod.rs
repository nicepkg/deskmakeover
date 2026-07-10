//! COM plumbing: the single-threaded-apartment guard and the dedicated STA executor that all
//! shell COM work is marshalled onto (ADR-0019 Amendment 1).

mod apartment;
mod sta_actor;

pub use apartment::Apartment;
pub use sta_actor::StaExecutor;
