#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::HistoricalAccount;
use pina_test::Keypair;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Signer;
use program_under_test::ID;

const STATE_DISCRIMINATOR: u8 = 1;
const UPDATE_DISCRIMINATOR: u8 = 0;
const RELAY_DISCRIMINATOR: u8 = 1;
const CURRENT_STATE_SIZE: usize = 43;

fn historical_update_data(value: u64) -> Vec<u8> {
	let mut data = vec![UPDATE_DISCRIMINATOR, 0];
	data.extend_from_slice(&value.to_le_bytes());
	data
}

fn relay_data(value: u64) -> Vec<u8> {
	let mut data = vec![RELAY_DISCRIMINATOR];
	data.extend_from_slice(&value.to_le_bytes());
	data
}

fn historical_state_data(authority: &Pubkey, value: u64) -> Vec<u8> {
	let mut data = vec![STATE_DISCRIMINATOR, 0];
	data.extend_from_slice(authority.as_ref());
	data.extend_from_slice(&value.to_le_bytes());
	data
}

/// A released v0 client can still send its shorter payload and one-account
/// process after v1 adds payload data and optional account slots.
#[test]
#[ignore = "run with pina test"]
fn historical_update_confirms_without_appended_optional_accounts() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");
		let authority = program.payer();

		program
			.send(
				&historical_update_data(42),
				vec![AccountMeta::new_readonly(authority, true)],
			)
			.expect("execute historical Update");

		program.stop().expect("stop isolated program test");
	});
}

/// The current generated client owns the migration version and emits it
/// without asking the caller to understand the account's migration history.
#[test]
#[ignore = "run with pina test"]
fn generated_current_update_writes_the_current_version() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");
		let authority = program.payer();
		let data = generated_client::instructions::UpdateInstructionData::new(|instruction| {
			instruction.value.set(42);
			instruction.memo.set(7);
		})
		.expect("encode current Update data");
		let instruction = generated_client::instructions::Update::new(authority).instruction(data);

		assert_eq!(&instruction.data[0..2], &[UPDATE_DISCRIMINATOR, 1]);
		program
			.send_instruction(instruction)
			.expect("execute generated current Update");

		program.stop().expect("stop isolated program test");
	});
}

/// A self-CPI can migrate stale state, resize it, execute current business
/// logic, and return a fresh readable account view to its caller.
#[test]
#[ignore = "run with pina test"]
fn relay_migrates_historical_state_through_self_cpi() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");
		let authority = program.payer();
		let referrer = Pubkey::new_from_array([0x11; 32]);
		let state = Pubkey::new_from_array([0x22; 32]);
		let migration_payer = Keypair::new_from_array([0x33; 32]);

		program
			.fund(&referrer, 1_000_000)
			.expect("fund deterministic referrer");
		program
			.fund(&migration_payer.pubkey(), 1_000_000_000)
			.expect("fund deterministic migration payer");
		program
			.install_historical_account(&HistoricalAccount::new(
				0,
				state,
				program_id,
				historical_state_data(&authority, 7),
			))
			.expect("install historical state");

		let instruction = program.instruction(
			&relay_data(144),
			vec![
				AccountMeta::new_readonly(authority, true),
				AccountMeta::new_readonly(referrer, false),
				AccountMeta::new(state, false),
				AccountMeta::new(migration_payer.pubkey(), true),
				AccountMeta::new_readonly(Pubkey::default(), false),
				AccountMeta::new_readonly(program_id, false),
			],
		);
		program
			.send_with_signers(instruction, &[&migration_payer])
			.expect("execute Relay through self-CPI");

		let account = program.account(&state).expect("fetch migrated state");
		assert_eq!(account.data.len(), CURRENT_STATE_SIZE);
		assert_eq!(&account.data[0..2], &[STATE_DISCRIMINATOR, 1]);
		assert_eq!(&account.data[2..34], authority.as_ref());
		assert_eq!(&account.data[34..42], &144_u64.to_le_bytes());
		assert_eq!(account.data[42], 1);

		program.stop().expect("stop isolated program test");
	});
}
