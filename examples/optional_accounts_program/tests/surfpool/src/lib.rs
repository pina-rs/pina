#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use program_under_test::ID;
use program_under_test::OptionalInstruction;

/// Seed prefix for store PDAs, mirroring `STORE_SEED` in the program.
const STORE_SEED: &[u8] = b"store";

fn pda_address(program_id: &Pubkey, authority: &Pubkey) -> Pubkey {
	Pubkey::find_program_address(&[STORE_SEED, authority.as_ref()], program_id).0
}

fn bump_for(program_id: &Pubkey, authority: &Pubkey) -> u8 {
	Pubkey::try_find_program_address(&[STORE_SEED, authority.as_ref()], program_id)
		.expect("canonical bump for the store PDA")
		.1
}

fn init_instruction(
	program: &ProgramTest,
	authority: &Pubkey,
	store: &Pubkey,
	bump: u8,
) -> pina_test::Instruction {
	program.instruction(
		&[OptionalInstruction::Init as u8, bump],
		vec![
			AccountMeta::new(*authority, true),
			AccountMeta::new(*store, false),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

/// Omitted optional slots fill with a readonly meta pointing at the program
/// ID; the store PDA is created and starts at zero.
#[test]
#[ignore = "run with pina test"]
fn init_creates_the_store_pda_at_zero() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let store = pda_address(&program_id, &authority);
		let bump = bump_for(&program_id, &authority);

		program
			.send_instruction(init_instruction(&program, &authority, &store, bump))
			.expect("execute Init");

		let account = program.account(&store).expect("fetch store account");
		assert_eq!(account.owner, program_id);
		assert_eq!(account.data.len(), 10, "StoreState layout is 10 bytes");
		assert_eq!(account.data[0], 1, "account discriminator is StoreState");
		assert_eq!(account.data[1], bump);
		assert_eq!(
			account.data[2..],
			0u64.to_le_bytes(),
			"count starts at zero"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// Touch with the store slot provided increments the on-chain counter; passing
/// the program ID instead skips mutation.
#[test]
#[ignore = "run with pina test"]
fn touch_increments_only_when_the_store_is_present() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let store = pda_address(&program_id, &authority);
		let bump = bump_for(&program_id, &authority);

		program
			.send_instruction(init_instruction(&program, &authority, &store, bump))
			.expect("execute Init");

		let present = |program: &ProgramTest| {
			program.instruction(
				&[OptionalInstruction::Touch as u8],
				vec![
					AccountMeta::new_readonly(authority, true),
					AccountMeta::new(store, false),
				],
			)
		};
		program
			.send_instruction(present(&program))
			.expect("Touch with the store present");
		let account = program.account(&store).expect("fetch store account");
		assert_eq!(
			account.data[2..],
			1u64.to_le_bytes(),
			"one provided touch adds 1"
		);

		let omitted = program.instruction(
			&[OptionalInstruction::Touch as u8],
			vec![
				AccountMeta::new_readonly(authority, true),
				AccountMeta::new_readonly(program_id, false),
			],
		);
		program
			.send_instruction(omitted)
			.expect("Touch with the store omitted");
		let account = program.account(&store).expect("fetch store account");
		assert_eq!(
			account.data[2..],
			1u64.to_le_bytes(),
			"omitted slot leaves the counter untouched"
		);

		program
			.send_instruction(present(&program))
			.expect("second provided Touch");
		let account = program.account(&store).expect("fetch store account");
		assert_eq!(
			account.data[2..],
			2u64.to_le_bytes(),
			"the counter reaches 2 of 3 touches"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// A store slot that is not the authority's PDA must fail the type guard.
#[test]
#[ignore = "run with pina test"]
fn touch_rejects_a_wrong_type_store() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let store = pda_address(&program_id, &authority);
		let bump = bump_for(&program_id, &authority);

		program
			.send_instruction(init_instruction(&program, &authority, &store, bump))
			.expect("execute Init");

		let impostor = Pubkey::new_unique();
		program
			.fund(&impostor, 1_000_000_000)
			.expect("fund impostor");

		let instruction = program.instruction(
			&[OptionalInstruction::Touch as u8],
			vec![
				AccountMeta::new_readonly(authority, true),
				AccountMeta::new(impostor, false),
			],
		);

		let error = program
			.send_instruction(instruction)
			.expect_err("a random account must fail the store guard");
		assert_eq!(error.operation(), "execute program instruction");
		eprintln!("wrong store error: {}", error.message());

		program.stop().expect("stop isolated program test");
	});
}

/// `Inspect` with an unsigned witness must be rejected even though the
/// witness slot is optional.
#[test]
#[ignore = "run with pina test"]
fn inspect_enforces_the_witness_signer_when_provided() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let store = pda_address(&program_id, &authority);
		let bump = bump_for(&program_id, &authority);

		program
			.send_instruction(init_instruction(&program, &authority, &store, bump))
			.expect("execute Init");

		let witness = Pubkey::new_unique();
		program.fund(&witness, 1_000_000_000).expect("fund witness");

		let instruction = program.instruction(
			&[OptionalInstruction::Inspect as u8],
			vec![
				AccountMeta::new_readonly(authority, true),
				AccountMeta::new_readonly(store, false),
				AccountMeta::new_readonly(witness, false),
			],
		);

		let error = program
			.send_instruction(instruction)
			.expect_err("a provided witness must sign");
		assert_eq!(error.operation(), "execute program instruction");
		eprintln!("unsigned witness error: {}", error.message());

		program.stop().expect("stop isolated program test");
	});
}
