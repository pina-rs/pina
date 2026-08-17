//! Alignment-safe primitive wrappers for use in `#[repr(C)]` account structs.
//!
//! Solana account data is a flat byte buffer with alignment 1. Standard Rust
//! integers (`u16`, `u32`, `u64`, etc.) require alignment > 1 and therefore
//! cannot be placed directly in a zero-copy struct without padding. The
//! `Pod*` types in this module (re-exported from [`zeropod`]) wrap byte arrays
//! and convert via little-endian encoding, making them safe to embed in any
//! `#[repr(C)]` account layout.

mod primitives;

pub use primitives::*;
pub use zeropod::pod::*;
