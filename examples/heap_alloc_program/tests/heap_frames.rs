//! Mollusk tests for the opt-in heap entrypoint's frame accounting.
//!
//! Mollusk exposes the runtime's compute budget directly
//! (`Mollusk::compute_budget.heap_size`), so it can hold the heap frame fixed
//! at a known size and show exactly what the allocator does at and past that
//! boundary. Surfpool cannot: it takes the frame from the transaction's
//! compute-budget instructions, which is a caller concern the on-chain suite
//! covers separately.
//!
//! These tests run against the compiled SBF artifact, so the allocator under
//! test is the on-chain `BumpAllocator` rather than the host's std allocator.
//!
//! ## Prerequisites
//!
//! The program must be compiled to an SBF binary first:
//!
//! ```sh
//! devenv shell -- cargo-build-sbf --manifest-path examples/heap_alloc_program/Cargo.toml \
//!     --features bpf-entrypoint --sbf-out-dir target/deploy
//! ```
//!
//! Then set `SBF_OUT_DIR`, or place the `.so` in `tests/fixtures/`.

use heap_alloc_program::FillInstruction;
use heap_alloc_program::HeapInstruction;
use heap_alloc_program::ID;
use mollusk_svm::Mollusk;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

/// Bytes in the runtime's default heap frame, asserted against mollusk below.
const DEFAULT_HEAP_FRAME: u32 = 32 * 1024;

fn program_id() -> Pubkey {
	Pubkey::new_from_array(ID.to_bytes())
}

fn fill_instruction(bytes: u32, fill: u8) -> Instruction {
	let mut data = Vec::with_capacity(FillInstruction::SIZE);
	data.push(HeapInstruction::Fill as u8);
	data.push(0);
	data.extend_from_slice(&bytes.to_le_bytes());
	data.push(fill);

	assert_eq!(data.len(), FillInstruction::SIZE);

	Instruction {
		program_id: program_id(),
		accounts: vec![],
		data,
	}
}

/// Build a mollusk instance with the heap frame pinned to `heap_size`.
///
/// Returns `None` when the SBF binary is missing, so a plain
/// `cargo test --workspace` skips these instead of failing. The artifact is
/// built by the `program-e2e` job, which also runs this suite with
/// `SBF_OUT_DIR` set; `Mollusk::new` panics on a missing file, so the check has
/// to happen before it is called.
fn try_create_mollusk() -> Option<Mollusk> {
	let so_name = "heap_alloc_program.so";
	let search_dirs: Vec<std::path::PathBuf> = [
		std::env::var("SBF_OUT_DIR").ok(),
		std::env::var("BPF_OUT_DIR").ok(),
		Some("tests/fixtures".to_owned()),
	]
	.into_iter()
	.flatten()
	.map(std::path::PathBuf::from)
	.collect();

	if !search_dirs.iter().any(|dir| dir.join(so_name).is_file()) {
		return None;
	}

	Some(Mollusk::new(&program_id(), "heap_alloc_program"))
}

/// Build a mollusk instance with the heap frame pinned to `heap_size`.
///
/// Pinning is what makes the boundary tests deterministic: they compare an
/// allocation against a frame the test chose, not against whatever the runtime
/// grants by default.
fn try_create_pinned_mollusk(heap_size: u32) -> Option<Mollusk> {
	let mut mollusk = try_create_mollusk()?;
	mollusk.compute_budget.heap_size = heap_size;

	Some(mollusk)
}

/// The runtime's default frame is exactly the documented 32 KiB.
///
/// This pins the number the entrypoint macro's docs quote, so a runtime change
/// that moves the default fails here instead of silently invalidating the
/// guidance.
#[test]
fn the_default_heap_frame_is_32_kib() {
	// Deliberately unpinned: assigning `heap_size` first would make this
	// assertion pass no matter what the runtime's default actually is.
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("skipping: build heap_alloc_program for SBF first");
		return;
	};

	assert_eq!(
		mollusk.compute_budget.heap_size, DEFAULT_HEAP_FRAME,
		"the documented default heap frame must match the runtime"
	);
}

/// Runs one fill and reports whether the program completed.
fn fill_succeeds(mollusk: &Mollusk, bytes: u32, fill: u8) -> bool {
	let result = mollusk.process_instruction(&fill_instruction(bytes, fill), &[]);

	result.program_result.is_ok()
}

