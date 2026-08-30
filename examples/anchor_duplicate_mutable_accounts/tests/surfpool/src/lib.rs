#![cfg(test)]

use pina_test::ProgramTest;
use pina_test::Pubkey;
use program_under_test::DuplicateMutableInstruction;
use program_under_test::ID;

/// Proves that the real SBF artifact can be deployed at its declared address.
#[test]
#[ignore = "run with pina test"]
fn data_only_duplicate_mutable_variant_succeeds() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		program
			.send(
				&[DuplicateMutableInstruction::AllowsDuplicateMutable as u8],
				Vec::new(),
			)
			.expect("execute duplicate mutable compatibility instruction");
		program.stop().expect("stop isolated program test");
	});
}
