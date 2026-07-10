//! Shell COM adapters: known-folder resolution, `.lnk` reading/writing, and the desktop scan.
//! Every function here must run on the STA thread ([`crate::com::StaExecutor`]).

pub mod attrs;
pub mod known_folders;
pub mod layout;
pub mod scan;
pub mod shell_link;

pub use scan::WindowsScanner;
