#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::Keypair;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Signer;
use program_under_test::CounterInstruction;
use program_under_test::ID;

/// Seed prefix for counter PDAs, mirroring `SEED_COUNTER` in the program.
const SEED_COUNTER: &[u8] = b"counter";

/// The counter PDA is seeded by the authority's address.
fn pda_address(program_id: &Pubkey, authority: &Pubkey) -> Pubkey {
	Pubkey::find_program_address(&[SEED_COUNTER, authority.as_ref()], program_id).0
}

fn counter_bump(program_id: &Pubkey, authority: &Pubkey) -> u8 {
	let (address, bump) =
		Pubkey::try_find_program_address(&[SEED_COUNTER, authority.as_ref()], program_id)
			.expect("canonical bump for the counter PDA");
	assert_eq!(
		address.as_ref(),
		pda_address(program_id, authority).as_ref(),
		"counter PDA derivation drifted from the program's seeds"
	);

	bump
}

/// The 10-byte on-chain layout of a counter account.
fn counter_bytes(count: u64, bump: u8) -> Vec<u8> {
	let mut data = Vec::with_capacity(10);
	data.push(1);
	data.push(bump);
	data.extend_from_slice(&count.to_le_bytes());

	data
}

fn initialize_instruction(
	program: &ProgramTest,
	authority: &Pubkey,
	counter: &Pubkey,
	bump: u8,
) -> pina_test::Instruction {
	program.instruction(
		&[CounterInstruction::Initialize as u8, bump],
		vec![
			AccountMeta::new(*authority, true),
			AccountMeta::new(*counter, false),
			// The system program backs the create-account CPI.
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

fn increment_instruction(
	program: &ProgramTest,
	authority: &Pubkey,
	counter: &Pubkey,
) -> pina_test::Instruction {
	program.instruction(
		&[CounterInstruction::Increment as u8],
		vec![
			AccountMeta::new_readonly(*authority, true),
			AccountMeta::new(*counter, false),
		],
	)
}

#[test]
#[ignore = "run with pina test"]
fn initializes_the_counter_pda_with_zeroed_state() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let counter = pda_address(&program_id, &authority);
		let bump = counter_bump(&program_id, &authority);

		program
			.send_instruction(initialize_instruction(&program, &authority, &counter, bump))
			.expect("execute Initialize");

		let account = program.account(&counter).expect("fetch counter account");
		assert_eq!(account.owner, program_id, "counter is owned by the program");
		assert_eq!(account.data.len(), 10, "counter state layout is 10 bytes");
		assert_eq!(account.data[0], 1, "account discriminator is CounterState");
		assert_eq!(
			account.data[1], bump,
			"stored bump matches the canonical bump"
		);
		assert_eq!(
			account.data[2..],
			0u64.to_le_bytes(),
			"fresh counters start at zero"
		);

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn increments_persist_across_transactions() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let counter = pda_address(&program_id, &authority);
		let bump = counter_bump(&program_id, &authority);

		program
			.send_instruction(initialize_instruction(&program, &authority, &counter, bump))
			.expect("execute Initialize");

		for expected in 1u64..=3u64 {
			program
				.send_instruction(increment_instruction(&program, &authority, &counter))
				.expect("execute Increment");

			let account = program.account(&counter).expect("fetch counter account");
			assert_eq!(
				account.data[2..],
				expected.to_le_bytes(),
				"counter value after {expected} increments"
			);
		}

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn cannot_increment_an_uninitialized_counter() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let counter = pda_address(&program_id, &authority);

		let error = program
			.send_instruction(increment_instruction(&program, &authority, &counter))
			.expect_err("Increment rejects a missing counter account");

		assert_eq!(error.operation(), "execute program instruction");
		assert!(!error.message().is_empty());
		eprintln!("uninitialized counter increment error: {}", error.message());

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn cannot_initialize_an_existing_counter() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let counter = pda_address(&program_id, &authority);
		let bump = counter_bump(&program_id, &authority);

		program
			.send_instruction(initialize_instruction(&program, &authority, &counter, bump))
			.expect("first Initialize succeeds");

		// The account now exists, so the create-account CPI inside Initialize
		// must fail and must not disturb stored state.
		let error = program
			.send_instruction(initialize_instruction(&program, &authority, &counter, bump))
			.expect_err("Initialize rejects an already-initialized counter");

		assert_eq!(error.operation(), "execute program instruction");
		eprintln!("double initialize error: {}", error.message());

		let account = program.account(&counter).expect("fetch counter account");
		assert_eq!(
			account.data[2..],
			0u64.to_le_bytes(),
			"state survives the failed tx"
		);

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn counters_are_isolated_per_authority() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let first_authority = program.payer();
		let second_authority = Keypair::new();
		program
			.fund(&second_authority.pubkey(), 1_000_000_000)
			.expect("fund second authority");

		for (authority_key, authority_signer) in [
			(first_authority, None),
			(second_authority.pubkey(), Some(&second_authority)),
		] {
			let counter = pda_address(&program_id, &authority_key);
			let bump = counter_bump(&program_id, &authority_key);

			let initialize = initialize_instruction(&program, &authority_key, &counter, bump);
			let increment = increment_instruction(&program, &authority_key, &counter);

			if let Some(signer) = authority_signer {
				program
					.send_with_signers(initialize, &[signer])
					.expect("execute Initialize for a funded authority");
				program
					.send_with_signers(increment, &[signer])
					.expect("increment a funded authority");
			} else {
				program
					.send_instruction(initialize)
					.expect("execute Initialize for the payer");
				program
					.send_instruction(increment)
					.expect("increment the payer authority");
			}

			let account = program.account(&counter).expect("fetch counter account");
			assert_eq!(account.data[2..], 1u64.to_le_bytes());
		}

		// The first authority's counter still reads 1 while the second was
		// initialized and incremented afterwards.
		let first_counter = pda_address(&program_id, &first_authority);
		let account = program
			.account(&first_counter)
			.expect("fetch first counter");
		assert_eq!(account.data[2..], 1u64.to_le_bytes());

		program.stop().expect("stop isolated program test");
	});
}
