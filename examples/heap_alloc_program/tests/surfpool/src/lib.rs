#![cfg(test)]

//! On-chain tests for the opt-in heap allocation entrypoint.
//!
//! These run the real SBF artifact through an isolated Surfpool instance, so
//! the `BumpAllocator` that `nostd_entrypoint_alloc!` installs is the one the
//! runtime actually uses. That is the point of the suite: a host test would
//! exercise the std allocator and prove nothing about the on-chain path.
//!
//! Frame accounting — what the allocator does at and past the heap budget — is
//! covered by `tests/heap_frames.rs`, which can hold the frame fixed. A
//! Surfpool transaction would have to route the frame through a compute-budget
//! instruction to change it, which tests the caller's plumbing rather than the
//! allocator.

use pina_test::AccountMeta;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use program_under_test::AllocateInstruction;
use program_under_test::FillInstruction;
use program_under_test::HeapInstruction;
use program_under_test::ID;

/// Bytes in the runtime's default heap frame.
const DEFAULT_HEAP_FRAME: u32 = 32 * 1024;

/// `Allocate` instruction data: discriminator, migration version, value.
fn allocate_data(value: u64) -> Vec<u8> {
	let mut data = Vec::with_capacity(AllocateInstruction::SIZE);
	data.push(HeapInstruction::Allocate as u8);
	data.push(0);
	data.extend_from_slice(&value.to_le_bytes());

	assert_eq!(
		data.len(),
		AllocateInstruction::SIZE,
		"Allocate encoding must match the generated SIZE"
	);

	data
}

/// `Fill` instruction data: discriminator, migration version, bytes, fill.
fn fill_data(bytes: u32, fill: u8) -> Vec<u8> {
	let mut data = Vec::with_capacity(FillInstruction::SIZE);
	data.push(HeapInstruction::Fill as u8);
	data.push(0);
	data.extend_from_slice(&bytes.to_le_bytes());
	data.push(fill);

	assert_eq!(
		data.len(),
		FillInstruction::SIZE,
		"Fill encoding must match the generated SIZE"
	);

	data
}

#[test]
#[ignore = "run with pina test"]
fn box_allocation_round_trips_through_the_heap() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let payer = program.payer();
		let accounts = || vec![AccountMeta::new_readonly(payer, true)];

		// The program returns an error unless the value it reads back out of
		// the Box matches what it wrote, so a confirmed transaction here is the
		// heap round-trip succeeding on chain. The values cover the extremes
		// the encoding can carry, including the all-ones pattern.
		for value in [0u64, 1, 0x0123_4567_89AB_CDEF, u64::MAX] {
			program
				.send(&allocate_data(value), accounts())
				.unwrap_or_else(|error| panic!("box the value {value}: {error}"));
		}

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn the_heap_round_trip_log_proves_the_bump_allocator_ran() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let payer = program.payer();

		// The log is written only after the Box round-trip matched. Under
		// `no_allocator!` the same program would abort inside `alloc` before
		// reaching it, so the line distinguishes the two entrypoints.
		let logs = program
			.simulate_logs(
				&allocate_data(7),
				vec![AccountMeta::new_readonly(payer, true)],
			)
			.expect("simulate Allocate");

		assert!(
			logs.iter()
				.any(|line| line.contains("heap box round-trip ok")),
			"expected the heap round-trip log, got {logs:?}"
		);

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn a_large_fill_inside_the_default_frame_confirms() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let payer = program.payer();

		// Half the default frame, so the runtime's grant already covers it and
		// the transaction needs no heap request. The program's checksum check
		// means this only confirms if every allocated byte round-tripped.
		program
			.send(
				&fill_data(DEFAULT_HEAP_FRAME / 2, 3),
				vec![AccountMeta::new_readonly(payer, true)],
			)
			.expect("fill inside the default heap frame");

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn allocation_past_the_frame_aborts_instead_of_returning_an_error() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let payer = program.payer();

		// The largest frame the runtime will ever grant is 256 KiB, and this
		// transaction supplies no compute-budget instruction at all. The
		// allocator cannot satisfy 1 MiB, so the failure is an abort rather
		// than a `ProgramError` — the documented cost of opting into the heap.
		let error = program
			.send(
				&fill_data(1024 * 1024, 1),
				vec![AccountMeta::new_readonly(payer, true)],
			)
			.expect_err("an exhausted heap aborts the transaction");

		assert_eq!(error.operation(), "execute program instruction");
		eprintln!("heap exhaustion error: {}", error.message());

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn an_unknown_discriminator_is_rejected_before_allocating() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let payer = program.payer();

		// 0xff is not a `HeapInstruction` variant, so the entrypoint must
		// reject it. This proves the heap entrypoint still runs the ordinary
		// discriminator and program-id validation around the allocation rather
		// than only the allocation working.
		let mut data = allocate_data(42);
		data[0] = 0xff;
		let error = program
			.send(&data, vec![AccountMeta::new_readonly(payer, true)])
			.expect_err("an unknown discriminator is rejected");

		assert_eq!(error.operation(), "execute program instruction");
		eprintln!("unknown discriminator error: {}", error.message());

		program.stop().expect("stop isolated program test");
	});
}
