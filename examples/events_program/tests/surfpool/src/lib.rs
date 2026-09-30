#![cfg(test)]

use base64::Engine as _;
// The program is a cdylib only (see ../../Cargo.toml), so its real types
// come in through a source include rather than an rlib dependency.
#[path = "../../../src/lib.rs"]
mod program;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use program::EventDiscriminator;
use program::EventsInstruction;
use program::ID;

/// The transport prefix the generated Rust, TypeScript, and Dart event decoders
/// all parse.
const PROGRAM_DATA_PREFIX: &str = "Program data: ";

/// Decode every `Program data:` payload from one program log.
///
/// This mirrors the generated client read path: strip the stable-log prefix,
/// then base64-decode the remainder into one event record.
fn decoded_event_records(logs: &[String]) -> Vec<Vec<u8>> {
	logs.iter()
		.filter_map(|log| log.strip_prefix(PROGRAM_DATA_PREFIX))
		.map(|payload| {
			base64::engine::general_purpose::STANDARD
				.decode(payload.trim())
				.unwrap_or_else(|error| {
					panic!("decode `Program data:` payload {payload:?}: {error}")
				})
		})
		.collect()
}

/// Every event instruction reaches the transaction log as a `Program data:`
/// record that names its own event discriminator.
///
/// This is the contract the generated Rust, TypeScript, and Dart decoders
/// depend on. Constructing event bytes on-chain is not enough: a consumer that
/// only watches logs must be able to find them.
#[test]
#[ignore = "run with pina test"]
fn event_instructions_emit_decodable_log_records() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let expected = [
			(EventsInstruction::Initialize, EventDiscriminator::MyEvent),
			(
				EventsInstruction::TestEvent,
				EventDiscriminator::MyOtherEvent,
			),
			(
				EventsInstruction::TestEventCpi,
				EventDiscriminator::MyOtherEvent,
			),
		];

		for (index, (instruction, discriminator)) in expected.into_iter().enumerate() {
			let logs = program
				.simulate_logs(&[instruction as u8], Vec::new())
				.expect("simulate event instruction");
			let records = decoded_event_records(&logs);
			assert_eq!(
				records.len(),
				1,
				"expected one `Program data:` record for event case {index}; logs: {logs:#?}"
			);
			assert_eq!(
				records[0].first().copied(),
				Some(discriminator as u8),
				"the emitted record must start with the event discriminator; logs: {logs:#?}"
			);
		}

		program.stop().expect("stop isolated program test");
	});
}

/// The emitted bytes decode through the generated client the way an indexer
/// would: the record carries the discriminator and the same field values the
/// program configured, so a consumer reconstructs the event without any
/// program-side knowledge.
#[test]
#[ignore = "run with pina test"]
fn generated_client_decodes_the_emitted_record() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let logs = program
			.simulate_logs(&[EventsInstruction::Initialize as u8], Vec::new())
			.expect("simulate event instruction");
		let records = decoded_event_records(&logs);
		let record = records
			.first()
			.unwrap_or_else(|| panic!("expected a `Program data:` record; logs: {logs:#?}"));

		let event = generated_client::events::MyEvent::from_bytes(record)
			.expect("the generated client decodes the emitted record");
		assert_eq!(event.data.get(), 5);
		assert_eq!(event.label, [b'h', b'e', b'l', b'l', b'o', 0, 0, 0]);

		program.stop().expect("stop isolated program test");
	});
}

/// Each event instruction succeeds against the real artifact and is
/// repeatable (events are stateless log emissions).
#[test]
#[ignore = "run with pina test"]
fn event_emitting_instructions_confirm() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		for instruction in [
			EventsInstruction::Initialize,
			EventsInstruction::TestEvent,
			EventsInstruction::TestEventCpi,
		] {
			program
				.send(&[instruction as u8], Vec::new())
				.expect("event instruction confirms");
		}

		program.stop().expect("stop isolated program test");
	});
}

/// Unknown discriminators are rejected by the program's dispatcher, not the
/// transport.
#[test]
#[ignore = "run with pina test"]
fn rejects_unparseable_instruction_data() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let error = program
			.send(&[8], Vec::new())
			.expect_err("unknown discriminator cannot dispatch");
		assert_eq!(error.operation(), "execute program instruction");

		let error = program
			.send(&[], Vec::new())
			.expect_err("empty data cannot dispatch");
		assert_eq!(error.operation(), "execute program instruction");

		program.stop().expect("stop isolated program test");
	});
}

/// Instructions carry no version envelope unless they opt into migrations, so
/// a zero-field event instruction is exactly its discriminator on the wire.
/// The event it emits is still enveloped: the logged record keeps its version
/// byte between the event discriminator and the payload.
#[test]
#[ignore = "run with pina test"]
fn bare_discriminator_dispatches_and_emits_a_versioned_record() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let logs = program
			.simulate_logs(&[EventsInstruction::Initialize as u8], Vec::new())
			.expect("a bare discriminator simulates");
		let records = decoded_event_records(&logs);
		assert_eq!(
			records.len(),
			1,
			"the bare discriminator must dispatch; logs: {logs:#?}"
		);

		// Event envelope: discriminator, migration version, then the u64 data
		// and the 8-byte label.
		let record = &records[0];
		assert_eq!(record.len(), 18, "MyEvent record is 18 bytes");
		assert_eq!(record[0], EventDiscriminator::MyEvent as u8);
		assert_eq!(record[1], 0, "the event record keeps its current version");

		program.stop().expect("stop isolated program test");
	});
}
