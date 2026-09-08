#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use program_under_test::DuplicateMutableInstruction;
use program_under_test::ID;

/// Builds the account list used by the instruction under test.
fn accounts(first: &Pubkey, second: &Pubkey, readonly: bool) -> Vec<AccountMeta> {
	if readonly {
		vec![
			AccountMeta::new_readonly(*first, false),
			AccountMeta::new_readonly(*second, false),
		]
	} else {
		vec![
			AccountMeta::new(*first, false),
			AccountMeta::new(*second, false),
		]
	}
}

/// Distinct writable accounts satisfy the guard; the instruction confirms.
#[test]
#[ignore = "run with pina test"]
fn fails_duplicate_mutable_accepts_distinct_accounts() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let first = program.payer();
		let second = Pubkey::new_unique();
		program.fund(&second, 1_000_000_000).expect("fund second");

		program
			.send(
				&[DuplicateMutableInstruction::FailsDuplicateMutable as u8],
				accounts(&first, &second, false),
			)
			.expect("distinct writable accounts pass the guard");

		program.stop().expect("stop isolated program test");
	});
}

/// Passing the same writable account twice is exactly what the program must
/// refuse: the runtime surfaces the custom error code verbatim.
#[test]
#[ignore = "run with pina test"]
fn fails_duplicate_mutable_rejects_the_same_account() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let same = program.payer();
		let error = program
			.send(
				&[DuplicateMutableInstruction::FailsDuplicateMutable as u8],
				accounts(&same, &same, false),
			)
			.expect_err("a duplicated writable account must fail");

		assert_eq!(error.operation(), "execute program instruction");
		assert!(
			error.message().contains("0x7f8"),
			"expected ConstraintDuplicateMutableAccount (2040), got: {}",
			error.message()
		);

		// Sanity: nothing about the explicit guard is broken — distinct
		// accounts still work on the same instance.
		let other = Pubkey::new_unique();
		program.fund(&other, 1_000_000_000).expect("fund other");
		program
			.send(
				&[DuplicateMutableInstruction::FailsDuplicateMutable as u8],
				accounts(&same, &other, false),
			)
			.expect("the guard accepts distinct accounts after the failure");

		program.stop().expect("stop isolated program test");
	});
}

/// The compatibility instruction allows duplicated writable metas by design.
#[test]
#[ignore = "run with pina test"]
fn allows_duplicate_mutable_and_readonly() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let same = program.payer();

		program
			.send(
				&[DuplicateMutableInstruction::AllowsDuplicateMutable as u8],
				accounts(&same, &same, false),
			)
			.expect("duplicated writable metas are allowed here");

		program
			.send(
				&[DuplicateMutableInstruction::AllowsDuplicateReadonly as u8],
				accounts(&same, &same, true),
			)
			.expect("duplicated readonly metas are always allowed");

		// Readonly duplicates stay valid across instruction variants.
		program
			.send(
				&[DuplicateMutableInstruction::FailsDuplicateMutable as u8],
				accounts(&same, &same, true),
			)
			.expect_err("FailsDuplicateMutable requires writable accounts");

		program.stop().expect("stop isolated program test");
	});
}
