#![cfg(test)]

use pina_test::ProgramTest;
use pina_test::Pubkey;
use program_under_test::EventsInstruction;
use program_under_test::ID;

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
