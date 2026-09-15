#![cfg(test)]

use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::default_v1_config;
use program_under_test::EventsInstruction;
use program_under_test::ID;

/// Every event instruction also executes when a v1 transaction carries it.
///
/// A program receives resolved accounts and instruction data, so the wire
/// format that delivered them is invisible to it. This asserts that rather
/// than assuming it: the same instruction that confirms as a legacy
/// transaction must confirm as a v1 transaction.
#[test]
#[ignore = "run with pina test"]
fn event_instructions_execute_in_a_v1_transaction() {
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
			let logs = program
				.simulate_v1_logs(&[instruction as u8], Vec::new(), default_v1_config())
				.expect("simulate v1 event instruction");
			assert!(
				logs.iter().any(|line| line.ends_with("success")),
				"v1 execution must reach program success; logs: {logs:#?}"
			);

			program
				.send_v1(&[instruction as u8], Vec::new(), default_v1_config())
				.expect("v1 event instruction confirms");
		}

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
