#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::Keypair;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Signer;
use program_under_test::DeclareIdInstruction;
use program_under_test::ID;

/// Runs the data-only instruction against the real SBF artifact, repeatedly,
/// to prove the program is a stateless no-op that always confirms.
#[test]
#[ignore = "run with pina test"]
fn initialize_runs_on_surfpool() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		for _ in 0..3 {
			program
				.send(&[DeclareIdInstruction::Initialize as u8], Vec::new())
				.expect("execute Initialize");
		}

		program.stop().expect("stop isolated program test");
	});
}

/// Unknown instruction data must be rejected by the program itself, not the
/// RPC layer.
#[test]
#[ignore = "run with pina test"]
fn unknown_discriminators_are_rejected() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let error = program
			.send(&[42], Vec::new())
			.expect_err("unknown discriminator is a program error");

		assert_eq!(error.operation(), "execute program instruction");
		assert!(!error.message().is_empty());

		let error = program
			.send(&[], Vec::new())
			.expect_err("empty instruction data is a program error");

		assert_eq!(error.operation(), "execute program instruction");

		program.stop().expect("stop isolated program test");
	});
}

/// Extra unrelated signers are harmless for a program that ignores accounts.
#[test]
#[ignore = "run with pina test"]
fn accepts_extra_signers() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let bystander = Keypair::new();
		program
			.fund(&bystander.pubkey(), 1_000_000_000)
			.expect("fund bystander");

		let instruction = program.instruction(
			&[DeclareIdInstruction::Initialize as u8],
			// A required signer meta adds the bystander to the message.
			vec![AccountMeta::new_readonly(bystander.pubkey(), true)],
		);
		program
			.send_with_signers(instruction, &[&bystander])
			.expect("execute Initialize with an extra signer");

		program.stop().expect("stop isolated program test");
	});
}
