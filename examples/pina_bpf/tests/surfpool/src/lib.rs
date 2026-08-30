#![cfg(test)]

use pina_test::ProgramTest;
use pina_test::Pubkey;
use program_under_test::ID;
use program_under_test::PinaBpfInstruction;

/// Proves that the real SBF artifact can be deployed at its declared address.
#[test]
#[ignore = "run with pina test"]
fn hello_runs_on_surfpool() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		program
			.send(&[PinaBpfInstruction::Hello as u8], Vec::new())
			.expect("execute Hello");
		program.stop().expect("stop isolated program test");
	});
}
