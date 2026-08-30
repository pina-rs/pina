#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use program_under_test::HelloInstruction;
use program_under_test::ID;

/// Proves that the real SBF artifact can be deployed at its declared address.
#[test]
#[ignore = "run with pina test"]
fn hello_accepts_the_funded_payer_signature() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		program
			.send(
				&[HelloInstruction::Hello as u8],
				vec![AccountMeta::new_readonly(program.payer(), true)],
			)
			.expect("execute Hello");
		program.stop().expect("stop isolated program test");
	});
}
