#![cfg(test)]

use pina_test::ProgramTest;
use pina_test::Pubkey;
use program_under_test::ID;

/// Proves that the real SBF artifact can be deployed at its declared address.
#[test]
#[ignore = "run with pina test"]
fn deploys() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		assert!(program.is_executable().expect("fetch deployed program"));
		program.stop().expect("stop isolated program test");
	});
}
