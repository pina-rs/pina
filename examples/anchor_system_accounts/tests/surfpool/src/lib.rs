#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use program_under_test::ID;
use program_under_test::SystemAccountsInstruction;

#[test]
#[ignore = "run with pina test"]
fn accepts_an_authority_and_a_system_owned_wallet() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let wallet = Pubkey::new_unique();
		program.fund(&wallet, 1_000_000_000).expect("fund wallet");

		program
			.send(
				&[SystemAccountsInstruction::Initialize as u8],
				vec![
					AccountMeta::new_readonly(authority, true),
					AccountMeta::new_readonly(wallet, false),
				],
			)
			.expect("execute Initialize");

		program.stop().expect("stop isolated program test");
	});
}

/// The wallet guard rejects accounts owned by other programs. The deployed
/// program account itself is owned by the BPF loader, so passing it as the
/// wallet exercises the rejection path against a real, existing account.
#[test]
#[ignore = "run with pina test"]
fn rejects_a_wallet_that_is_not_system_owned() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let wallet = program_id;

		let error = program
			.send(
				&[SystemAccountsInstruction::Initialize as u8],
				vec![
					AccountMeta::new_readonly(authority, true),
					AccountMeta::new_readonly(wallet, false),
				],
			)
			.expect_err("a BPF-loader-owned wallet must be rejected");

		assert_eq!(error.operation(), "execute program instruction");
		eprintln!("wrong wallet owner error: {}", error.message());

		program.stop().expect("stop isolated program test");
	});
}

/// The authority must sign — this is rejected inside the program.
#[test]
#[ignore = "run with pina test"]
fn requires_authority_signature() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		// A bystander account that appears unsigned cannot satisfy the guard.
		let authority = Pubkey::new_unique();
		let wallet = program.payer();
		let error = program
			.send(
				&[SystemAccountsInstruction::Initialize as u8],
				vec![
					AccountMeta::new_readonly(authority, false),
					AccountMeta::new_readonly(wallet, false),
				],
			)
			.expect_err("an unsigned authority must be rejected");

		assert_eq!(error.operation(), "execute program instruction");

		program.stop().expect("stop isolated program test");
	});
}
