#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::Keypair;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Rent;
use pina_test::Signer;
use program_under_test::CompactInstruction;
use program_under_test::DEFAULT_TITLE;
use program_under_test::ID;
use program_under_test::Journal;

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
	marker_count: u8,
) -> pina_test::Instruction {
	program.instruction(
		&[
			CompactInstruction::Initialize as u8,
			bump,
			entry_count,
			marker_count,
		],
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
	marker_count: u8,
) -> pina_test::Instruction {
	program.instruction(
		&[CompactInstruction::Resize as u8, entry_count, marker_count],
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

fn rename_instruction(
	program: &ProgramTest,
	authority: &Pubkey,
	journal: &Pubkey,
	title: &str,
) -> pina_test::Instruction {
	let mut data = vec![CompactInstruction::Rename as u8, title.len() as u8];
	let mut title_bytes = [0; Journal::TITLE_CAPACITY];
	title_bytes[..title.len()].copy_from_slice(title.as_bytes());
	data.extend_from_slice(&title_bytes);
	program.instruction(
		&data,
		vec![
			AccountMeta::new(*authority, true),
			AccountMeta::new(*journal, false),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

struct ExpectedJournal<'a> {
	revision: u32,
	featured_entry: Option<u64>,
	title: &'a str,
	entries: &'a [u64],
	markers: &'a [u8],
}

fn assert_journal(
	program: &ProgramTest,
	journal: &Pubkey,
	authority: &Pubkey,
	expected: ExpectedJournal<'_>,
) {
	let account = program.account(journal).expect("fetch journal account");
	let journal = Journal::try_from_bytes(&account.data).expect("decode journal");
	let expected_size = journal.encoded_len();
	assert_eq!(account.owner, program.program_id());
	assert_eq!(account.data.len(), expected_size);
	assert_eq!(
		account.lamports,
		Rent::default().minimum_balance(expected_size),
		"compact realloc keeps exactly the rent-exempt minimum",
	);
	assert_eq!(journal.authority.to_bytes(), authority.to_bytes());
	assert_eq!(journal.revision.get(), expected.revision);
	assert_eq!(
		journal.featured_entry.get().map(|value| value.get()),
		expected.featured_entry,
	);
	assert_eq!(journal.title(), expected.title);
	assert_eq!(
		journal
			.entries()
			.iter()
			.map(|entry| entry.get())
			.collect::<Vec<_>>(),
		expected.entries,
	);
	assert_eq!(journal.markers(), expected.markers);
	assert_eq!(journal.encoded_len(), expected_size);
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
				&program, &authority, &empty, empty_bump, 0, 0,
			))
			.expect("initialize header-only journal");
		assert_journal(
			&program,
			&empty,
			&authority,
			ExpectedJournal {
				revision: 0,
				featured_entry: None,
				title: DEFAULT_TITLE,
				entries: &[],
				markers: &[],
			},
		);

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
					5,
				),
				&[&second_authority],
			)
			.expect("initialize nonempty journal");
		assert_journal(
			&program,
			&nonempty,
			&second_authority.pubkey(),
			ExpectedJournal {
				revision: 0,
				featured_entry: None,
				title: DEFAULT_TITLE,
				entries: &[0, 1, 2],
				markers: &[0, 1, 2, 3, 4],
			},
		);

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn grows_and_shrinks_a_compact_pod_string_with_exact_rent() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");
		let authority = program.payer();
		let (journal, bump) = journal_pda(&program_id, &authority);
		program
			.send_instruction(initialize_instruction(
				&program, &authority, &journal, bump, 2, 3,
			))
			.expect("initialize journal");

		program
			.send_instruction(rename_instruction(
				&program,
				&authority,
				&journal,
				"compact journal",
			))
			.expect("grow title");
		let balance_after_growth = program.balance(&authority).expect("balance after growth");
		assert_journal(
			&program,
			&journal,
			&authority,
			ExpectedJournal {
				revision: 1,
				featured_entry: None,
				title: "compact journal",
				entries: &[0, 1],
				markers: &[0, 1, 2],
			},
		);

		program
			.send_instruction(rename_instruction(&program, &authority, &journal, "pina"))
			.expect("shrink title");
		let balance_after_shrink = program.balance(&authority).expect("balance after shrink");
		assert!(
			balance_after_shrink > balance_after_growth,
			"the title shrink refunds more rent than the transaction fee",
		);
		assert_journal(
			&program,
			&journal,
			&authority,
			ExpectedJournal {
				revision: 2,
				featured_entry: None,
				title: "pina",
				entries: &[0, 1],
				markers: &[0, 1, 2],
			},
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
				&program, &authority, &journal, bump, 2, 4,
			))
			.expect("initialize journal");

		program
			.send_instruction(resize_instruction(
				&program,
				&authority,
				&journal,
				Journal::ENTRIES_CAPACITY as u8,
				6,
			))
			.expect("grow to full capacity");
		let balance_after_growth = program.balance(&authority).expect("balance after growth");
		assert_journal(
			&program,
			&journal,
			&authority,
			ExpectedJournal {
				revision: 1,
				featured_entry: None,
				title: DEFAULT_TITLE,
				entries: &[0, 1, 2, 3, 4, 5, 6, 7],
				markers: &[0, 1, 2, 3, 4, 5],
			},
		);

		program
			.send_instruction(resize_instruction(
				&program,
				&authority,
				&journal,
				Journal::ENTRIES_CAPACITY as u8,
				6,
			))
			.expect("same-size resize");
		assert_journal(
			&program,
			&journal,
			&authority,
			ExpectedJournal {
				revision: 2,
				featured_entry: None,
				title: DEFAULT_TITLE,
				entries: &[0, 1, 2, 3, 4, 5, 6, 7],
				markers: &[0, 1, 2, 3, 4, 5],
			},
		);

		program
			.send_instruction(write_instruction(&program, &authority, &journal, 3, 99))
			.expect("write without resizing");
		assert_journal(
			&program,
			&journal,
			&authority,
			ExpectedJournal {
				revision: 3,
				featured_entry: Some(99),
				title: DEFAULT_TITLE,
				entries: &[0, 1, 2, 99, 4, 5, 6, 7],
				markers: &[0, 1, 2, 3, 4, 5],
			},
		);

		program
			.send_instruction(resize_instruction(&program, &authority, &journal, 3, 8))
			.expect("shrink entries while growing markers");
		let balance_after_shrink = program.balance(&authority).expect("balance after shrink");
		assert!(
			balance_after_shrink > balance_after_growth,
			"rent refund from removing five entries exceeds the transaction fee",
		);
		assert_journal(
			&program,
			&journal,
			&authority,
			ExpectedJournal {
				revision: 4,
				featured_entry: Some(99),
				title: DEFAULT_TITLE,
				entries: &[0, 1, 2],
				markers: &[0, 1, 2, 3, 4, 5, 6, 7],
			},
		);

		program
			.send_instruction(resize_instruction(&program, &authority, &journal, 5, 2))
			.expect("grow entries while shrinking markers");
		assert_journal(
			&program,
			&journal,
			&authority,
			ExpectedJournal {
				revision: 5,
				featured_entry: Some(99),
				title: DEFAULT_TITLE,
				entries: &[0, 1, 2, 3, 4],
				markers: &[0, 1],
			},
		);

		program
			.send_instruction(resize_instruction(&program, &authority, &journal, 0, 2))
			.expect("clear entries while preserving markers");
		assert_journal(
			&program,
			&journal,
			&authority,
			ExpectedJournal {
				revision: 6,
				featured_entry: Some(99),
				title: DEFAULT_TITLE,
				entries: &[],
				markers: &[0, 1],
			},
		);

		program
			.send_instruction(resize_instruction(&program, &authority, &journal, 0, 0))
			.expect("clear vector tails while preserving the title");
		assert_journal(
			&program,
			&journal,
			&authority,
			ExpectedJournal {
				revision: 7,
				featured_entry: Some(99),
				title: DEFAULT_TITLE,
				entries: &[],
				markers: &[],
			},
		);

		program
			.send_instruction(rename_instruction(&program, &authority, &journal, ""))
			.expect("clear the final string tail to header-only");
		assert_journal(
			&program,
			&journal,
			&authority,
			ExpectedJournal {
				revision: 8,
				featured_entry: Some(99),
				title: "",
				entries: &[],
				markers: &[],
			},
		);

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
				&program, &authority, &journal, bump, 2, 3,
			))
			.expect("initialize journal");
		let before = program.account(&journal).expect("journal before rejection");

		program
			.send_instruction(resize_instruction(
				&program,
				&authority,
				&journal,
				(Journal::ENTRIES_CAPACITY + 1) as u8,
				0,
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
				&program, &authority, &journal, bump, 2, 3,
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
fn rejected_invalid_titles_preserve_data_and_lamports() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");
		let authority = program.payer();
		let (journal, bump) = journal_pda(&program_id, &authority);
		program
			.send_instruction(initialize_instruction(
				&program, &authority, &journal, bump, 2, 3,
			))
			.expect("initialize journal");
		let before = program.account(&journal).expect("journal before rejection");

		for (title_len, first_byte) in [((Journal::TITLE_CAPACITY + 1) as u8, b'x'), (1, 0xff)] {
			let mut data = vec![CompactInstruction::Rename as u8, title_len];
			let mut title = [0; Journal::TITLE_CAPACITY];
			title[0] = first_byte;
			data.extend_from_slice(&title);
			let instruction = program.instruction(
				&data,
				vec![
					AccountMeta::new(authority, true),
					AccountMeta::new(journal, false),
					AccountMeta::new_readonly(Pubkey::default(), false),
				],
			);
			program
				.send_instruction(instruction)
				.expect_err("invalid title must fail");
		}

		let after = program.account(&journal).expect("journal after rejection");
		assert_eq!(after.data, before.data);
		assert_eq!(after.lamports, before.lamports);

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
				&program, &authority, &journal, bump, 2, 3,
			))
			.expect("initialize journal");
		let before = program.account(&journal).expect("journal before attack");
		let attacker = Keypair::new();
		program
			.fund(&attacker.pubkey(), 1_000_000_000)
			.expect("fund attacker");

		program
			.send_with_signers(
				resize_instruction(&program, &attacker.pubkey(), &journal, 4, 5),
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
				0,
			))
			.expect_err("noncanonical bump must fail");
		program
			.send_instruction(initialize_instruction(
				&program,
				&authority,
				&journal,
				bump,
				(Journal::ENTRIES_CAPACITY + 1) as u8,
				0,
			))
			.expect_err("oversized initial entry tail must fail");
		program
			.send_instruction(initialize_instruction(
				&program,
				&authority,
				&journal,
				bump,
				0,
				(Journal::MARKERS_CAPACITY + 1) as u8,
			))
			.expect_err("oversized initial marker tail must fail");
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
			&[CompactInstruction::Initialize as u8, bump, 0, 0],
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
			&[CompactInstruction::Initialize as u8, bump, 0, 0],
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
