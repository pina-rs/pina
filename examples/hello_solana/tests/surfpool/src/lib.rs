#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::Keypair;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Signer;
use program_under_test::HelloInstruction;
use program_under_test::ID;

/// The program requires the `user` account to have signed the transaction.
#[test]
#[ignore = "run with pina test"]
fn hello_accepts_the_funded_payer_signature() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");
		let user = program.payer();

		program
			.send(
				&[HelloInstruction::Hello as u8],
				vec![AccountMeta::new_readonly(user, true)],
			)
			.expect("execute Hello");

		// A second invocation is stateless and therefore succeeds again.
		program
			.send(
				&[HelloInstruction::Hello as u8],
				vec![AccountMeta::new_readonly(user, true)],
			)
			.expect("execute Hello a second time");

		program.stop().expect("stop isolated program test");
	});
}

/// The `user` account must sign; a read-only proxy must not count.
#[test]
#[ignore = "run with pina test"]
fn hello_rejects_a_user_that_did_not_sign() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");
		// A bystander that is not the fee payer and did not sign: the runtime
		// must not grant it signer privileges.
		let user = Pubkey::new_unique();
		program.fund(&user, 1_000_000_000).expect("fund user");

		let error = program
			.send(
				&[HelloInstruction::Hello as u8],
				vec![AccountMeta::new_readonly(user, false)],
			)
			.expect_err("Hello rejects an unsigned user");

		assert_eq!(error.operation(), "execute program instruction");
		program.stop().expect("stop isolated program test");
	});
}

/// A fully unrelated key, funded inside the isolated surfnet, may also act as
/// the signer — the program never ties `user` to the fee payer.
#[test]
#[ignore = "run with pina test"]
fn hello_accepts_any_funded_signer() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let user = Keypair::new();
		program
			.fund(&user.pubkey(), 1_000_000_000)
			.expect("fund the guest user");

		let instruction = program.instruction(
			&[HelloInstruction::Hello as u8],
			vec![AccountMeta::new_readonly(user.pubkey(), true)],
		);
		program
			.send_with_signers(instruction, &[&user])
			.expect("execute Hello");

		program.stop().expect("stop isolated program test");
	});
}
