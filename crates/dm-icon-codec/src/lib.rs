//! Icon codec (ADR-0019): ICO container assembly, the size ladder, and content hashing —
//! the bytes the M34 transaction driver stores as an opaque, content-addressed `AssetRef`.
//!
//! Ported from the frozen C# oracle (`IcoWriter` / `IconResampler` / `GeneratedIconStore`
//! / `OverlayBadgeIconFactory`). The container ([`ico`]) is byte-identical to C#; the
//! resampler ladder ([`ladder`]) is **reused** from `dm_icon_core::sampling` (the
//! M5-certified single truth source), so a baked ICO matches the on-screen preview
//! pixel-for-pixel. The shared pixel primitive is [`Raster`], re-exported from the core.
//!
//! Consumers: ledger `AssetRef` content-addressing ([`write_ico`] + [`content_hash`]);
//! the ADR-0021 transparent global overlay ([`transparent_ico`]); and the Recycle-Bin
//! `<asset>-empty.ico` companion (a second [`bake_ico`] of the empty-state source).

#![forbid(unsafe_code)]

pub mod hash;
pub mod ico;
pub mod ladder;

pub use dm_icon_core::raster::Raster;

pub use hash::{content_hash, write_ico_asset, IcoAsset};
pub use ico::{parse, write_ico, IcoEntry};
pub use ladder::{bake_ico, resample_ladder, transparent_ico, LADDER_SIZES, OVERLAY_SIZES};
