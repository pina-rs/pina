#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use program_under_test::CheckPolicyInstruction;
use program_under_test::ID;
use program_under_test::InitializePolicyInstruction;
use program_under_test::PolicyState;
use program_under_test::ValidationInstruction;

const POLICY_SEED: &[u8] = b"validation-policy";

fn policy_address(program_id: &Pubkey, authority: &Pubkey) -> (Pubkey, u8) {
	Pubkey::find_program_address(&[POLICY_SEED, authority.as_ref()], program_id)
}

fn initialize_data(bump: u8, minimum: u64, maximum: u64) -> Vec<u8> {
	let mut bytes = vec![0u8; InitializePolicyInstruction::SIZE];
	InitializePolicyInstruction::initialize(&mut bytes, |args| {
		args.bump = bump;
		args.minimum.set(minimum);
		args.maximum.set(maximum);
		args.required_approvals = 2;
		Ok(())
	})
	.expect("build validated InitializePolicy instruction");
	bytes
}

fn check_data(amount: u64) -> Vec<u8> {
	let mut bytes = vec![0u8; CheckPolicyInstruction::SIZE];
	CheckPolicyInstruction::initialize(&mut bytes, |args| {
		args.amount.set(amount);
		args.memo.try_set("release payment")?;
		args.approvals.try_set([7, 9])?;
		Ok(())
	})
	.expect("build validated CheckPolicy instruction");
	bytes
}

fn initialize_accounts(authority: Pubkey, policy: Pubkey) -> Vec<AccountMeta> {
	vec![
		AccountMeta::new(authority, true),
		AccountMeta::new(policy, false),
		AccountMeta::new_readonly(Pubkey::default(), false),
	]
}

fn check_accounts(authority: Pubkey, policy: Pubkey) -> Vec<AccountMeta> {
	vec![
		AccountMeta::new_readonly(authority, true),
		AccountMeta::new_readonly(policy, false),
		// Reuse the transaction payer as a shared account whose writable flag
		// is part of the declarative validation contract.
		AccountMeta::new(authority, true),
		AccountMeta::new_readonly(Pubkey::default(), false),
	]
}

#[test]
#[ignore = "run with pina test"]
fn validates_every_program_boundary() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");
		let authority = program.payer();
		let (policy, bump) = policy_address(&program_id, &authority);

		program
			.send(
				&initialize_data(bump, 100, 1_000),
				initialize_accounts(authority, policy),
			)
			.expect("initialize a validated policy account");

		let account = program.account(&policy).expect("fetch policy account");
		assert_eq!(account.owner, program_id);
		assert_eq!(account.data.len(), PolicyState::SIZE);

		program
			.send(&check_data(500), check_accounts(authority, policy))
			.expect("validate the instruction, accounts, state, and event");

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn rejects_valid_payloads_that_violate_loaded_policy_state() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");
		let authority = program.payer();
		let (policy, bump) = policy_address(&program_id, &authority);

		program
			.send(
				&initialize_data(bump, 100, 1_000),
				initialize_accounts(authority, policy),
			)
			.expect("initialize a validated policy account");

		let error = program
			.send(&check_data(50), check_accounts(authority, policy))
			.expect_err("reject an amount below the stored policy minimum");
		assert_eq!(error.operation(), "execute program instruction");
		assert!(!error.message().is_empty());

		program.stop().expect("stop isolated program test");
	});
}

#[test]
fn instruction_discriminators_remain_client_visible() {
	assert_eq!(ValidationInstruction::InitializePolicy as u8, 0);
	assert_eq!(ValidationInstruction::CheckPolicy as u8, 1);
}
