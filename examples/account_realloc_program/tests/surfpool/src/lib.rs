#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use program_under_test::ID;
use program_under_test::ReallocInstruction;

/// Seed prefix for sample PDAs, mirroring `SEED_SAMPLE` in the program.
const SEED_SAMPLE: &[u8] = b"sample";

/// The Sample header is discriminator + bump + authority + u16 value count.
const SAMPLE_HEADER_LEN: usize = 36;

/// Maximum bytes occupied by the compact tail's 64 u64 values.
const MAX_COMPACT_TAIL_LEN: u16 = 512;

fn pda_address(program_id: &Pubkey, authority: &Pubkey) -> Pubkey {
	Pubkey::find_program_address(&[SEED_SAMPLE, authority.as_ref()], program_id).0
}

fn bump_for(program_id: &Pubkey, authority: &Pubkey) -> u8 {
	Pubkey::try_find_program_address(&[SEED_SAMPLE, authority.as_ref()], program_id)
		.expect("canonical bump for the sample PDA")
		.1
}

fn initialize_instruction(
	program: &ProgramTest,
	authority: &Pubkey,
	sample: &Pubkey,
	bump: u8,
) -> pina_test::Instruction {
	program.instruction(
		&[ReallocInstruction::Initialize as u8, bump],
		vec![
			AccountMeta::new(*authority, true),
			AccountMeta::new(*sample, false),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

fn realloc_instruction(
	program: &ProgramTest,
	authority: &Pubkey,
	sample: &Pubkey,
	len: u16,
) -> pina_test::Instruction {
	let mut data = vec![ReallocInstruction::Realloc as u8];
	data.extend_from_slice(&len.to_le_bytes());

	program.instruction(
		&data,
		vec![
			AccountMeta::new(*authority, true),
			AccountMeta::new(*sample, false),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

fn realloc2_instruction(
	program: &ProgramTest,
	authority: &Pubkey,
	first: &Pubkey,
	second: &Pubkey,
) -> pina_test::Instruction {
	// Realloc2 carries a legacy ignored `len` field in its wire format.
	let mut data = vec![ReallocInstruction::Realloc2 as u8];
	data.extend_from_slice(&0u16.to_le_bytes());

	program.instruction(
		&data,
		vec![
			AccountMeta::new(*authority, true),
			AccountMeta::new(*first, false),
			AccountMeta::new(*second, false),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

/// Initialize creates the sample PDA with the fixed 36-byte compact header.
#[test]
#[ignore = "run with pina test"]
fn initializes_the_sample_account() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let sample = pda_address(&program_id, &authority);
		let bump = bump_for(&program_id, &authority);

		program
			.send_instruction(initialize_instruction(&program, &authority, &sample, bump))
			.expect("execute Initialize");

		let account = program.account(&sample).expect("fetch sample account");
		assert_eq!(account.owner, program_id);
		assert_eq!(account.data.len(), SAMPLE_HEADER_LEN);
		assert_eq!(account.data[0], 1, "account discriminator is Sample");
		assert_eq!(account.data[1], bump);
		assert_eq!(&account.data[2..34], authority.to_bytes());
		assert_eq!(&account.data[34..36], &[0, 0]);

		program.stop().expect("stop isolated program test");
	});
}

/// A single Realloc grows the account by one permitted delta and the bytes
/// actually move on-chain.
#[test]
#[ignore = "run with pina test"]
fn realloc_grows_within_the_increase_limit() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let sample = pda_address(&program_id, &authority);
		let bump = bump_for(&program_id, &authority);

		program
			.send_instruction(initialize_instruction(&program, &authority, &sample, bump))
			.expect("execute Initialize");

		let grown = SAMPLE_HEADER_LEN + usize::from(MAX_COMPACT_TAIL_LEN);
		program
			.send_instruction(realloc_instruction(
				&program,
				&authority,
				&sample,
				grown.try_into().expect("grown len"),
			))
			.expect("execute Realloc to +1024");

		let account = program.account(&sample).expect("fetch sample account");
		assert_eq!(
			account.data.len(),
			grown,
			"realloc moved the account length on-chain"
		);
		assert_eq!(&account.data[34..36], &64u16.to_le_bytes());
		for (index, value) in account.data[SAMPLE_HEADER_LEN..]
			.chunks_exact(8)
			.enumerate()
		{
			assert_eq!(
				u64::from_le_bytes(value.try_into().expect("u64 value bytes")),
				u64::try_from(index).expect("value index fits u64"),
			);
		}

		program.stop().expect("stop isolated program test");
	});
}

/// The compact account refuses allocations beyond its declared 64-value cap.
#[test]
#[ignore = "run with pina test"]
fn realloc_growth_beyond_the_compact_capacity_is_rejected() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let sample = pda_address(&program_id, &authority);
		let bump = bump_for(&program_id, &authority);

		program
			.send_instruction(initialize_instruction(&program, &authority, &sample, bump))
			.expect("execute Initialize");

		let target = SAMPLE_HEADER_LEN + usize::from(MAX_COMPACT_TAIL_LEN) + 8;
		let error = program
			.send_instruction(realloc_instruction(
				&program,
				&authority,
				&sample,
				target.try_into().expect("target len"),
			))
			.expect_err("compact capacity must bound account growth");

		let account = program.account(&sample).expect("fetch sample account");
		assert_eq!(account.data.len(), SAMPLE_HEADER_LEN);
		assert_eq!(error.operation(), "execute program instruction");

		program.stop().expect("stop isolated program test");
	});
}

/// The second realloc instruction never resizes anything: it exists to catch
/// duplicate-target parsing, so the same account must be refused.
#[test]
#[ignore = "run with pina test"]
fn realloc2_rejects_a_duplicated_sample() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let sample = pda_address(&program_id, &authority);
		let bump = bump_for(&program_id, &authority);

		program
			.send_instruction(initialize_instruction(&program, &authority, &sample, bump))
			.expect("execute Initialize");

		let error = program
			.send_instruction(realloc2_instruction(&program, &authority, &sample, &sample))
			.expect_err("Realloc2 must refuse the same sample twice");

		assert_eq!(error.operation(), "execute program instruction");
		eprintln!("duplicate realloc2 error: {}", error.message());

		program.stop().expect("stop isolated program test");
	});
}

/// Realloc2 validates both samples against the signing authority; a sample
/// backed by a different authority is rejected before the duplicate check.
#[test]
#[ignore = "run with pina test"]
fn realloc2_rejects_a_foreign_authority_sample() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let sample = pda_address(&program_id, &authority);
		let bump = bump_for(&program_id, &authority);

		program
			.send_instruction(initialize_instruction(&program, &authority, &sample, bump))
			.expect("initialize sample");

		let foreign = Pubkey::new_from_array([2; 32]);
		program.fund(&foreign, 1_000_000_000).expect("fund foreign");

		let error = program
			.send_instruction(realloc2_instruction(
				&program, &authority, &sample, &foreign,
			))
			.expect_err("Realloc2 must only accept two samples of its authority");

		assert_eq!(error.operation(), "execute program instruction");
		eprintln!("foreign realloc2 error: {}", error.message());

		program.stop().expect("stop isolated program test");
	});
}
