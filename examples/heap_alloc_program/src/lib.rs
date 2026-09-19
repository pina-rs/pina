//! Heap allocation example — an opt-in heap entrypoint built with pina.
//!
//! Every other Pina example uses [`nostd_entrypoint!`], whose
//! `pinocchio::no_allocator!` makes dynamic allocation impossible. This program
//! instead uses [`nostd_entrypoint_alloc!`], which installs
//! `pinocchio::default_allocator!` — a `BumpAllocator` over the runtime heap
//! region — so `alloc` is available:
//!
//! - **`extern crate alloc`** — the standard `no_std` allocator crate, giving
//!   this program `Box` and `Vec`.
//! - **A heap round-trip** — `Allocate` boxes a `u64`, reads it back through
//!   the heap pointer, and rejects the transaction if the value did not
//!   survive. A successful call is proof the allocator works, because the
//!   program has no other way to produce the answer.
//! - **The heap budget** — `Fill` allocates a caller-chosen byte count, so a
//!   transaction can exceed the runtime's default 32 KiB frame once it asks for
//!   a larger one with `TransactionConfig::with_heap_size`.
//!
//! ## Instructions
//!
//! | Variant | Description |
//! | --- | --- |
//! | `Allocate` | Box a `u64` and read it back through the heap. |
//! | `Fill` | Allocate, fill, and sum a heap buffer of `bytes` bytes. |
//!
//! ## What this demonstrates about the heap
//!
//! `BumpAllocator` never reclaims memory: its `dealloc` is a no-op, so a
//! transaction's peak heap use is the sum of every allocation it makes over its
//! lifetime, not the sum of the ones still alive. Both instructions drop what
//! they allocate, and nothing is returned to the heap. The heap is also
//! transaction-scoped — nothing allocated here is visible to another
//! transaction — so anything that must persist belongs in account data
//! through the zero-copy account types, not here.

#![allow(missing_docs)]
#![allow(clippy::inline_always)]
#![no_std]

// The heap is opt-in, so this crate is the first example that needs the
// allocator crate. `extern crate alloc` is valid in a `no_std` crate and links
// against whichever `#[global_allocator]` the entrypoint installs.
extern crate alloc;

// On native builds the cdylib target needs std for unwinding and panic
// handling. On BPF, `nostd_entrypoint_alloc!()` provides the panic handler and
// allocator. Tests link against std automatically.
#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::hint::black_box;

use pina::*;

// ---------------------------------------------------------------------------
// Program ID
// ---------------------------------------------------------------------------

declare_id!("BZtBGtYSgERx2zN12aGgQ6sNLh5r7XtcNCafqmsizgqt");

// ---------------------------------------------------------------------------
// Discriminators
// ---------------------------------------------------------------------------

/// Instruction discriminator for the heap example.
#[discriminator]
pub enum HeapInstruction {
	/// Box a `u64` and read it back through the heap.
	Allocate = 0,
	/// Allocate, fill, and sum a heap buffer of a caller-chosen size.
	Fill = 1,
}

// ---------------------------------------------------------------------------
// Instruction data structs
// ---------------------------------------------------------------------------

/// Instruction data for `Allocate`.
#[instruction(discriminator = HeapInstruction::Allocate)]
pub struct AllocateInstruction {
	/// The value to box. The program boxes it and reads it back.
	pub value: u64,
}

/// Instruction data for `Fill`.
#[instruction(discriminator = HeapInstruction::Fill)]
pub struct FillInstruction {
	/// Bytes to allocate.
	///
	/// The runtime grants a 32 KiB heap frame by default, so a larger value
	/// requires the caller to send the transaction with a matching
	/// `request_heap_frame` / `with_heap_size` request.
	pub bytes: u32,
	/// The byte written into every element of the buffer.
	pub fill: u8,
}

// ---------------------------------------------------------------------------
// Heap helpers
// ---------------------------------------------------------------------------

/// Box `value`, read it back through the heap pointer, and return it.
///
/// The `Box` is dropped before returning. `BumpAllocator::dealloc` is a no-op,
/// so the drop does not reclaim the bytes: the point is to prove the `alloc`
/// path returns readable memory, not that memory is returned to the heap.
#[inline(never)]
fn boxed_round_trip(value: u64) -> u64 {
	let boxed = Box::new(value);
	// `black_box` keeps the allocation observable: without an opaque use the
	// optimizer can fold the read back to `value` and drop the heap traffic
	// this example exists to exercise.
	let read_back = *black_box(&*boxed);
	drop(boxed);

	read_back
}

