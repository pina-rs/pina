#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::Keypair;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Rent;
use pina_test::Signer;
use program_under_test::CompactInstruction;
use program_under_test::ID;
use program_under_test::Journal;
use program_under_test::MAX_ENTRIES;

const SEED_JOURNAL: &[u8] = b"compact-journal";

fn journal_pda(program_id: &Pubkey, authority: &Pubkey) -> (Pubkey, u8) {
	Pubkey::find_program_address(&[SEED_JOURNAL, authority.as_ref()], program_id)
}

fn initialize_instruction(
	program: &ProgramTest,
	authority: &Pubkey,
	journal: &Pubkey,
	bump: u8,
	entry_count: u8,
) -> pina_test::Instruction {
	program.instruction(
		&[CompactInstruction::Initialize as u8, bump, entry_count],
		vec![
			AccountMeta::new(*authority, true),
			AccountMeta::new(*journal, false),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

fn resize_instruction(
	program: &ProgramTest,
	authority: &Pubkey,
	journal: &Pubkey,
	entry_count: u8,
) -> pina_test::Instruction {
	program.instruction(
		&[CompactInstruction::Resize as u8, entry_count],
		vec![
			AccountMeta::new(*authority, true),
			AccountMeta::new(*journal, false),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

fn write_instruction(
	program: &ProgramTest,
	authority: &Pubkey,
	journal: &Pubkey,
	index: u8,
	value: u64,
) -> pina_test::Instruction {
	let mut data = vec![CompactInstruction::Write as u8, index];
	data.extend_from_slice(&value.to_le_bytes());
	program.instruction(
		&data,
		vec![
			AccountMeta::new_readonly(*authority, true),
			AccountMeta::new(*journal, false),
		],
	)
}

fn assert_journal(
	program: &ProgramTest,
	journal: &Pubkey,
	authority: &Pubkey,
	revision: u32,
	expected_entries: &[u64],
) {
	let account = program.account(journal).expect("fetch journal account");
	let expected_size = Journal::HEADER_SIZE + expected_entries.len() * 9;
	assert_eq!(account.owner, program.program_id());
	assert_eq!(account.data.len(), expected_size);
	assert_eq!(
		account.lamports,
		Rent::default().minimum_balance(expected_size),
		"compact realloc keeps exactly the rent-exempt minimum",
	);
	assert_eq!(account.data[0], 1, "journal discriminator");
	assert_eq!(&account.data[2..34], authority.as_ref());
	assert_eq!(
		u32::from_le_bytes(account.data[34..38].try_into().expect("revision bytes")),
		revision
	);
	assert_eq!(
		u16::from_le_bytes(account.data[38..40].try_into().expect("entry-count bytes")) as usize,
		expected_entries.len(),
	);
	assert_eq!(
		u16::from_le_bytes(account.data[40..42].try_into().expect("marker-count bytes")) as usize,
		expected_entries.len(),
	);
	let entries_end = Journal::HEADER_SIZE + expected_entries.len() * 8;
	let entries = account.data[Journal::HEADER_SIZE..entries_end]
		.chunks_exact(8)
		.map(|bytes| u64::from_le_bytes(bytes.try_into().expect("entry bytes")))
		.collect::<Vec<_>>();
	assert_eq!(entries, expected_entries);
	assert_eq!(
		&account.data[entries_end..],
		&(0..expected_entries.len() as u8).collect::<Vec<_>>(),
	);
}

#[test]
#[ignore = "run with pina test"]
fn initializes_at_header_only_and_at_a_nonempty_size() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");
		let authority = program.payer();
		let (empty, empty_bump) = journal_pda(&program_id, &authority);

		program
			.send_instruction(initialize_instruction(
				&program, &authority, &empty, empty_bump, 0,
			))
			.expect("initialize header-only journal");
		assert_journal(&program, &empty, &authority, 0, &[]);

		let second_authority = Keypair::new();
		program
			.fund(&second_authority.pubkey(), 1_000_000_000)
			.expect("fund second authority");
		let (nonempty, nonempty_bump) = journal_pda(&program_id, &second_authority.pubkey());
		program
			.send_with_signers(
				initialize_instruction(
					&program,
					&second_authority.pubkey(),
					&nonempty,
					nonempty_bump,
					3,
				),
				&[&second_authority],
			)
			.expect("initialize nonempty journal");
		assert_journal(
			&program,
			&nonempty,
			&second_authority.pubkey(),
			0,
			&[0, 1, 2],
		);

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn grows_updates_without_reallocating_shrinks_and_clears() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");
		let authority = program.payer();
		let (journal, bump) = journal_pda(&program_id, &authority);
		program
			.send_instruction(initialize_instruction(
				&program, &authority, &journal, bump, 2,
			))
			.expect("initialize journal");

		program
			.send_instruction(resize_instruction(
				&program,
				&authority,
				&journal,
				MAX_ENTRIES as u8,
			))
			.expect("grow to full capacity");
		let balance_after_growth = program.balance(&authority).expect("balance after growth");
		assert_journal(&program, &journal, &authority, 1, &[0, 1, 2, 3, 4, 5, 6, 7]);

		program
			.send_instruction(resize_instruction(
				&program,
				&authority,
				&journal,
				MAX_ENTRIES as u8,
			))
			.expect("same-size resize");
		assert_journal(&program, &journal, &authority, 2, &[0, 1, 2, 3, 4, 5, 6, 7]);

		program
			.send_instruction(write_instruction(&program, &authority, &journal, 3, 99))
			.expect("write without resizing");
		assert_journal(
			&program,
			&journal,
			&authority,
			3,
			&[0, 1, 2, 99, 4, 5, 6, 7],
		);

		program
			.send_instruction(resize_instruction(&program, &authority, &journal, 3))
			.expect("shrink journal");
		let balance_after_shrink = program.balance(&authority).expect("balance after shrink");
		assert!(
			balance_after_shrink > balance_after_growth,
			"rent refund from removing five entries exceeds the transaction fee",
		);
		assert_journal(&program, &journal, &authority, 4, &[0, 1, 2]);

		program
			.send_instruction(resize_instruction(&program, &authority, &journal, 0))
			.expect("clear to header-only");
		assert_journal(&program, &journal, &authority, 5, &[]);

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn rejected_growth_past_capacity_preserves_data_and_lamports() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");
		let authority = program.payer();
		let (journal, bump) = journal_pda(&program_id, &authority);
		program
			.send_instruction(initialize_instruction(
				&program, &authority, &journal, bump, 2,
			))
			.expect("initialize journal");
		let before = program.account(&journal).expect("journal before rejection");

		program
			.send_instruction(resize_instruction(
				&program,
				&authority,
				&journal,
				(MAX_ENTRIES + 1) as u8,
			))
			.expect_err("capacity overflow must fail");
		let after = program.account(&journal).expect("journal after rejection");
		assert_eq!(after.data, before.data);
		assert_eq!(after.lamports, before.lamports);

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn rejected_out_of_bounds_write_preserves_data() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");
		let authority = program.payer();
		let (journal, bump) = journal_pda(&program_id, &authority);
		program
			.send_instruction(initialize_instruction(
				&program, &authority, &journal, bump, 2,
			))
			.expect("initialize journal");
		let before = program.account(&journal).expect("journal before rejection");

		program
			.send_instruction(write_instruction(&program, &authority, &journal, 2, 99))
			.expect_err("index equal to length must fail");
		assert_eq!(
			program
				.account(&journal)
				.expect("journal after rejection")
				.data,
			before.data,
		);

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn foreign_signer_cannot_resize_another_authoritys_journal() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");
		let authority = program.payer();
		let (journal, bump) = journal_pda(&program_id, &authority);
		program
			.send_instruction(initialize_instruction(
				&program, &authority, &journal, bump, 2,
			))
			.expect("initialize journal");
		let before = program.account(&journal).expect("journal before attack");
		let attacker = Keypair::new();
		program
			.fund(&attacker.pubkey(), 1_000_000_000)
			.expect("fund attacker");

		program
			.send_with_signers(
				resize_instruction(&program, &attacker.pubkey(), &journal, 4),
				&[&attacker],
			)
			.expect_err("foreign signer must fail PDA validation");
		assert_eq!(
			program
				.account(&journal)
				.expect("journal after attack")
				.data,
			before.data,
		);

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn initialization_rejects_noncanonical_bumps_and_oversized_tails() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");
		let authority = program.payer();
		let (journal, bump) = journal_pda(&program_id, &authority);

		program
			.send_instruction(initialize_instruction(
				&program,
				&authority,
				&journal,
				bump.wrapping_add(1),
				0,
			))
			.expect_err("noncanonical bump must fail");
		program
			.send_instruction(initialize_instruction(
				&program,
				&authority,
				&journal,
				bump,
				(MAX_ENTRIES + 1) as u8,
			))
			.expect_err("oversized initial tail must fail");
		assert!(
			program.account(&journal).is_err(),
			"failed initialization must not create the PDA"
		);

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn signer_and_system_program_constraints_are_enforced() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");
		let unsigned_authority = Keypair::new().pubkey();
		program
			.fund(&unsigned_authority, 1_000_000_000)
			.expect("fund unsigned authority");
		let (journal, bump) = journal_pda(&program_id, &unsigned_authority);

		let unsigned = program.instruction(
			&[CompactInstruction::Initialize as u8, bump, 0],
			vec![
				AccountMeta::new(unsigned_authority, false),
				AccountMeta::new(journal, false),
				AccountMeta::new_readonly(Pubkey::default(), false),
			],
		);
		program
			.send_instruction(unsigned)
			.expect_err("authority must be a signer");

		let authority = program.payer();
		let (journal, bump) = journal_pda(&program_id, &authority);
		let wrong_system = program.instruction(
			&[CompactInstruction::Initialize as u8, bump, 0],
			vec![
				AccountMeta::new(authority, true),
				AccountMeta::new(journal, false),
				AccountMeta::new_readonly(program_id, false),
			],
		);
		program
			.send_instruction(wrong_system)
			.expect_err("system program address must be canonical");

		program.stop().expect("stop isolated program test");
	});
}
