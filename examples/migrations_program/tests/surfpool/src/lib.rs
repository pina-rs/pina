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
const CURRENT_STATE_SIZE: usize = 44;
const MIGRATE_DISCRIMINATOR: u8 = 0xff;

/// The well-known system program that the reserved instruction's rent
/// transfers invoke.
fn system_program() -> Pubkey {
	Pubkey::new_from_array([0; 32])
}

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

/// The reserved framework instruction migrates a stale account on its own:
/// no business instruction runs, and the payer authorizes exactly the
/// migration cost.
#[test]
#[ignore = "run with pina test"]
fn reserved_migrate_instruction_advances_a_stale_account() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");
		let authority = program.payer();
		let state = Pubkey::new_from_array([0x22; 32]);
		program
			.install_historical_account(&HistoricalAccount::new(
				0,
				state,
				program_id,
				historical_state_data(&authority, 7),
			))
			.expect("install historical state");

		let stale = program.account(&state).expect("read stale state");
		assert_eq!(stale.data.len(), 42);

		program
			.send(
				&[MIGRATE_DISCRIMINATOR],
				vec![
					AccountMeta::new(authority, true),
					AccountMeta::new_readonly(system_program(), false),
					AccountMeta::new(state, false),
				],
			)
			.expect("execute reserved Migrate");

		let migrated = program.account(&state).expect("read migrated state");
		assert_eq!(migrated.data.len(), CURRENT_STATE_SIZE);
		assert_eq!(migrated.data[1], 2);

		program.stop().expect("stop isolated program test");
	});
}

/// A client may send only the slots it needs and fill the rest with the
/// program address; a foreign account in a declared slot fails closed.
#[test]
#[ignore = "run with pina test"]
fn reserved_migrate_instruction_skips_placeholders_and_rejects_foreign_accounts() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");
		let authority = program.payer();
		let state = Pubkey::new_from_array([0x24; 32]);
		program
			.install_historical_account(&HistoricalAccount::new(
				0,
				state,
				program_id,
				historical_state_data(&authority, 7),
			))
			.expect("install historical state");

		// The trailing slots hold the program-address placeholder and are skipped.
		program
			.send(
				&[MIGRATE_DISCRIMINATOR],
				vec![
					AccountMeta::new(authority, true),
					AccountMeta::new_readonly(system_program(), false),
					AccountMeta::new(state, false),
					AccountMeta::new_readonly(program_id, false),
					AccountMeta::new_readonly(program_id, false),
				],
			)
			.expect("execute reserved Migrate with placeholders");
		assert_eq!(
			program
				.account(&state)
				.expect("read migrated state")
				.data
				.len(),
			CURRENT_STATE_SIZE
		);

		// A funded account is owned by the system program, not by this program.
		let foreign = Pubkey::new_from_array([0x25; 32]);
		program
			.fund(&foreign, 1_000_000)
			.expect("fund foreign account");
		let rejection = program.send(
			&[MIGRATE_DISCRIMINATOR],
			vec![
				AccountMeta::new(authority, true),
				AccountMeta::new_readonly(system_program(), false),
				AccountMeta::new(foreign, false),
			],
		);
		assert!(rejection.is_err(), "a foreign account was migrated");

		program.stop().expect("stop isolated program test");
	});
}

/// The reserved instruction charges one shared lamport budget: a cap that
/// funds one growing account must not fund two in the same invocation.
#[test]
#[ignore = "run with pina test"]
fn reserved_migrate_instruction_shares_one_lamport_budget_across_slots() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");
		let authority = program.payer();
		let state_a = Pubkey::new_from_array([0x26; 32]);
		let state_b = Pubkey::new_from_array([0x27; 32]);
		for state in [&state_a, &state_b] {
			program
				.install_historical_account(&HistoricalAccount::new(
					0,
					*state,
					program_id,
					historical_state_data(&authority, 7),
				))
				.expect("install historical state");
		}

		// Each v0 state is rent-exempt at 42 bytes, so each migration tops up
		// two bytes of rent (13,920 lamports). The 20,000-lamport cap funds
		// either account alone but never both, and the transaction fails
		// atomically with the budget error.
		let rejection = program.send(
			&[MIGRATE_DISCRIMINATOR],
			vec![
				AccountMeta::new(authority, true),
				AccountMeta::new_readonly(system_program(), false),
				AccountMeta::new(state_a, false),
				AccountMeta::new_readonly(program_id, false),
				AccountMeta::new_readonly(program_id, false),
				AccountMeta::new(state_b, false),
			],
		);
		let error = rejection.expect_err("two growing migrations must exceed the shared budget");
		assert!(
			error.message().contains("fffffff5"),
			"expected the migration budget error, got: {}",
			error.message()
		);
		for state in [&state_a, &state_b] {
			assert_eq!(
				program
					.account(state)
					.expect("read rolled back state")
					.data
					.len(),
				42,
				"a failed sweep must leave every account stale"
			);
		}

		// The same cap funds one account alone, so the rejection above was the
		// shared budget, not an undersized per-account one.
		program
			.send(
				&[MIGRATE_DISCRIMINATOR],
				vec![
					AccountMeta::new(authority, true),
					AccountMeta::new_readonly(system_program(), false),
					AccountMeta::new_readonly(program_id, false),
					AccountMeta::new_readonly(program_id, false),
					AccountMeta::new_readonly(program_id, false),
					AccountMeta::new(state_b, false),
				],
			)
			.expect("execute reserved Migrate within the shared budget");
		assert_eq!(
			program
				.account(&state_b)
				.expect("read migrated state")
				.data
				.len(),
			CURRENT_STATE_SIZE
		);

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

		assert_eq!(&instruction.data[0..2], &[UPDATE_DISCRIMINATOR, 2]);
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
		assert_eq!(&account.data[0..2], &[STATE_DISCRIMINATOR, 2]);
		assert_eq!(&account.data[2..34], authority.as_ref());
		assert_eq!(&account.data[34..42], &144_u64.to_le_bytes());
		assert_eq!(account.data[42], 1);
		assert_eq!(account.data[43], 1);

		program.stop().expect("stop isolated program test");
	});
}