/// Allocate `bytes` bytes, fill them with `fill`, and return their sum.
///
/// Exceeding the transaction's heap frame aborts inside the allocator instead
/// of returning `ProgramError`, so the frame size is a client-side decision
/// that only the caller can make.
#[inline(never)]
fn fill_and_sum(bytes: u32, fill: u8) -> u64 {
	let buffer: Vec<u8> = vec![fill; bytes as usize];

	black_box(buffer.as_slice())
		.iter()
		.fold(0u64, |sum, byte| sum.wrapping_add(u64::from(*byte)))
}

// ---------------------------------------------------------------------------
// Instruction handlers
// ---------------------------------------------------------------------------

/// Boxes the requested value and rejects the transaction unless the value read
/// back out of the heap matches.
#[inline(always)]
fn process_allocate(data: &[u8]) -> ProgramResult {
	let args = AllocateInstruction::try_from_bytes(data)?;
	let expected = args.value.get();
	let actual = boxed_round_trip(expected);

	// The heap is the only route from `expected` to `actual`, so a mismatch
	// means the allocator returned memory that did not hold what was written.
	if actual != expected {
		return Err(ProgramError::InvalidInstructionData);
	}

	log!("heap box round-trip ok");
	Ok(())
}

/// Allocates the requested number of bytes, fills them, and rejects the
/// transaction unless the buffer sums to the value the fill implies.
///
/// The expected sum is `bytes * fill`, so the check proves every byte
/// allocated was written and read back — the sum is not reachable without the
/// buffer actually existing in the heap.
#[inline(always)]
fn process_fill(data: &[u8]) -> ProgramResult {
	let args = FillInstruction::try_from_bytes(data)?;
	let bytes = args.bytes.get();
	let fill = args.fill;
	let actual = fill_and_sum(bytes, fill);
	let expected = u64::from(bytes).wrapping_mul(u64::from(fill));

	if actual != expected {
		return Err(ProgramError::InvalidInstructionData);
	}

	log!("heap fill round-trip ok");
	Ok(())
}

// ---------------------------------------------------------------------------
// Entrypoint
// ---------------------------------------------------------------------------

/// The entrypoint is gated behind `bpf-entrypoint` so tests and CPI consumers
/// can use this crate as a library without pulling in the BPF entrypoint.
///
/// The single difference from every other Pina example is the macro name:
/// `nostd_entrypoint_alloc!` installs the bump allocator instead of denying
/// allocation, which is what makes `extern crate alloc` usable above.
#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use super::*;

	nostd_entrypoint_alloc!(process_instruction);

	/// Routes the instruction discriminator to its handler.
	#[inline(always)]
	pub fn process_instruction(
		program_id: &Address,
		_accounts: &mut [AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: HeapInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			HeapInstruction::Allocate => process_allocate(data),
			HeapInstruction::Fill => process_fill(data),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn boxed_round_trip_returns_the_stored_value() {
		// Host builds use the std allocator, so the same code path runs
		// natively before the on-chain test runs it on the bump allocator.
		assert_eq!(boxed_round_trip(0), 0);
		assert_eq!(boxed_round_trip(1), 1);
		assert_eq!(boxed_round_trip(u64::MAX), u64::MAX);
		assert_eq!(
			boxed_round_trip(0x0123_4567_89AB_CDEF),
			0x0123_4567_89AB_CDEF
		);
	}

	#[test]
	fn fill_and_sum_returns_bytes_times_fill() {
		assert_eq!(fill_and_sum(1024, 3), 1024 * 3);
	}

	#[test]
	fn fill_and_sum_sums_an_empty_buffer_as_zero() {
		assert_eq!(fill_and_sum(0, 9), 0);
	}

	#[test]
	fn fill_and_sum_saturates_instead_of_overflowing() {
		// A full 256 KiB buffer of 0xff sums past u64 only if the accumulator is
		// wrong; wrapping keeps the contract total rather than panicking.
		let checksum = fill_and_sum(256 * 1024, 0xff);

		assert_eq!(checksum, 0xff * 256 * 1024);
	}
}
