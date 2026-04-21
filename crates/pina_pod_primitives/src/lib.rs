#![no_std]

//! Alignment-safe primitive wrappers and fixed-capacity collection types
//! for use in `Pod` structs.
//!
//! Pod integer types (`PodU64`, `PodU32`, etc.) wrap native integers in
//! `[u8; N]` arrays, guaranteeing alignment 1. This allows direct pointer casts
//! from account data without alignment concerns — critical for `#[repr(C)]`
//! zero-copy structs on Solana.
//!
//! # Arithmetic
//!
//! Arithmetic operators (`+`, `-`, `*`) on Pod **integer** types use **wrapping**
//! semantics in release builds for CU efficiency and **panic on overflow** in
//! debug builds. Use `checked_add`, `checked_sub`, `checked_mul`,
//! `checked_div` where overflow must be detected in all build profiles.
//!
//! # Constants
//!
//! Each Pod integer type provides `ZERO`, `MIN`, and `MAX` constants.
//!
//! # Collection types
//!
//! `PodOption<T>`, `PodString<N, PFX>`, and `PodVec<T, N, PFX>` are
//! fixed-capacity, alignment-1 types that store data inline with a length
//! prefix. They implement `bytemuck::Pod` + `bytemuck::Zeroable` and can be
//! embedded directly in `#[repr(C)]` account structs. Overflow is detected at
//! insertion time via `try_set` / `try_push`, which return
//! `Err(PodCollectionError::Overflow)` when capacity is exceeded.

// Allow unsafe code for the collection types that need MaybeUninit.
// Safety is guaranteed by:
// - All types are #[repr(C)] with alignment 1
// - MaybeUninit allows any bit pattern (satisfying Pod requirements)
// - Length prefixes prevent reading uninitialized data as initialized
#![allow(unsafe_code)]

mod error;
mod macros;
mod option;
mod pod_bool;
mod pod_numeric;
mod string;
mod vec;

#[cfg(test)]
mod tests;

pub use error::PodCollectionError;
pub use option::PodOption;
pub use pod_bool::PodBool;
// Numeric types are defined via macros in the `numeric` module and re-exported
// here for the public API. The macros themselves are `#[macro_export]` so they
// are available at the crate root.
pub use pod_numeric::{PodI16, PodI32, PodI64, PodI128, PodU16, PodU32, PodU64, PodU128};
pub use string::PodString;
pub use vec::PodVec;
