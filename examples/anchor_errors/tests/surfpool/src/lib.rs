#![cfg(test)]

use pina_test::ProgramTest;
use pina_test::Pubkey;
use program_under_test::ErrorsInstruction;
use program_under_test::ID;

/// Proves that the real SBF artifact can be deployed at its declared address.
#[test]
#[ignore = "run with pina test"]
fn custom_error_is_returned_by_the_runtime() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let error = program
			.send(&[ErrorsInstruction::Hello as u8], Vec::new())
			.expect_err("Hello returns the example's custom error");

		assert_eq!(error.operation(), "execute program instruction");
		assert!(error.message().contains("0x1770"));
		program.stop().expect("stop isolated program test");
	});
}
