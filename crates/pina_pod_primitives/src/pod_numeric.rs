//! Pod integer type definitions (`PodU16`, `PodI16`, …, `PodU128`, `PodI128`).

use bytemuck::Pod;
use bytemuck::Zeroable;

use crate::define_pod_signed;
use crate::define_pod_unsigned;

define_pod_unsigned!(
	PodU16,
	u16,
	2,
	"An alignment-1 wrapper around `u16` stored as `[u8; 2]`.\n\nEnables safe zero-copy access \
	 inside `#[repr(C)]` account structs."
);

define_pod_signed!(
	PodI16,
	i16,
	2,
	"An alignment-1 wrapper around `i16` stored as `[u8; 2]`.\n\nEnables safe zero-copy access \
	 inside `#[repr(C)]` account structs."
);

define_pod_unsigned!(
	PodU32,
	u32,
	4,
	"An alignment-1 wrapper around `u32` stored as `[u8; 4]`.\n\nEnables safe zero-copy access \
	 inside `#[repr(C)]` account structs."
);

define_pod_signed!(
	PodI32,
	i32,
	4,
	"An alignment-1 wrapper around `i32` stored as `[u8; 4]`.\n\nEnables safe zero-copy access \
	 inside `#[repr(C)]` account structs."
);

define_pod_unsigned!(
	PodU64,
	u64,
	8,
	"An alignment-1 wrapper around `u64` stored as `[u8; 8]`.\n\nEnables safe zero-copy access \
	 inside `#[repr(C)]` account structs."
);

define_pod_signed!(
	PodI64,
	i64,
	8,
	"An alignment-1 wrapper around `i64` stored as `[u8; 8]`.\n\nEnables safe zero-copy access \
	 inside `#[repr(C)]` account structs."
);

define_pod_unsigned!(
	PodU128,
	u128,
	16,
	"An alignment-1 wrapper around `u128` stored as `[u8; 16]`.\n\nEnables safe zero-copy access \
	 inside `#[repr(C)]` account structs."
);

define_pod_signed!(
	PodI128,
	i128,
	16,
	"An alignment-1 wrapper around `i128` stored as `[u8; 16]`.\n\nEnables safe zero-copy access \
	 inside `#[repr(C)]` account structs."
);

// Compile-time invariant: all numeric Pod types must have alignment 1 and
// correct size.
const _: () = assert!(align_of::<PodU16>() == 1);
const _: () = assert!(size_of::<PodU16>() == 2);
const _: () = assert!(align_of::<PodI16>() == 1);
const _: () = assert!(size_of::<PodI16>() == 2);
const _: () = assert!(align_of::<PodU32>() == 1);
const _: () = assert!(size_of::<PodU32>() == 4);
const _: () = assert!(align_of::<PodI32>() == 1);
const _: () = assert!(size_of::<PodI32>() == 4);
const _: () = assert!(align_of::<PodU64>() == 1);
const _: () = assert!(size_of::<PodU64>() == 8);
const _: () = assert!(align_of::<PodI64>() == 1);
const _: () = assert!(size_of::<PodI64>() == 8);
const _: () = assert!(align_of::<PodU128>() == 1);
const _: () = assert!(size_of::<PodU128>() == 16);
const _: () = assert!(align_of::<PodI128>() == 1);
const _: () = assert!(size_of::<PodI128>() == 16);
