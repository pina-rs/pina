#![cfg(test)]

use pina_test::ProgramTest;
use pina_test::Pubkey;
use program_under_test::ErrorsInstruction;
use program_under_test::ID;

/// Every instruction in this example returns a custom error with an exact
/// code. The runtime must surface the numeric code so clients can match it.
#[test]
#[ignore = "run with pina test"]
fn every_instruction_returns_the_documented_custom_error() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let cases = [
			(ErrorsInstruction::Hello, 6_000u32),
			(ErrorsInstruction::HelloNoMsg, 6_123),
			(ErrorsInstruction::HelloNext, 6_124),
			(ErrorsInstruction::RequireEq, 6_126),
			(ErrorsInstruction::RequireNeq, 6_127),
			(ErrorsInstruction::RequireGt, 6_129),
			(ErrorsInstruction::RequireGte, 6_128),
		];

		for (instruction, code) in cases {
			let error = program
				.send(&[instruction as u8], Vec::new())
				.expect_err("the instruction always fails with a custom error");

			assert_eq!(error.operation(), "execute program instruction");
			assert!(
				error.message().contains(&format!("0x{:x}", code)),
				"instruction {code} must surface its custom error: {}",
				error.message()
			);
		}

		program.stop().expect("stop isolated program test");
	});
}

/// Unknown and empty payloads fail as instruction-data errors, not transport
/// failures (`operation` stays `execute program instruction`).
#[test]
#[ignore = "run with pina test"]
fn rejects_unparseable_instruction_data() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let error = program
			.send(&[9], Vec::new())
			.expect_err("an unknown discriminator cannot dispatch");
		assert_eq!(error.operation(), "execute program instruction");

		let error = program
			.send(&[], Vec::new())
			.expect_err("empty data cannot dispatch");
		assert_eq!(error.operation(), "execute program instruction");

		program.stop().expect("stop isolated program test");
	});
}
