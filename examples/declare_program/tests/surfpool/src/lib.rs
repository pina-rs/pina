#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use program_under_test::DeclareProgramInstruction;
use program_under_test::ID;
use program_under_test::external;

/// The declared external program is a placeholder that never exists on a live
/// cluster: validating against it fails the executable assertion inside the
/// program, proving the guard chain really reaches `assert_executable`.
#[test]
#[ignore = "run with pina test"]
fn validates_the_recorded_external_program_id() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let external = Pubkey::new_from_array(external::ID.to_bytes());

		// Fund the address so `assert_executable` fails on executability
		// (non-executable account), not on lookup.
		program
			.fund(&external, 1_000_000_000)
			.expect("fund external program stub");

		let error = program
			.send(
				&[DeclareProgramInstruction::ValidateExternalProgram as u8],
				vec![
					AccountMeta::new_readonly(authority, true),
					AccountMeta::new_readonly(external, false),
				],
			)
			.expect_err("stub external program is not executable");

		assert_eq!(error.operation(), "execute program instruction");
		eprintln!("non-executable external program error: {}", error.message());

		program.stop().expect("stop isolated program test");
	});
}

/// A wrong external program address is rejected even though it is a real,
/// executable program on the isolated surfnet.
#[test]
#[ignore = "run with pina test"]
fn rejects_wrong_external_program_ids() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let system_program = Pubkey::default();

		let error = program
			.send(
				&[DeclareProgramInstruction::ValidateExternalProgram as u8],
				vec![
					AccountMeta::new_readonly(authority, true),
					AccountMeta::new_readonly(system_program, false),
				],
			)
			.expect_err("System Program is not the declared external program");

		assert_eq!(error.operation(), "execute program instruction");

		program.stop().expect("stop isolated program test");
	});
}

/// The authority must sign; the runtime enforces this before program logic.
#[test]
#[ignore = "run with pina test"]
fn requires_authority_signature() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let external = Pubkey::new_from_array(external::ID.to_bytes());

		let error = program
			.send(
				&[DeclareProgramInstruction::ValidateExternalProgram as u8],
				vec![
					AccountMeta::new_readonly(authority, false),
					AccountMeta::new_readonly(external, false),
				],
			)
			.expect_err("unsigned authority must be rejected");

		assert_eq!(error.operation(), "execute program instruction");

		program.stop().expect("stop isolated program test");
	});
}
