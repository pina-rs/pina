#![cfg(test)]

use pina_test::ProgramTest;
use pina_test::Pubkey;
use program_under_test::DeclareIdInstruction;
use program_under_test::ID;

/// Runs the data-only instruction against the real SBF artifact.
#[test]
#[ignore = "run with pina test"]
fn initialize_runs_on_surfpool() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		program
			.send(&[DeclareIdInstruction::Initialize as u8], Vec::new())
			.expect("execute Initialize");
		program.stop().expect("stop isolated program test");
	});
}