/// A fill that fits inside the default frame succeeds without any request.
#[test]
fn a_fill_inside_the_default_frame_succeeds() {
	let Some(mollusk) = try_create_pinned_mollusk(DEFAULT_HEAP_FRAME) else {
		eprintln!("skipping: build heap_alloc_program for SBF first");
		return;
	};

	assert!(
		fill_succeeds(&mollusk, DEFAULT_HEAP_FRAME / 2, 3),
		"half a frame fits in the default heap"
	);
}

/// An allocation that exactly fills the frame leaves no room for the
/// allocator's own bookkeeping, so it fails.
///
/// `BumpAllocator` stores its current position in the first word of the heap
/// region, so the bytes actually available to a program are the frame minus
/// that word. A program that sizes its request from the raw frame and then
/// asks for the whole thing aborts — the failure mode the docs warn about.
#[test]
fn a_fill_at_the_raw_frame_size_fails() {
	let Some(mollusk) = try_create_pinned_mollusk(DEFAULT_HEAP_FRAME) else {
		eprintln!("skipping: build heap_alloc_program for SBF first");
		return;
	};

	assert!(
		!fill_succeeds(&mollusk, DEFAULT_HEAP_FRAME, 1),
		"the allocator's bookkeeping word is not part of the usable heap"
	);
}

/// Sizing a request to leave the bookkeeping word free is what actually works.
#[test]
fn a_fill_inside_the_frame_after_bookkeeping_succeeds() {
	let Some(mollusk) = try_create_pinned_mollusk(DEFAULT_HEAP_FRAME) else {
		eprintln!("skipping: build heap_alloc_program for SBF first");
		return;
	};

	// One word (8 bytes on 64-bit) is reserved for the heap pointer.
	let usable = DEFAULT_HEAP_FRAME - 8;
	assert!(
		fill_succeeds(&mollusk, usable, 1),
		"a fill that leaves the bookkeeping word free completes"
	);
}

/// A fill past the frame aborts rather than returning `ProgramError`.
///
/// This is the documented failure mode: the allocator cannot satisfy the
/// request, and there is no recoverable error path out of `alloc`. The
/// distinction matters because the program cannot catch it — a caller sees an
/// aborted instruction, not a program error code it could handle.
#[test]
fn a_fill_past_the_frame_aborts() {
	let Some(mollusk) = try_create_pinned_mollusk(DEFAULT_HEAP_FRAME) else {
		eprintln!("skipping: build heap_alloc_program for SBF first");
		return;
	};

	let result = mollusk.process_instruction(&fill_instruction(2 * DEFAULT_HEAP_FRAME, 1), &[]);

	// The program returns `InvalidInstructionData` only when its own checksum
	// check fails, and that cannot happen here: the buffer is allocated before
	// any byte is summed. So a failure that is not a `ProgramError` can only be
	// the allocator aborting, which is the documented way out of this
	// instruction when the heap cannot satisfy the request.
	assert!(
		matches!(
			result.program_result,
			mollusk_svm::result::ProgramResult::UnknownError(_)
		),
		"an allocation past the frame aborts in the allocator, got {:?}",
		result.program_result
	);
}

/// Raising the frame lets the same instruction succeed.
///
/// Together with the test above, this shows the frame — not the program — is
/// what decides the outcome, which is why the request belongs to the caller.
#[test]
fn raising_the_frame_lets_the_same_fill_succeed() {
	let Some(mollusk) = try_create_pinned_mollusk(4 * DEFAULT_HEAP_FRAME) else {
		eprintln!("skipping: build heap_alloc_program for SBF first");
		return;
	};

	assert!(
		fill_succeeds(&mollusk, 2 * DEFAULT_HEAP_FRAME, 1),
		"the same fill completes once the frame covers it"
	);
}

/// The largest frame the runtime will grant is 256 KiB, and a fill sized to
/// leave the bookkeeping word free succeeds within it.
#[test]
fn the_maximum_frame_covers_a_fill_the_size_of_the_heap() {
	let maximum_heap_frame: u32 = 256 * 1024;
	let Some(mollusk) = try_create_pinned_mollusk(maximum_heap_frame) else {
		eprintln!("skipping: build heap_alloc_program for SBF first");
		return;
	};

	assert!(
		fill_succeeds(&mollusk, maximum_heap_frame - 8, 0xff),
		"a fill of the largest grantable frame completes"
	);
}
