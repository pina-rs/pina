#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::Keypair;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Signer;
use program_under_test::ID;
use program_under_test::ProfileInstruction;

/// On-chain `ProfileState` field offsets (after the 1-byte discriminator).
const BUMP_AT: usize = 1;
const NAME_AT: usize = 2;
const BIO_AT: usize = NAME_AT + 33;
const TAGS_AT: usize = BIO_AT + 129;
/// Option<u64> occupies a 1-byte tag plus an 8-byte slot on-chain.
const ACTIVE_AT: usize = TAGS_AT + 66 + 9;

/// Seed prefix for profile PDAs.
const PROFILE_SEED: &[u8] = b"profile";

fn pda_address(program_id: &Pubkey, authority: &Pubkey) -> (Pubkey, u8) {
	Pubkey::find_program_address(&[PROFILE_SEED, authority.as_ref()], program_id)
}

/// Bounded text: 1 length byte + UTF-8 payload.
fn bounded_text<const N: usize>(value: &str) -> [u8; N] {
	let mut bytes = [0u8; N];
	assert!(value.len() < N, "test text must fit");
	bytes[0] = value.len() as u8;
	bytes[1..1 + value.len()].copy_from_slice(value.as_bytes());

	bytes
}

fn initialize_instruction(
	program: &ProgramTest,
	authority: &Pubkey,
	profile: &Pubkey,
	bump: u8,
	name: &str,
	bio: &str,
) -> pina_test::Instruction {
	let mut data = vec![ProfileInstruction::Initialize as u8, bump];
	data.extend_from_slice(&bounded_text::<33>(name));
	data.extend_from_slice(&bounded_text::<129>(bio));

	program.instruction(
		&data,
		vec![
			AccountMeta::new(*authority, true),
			AccountMeta::new(*profile, false),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

/// `payload` excludes the discriminator byte; it is prepended here.
fn with_payload(
	program: &ProgramTest,
	authority: &Pubkey,
	profile: &Pubkey,
	kind: ProfileInstruction,
	payload: &[u8],
) -> pina_test::Instruction {
	let mut data = vec![kind as u8];
	data.extend_from_slice(payload);

	program.instruction(
		&data,
		vec![
			AccountMeta::new_readonly(*authority, true),
			AccountMeta::new(*profile, false),
		],
	)
}

fn assert_tags(account: &pina_test::Account, expected: &[u64]) {
	let tags = &account.data[TAGS_AT..ACTIVE_AT];
	let count = u16::from_le_bytes([tags[0], tags[1]]) as usize;
	assert_eq!(count, expected.len(), "on-chain tag count");

	for (index, expected_value) in expected.iter().enumerate() {
		let start = 2 + index * 8;
		let stored = u64::from_le_bytes(tags[start..start + 8].try_into().expect("tag slot"));
		assert_eq!(stored, *expected_value, "tag slot {index}");
	}
}

#[test]
#[ignore = "run with pina test"]
fn initialize_writes_name_bio_and_active_flag() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let (profile, bump) = pda_address(&program_id, &authority);

		program
			.send_instruction(initialize_instruction(
				&program,
				&authority,
				&profile,
				bump,
				"ifiok",
				"ships unique things",
			))
			.expect("execute Initialize");

		let account = program.account(&profile).expect("fetch profile account");
		assert_eq!(account.owner, program_id);
		assert_eq!(account.data[0], 1, "account discriminator is ProfileState");
		assert_eq!(account.data[BUMP_AT], bump);
		assert_eq!(
			&account.data[NAME_AT..NAME_AT + 33],
			&bounded_text::<33>("ifiok"),
			"name stored"
		);
		assert_eq!(
			&account.data[BIO_AT..BIO_AT + 129],
			&bounded_text::<129>("ships unique things"),
			"bio stored"
		);
		assert_eq!(account.data.len(), 240, "ProfileState layout");
		assert_eq!(
			&account.data[TAGS_AT..TAGS_AT + 2],
			0u16.to_le_bytes(),
			"no tags yet"
		);
		assert_eq!(account.data[ACTIVE_AT], 1, "profile starts active");

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn update_profile_replaces_name_and_bio() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let (profile, bump) = pda_address(&program_id, &authority);

		program
			.send_instruction(initialize_instruction(
				&program, &authority, &profile, bump, "before", "old bio",
			))
			.expect("execute Initialize");

		let mut payload = bounded_text::<33>("after").to_vec();
		payload.extend_from_slice(&bounded_text::<129>("new bio"));

		program
			.send_instruction(with_payload(
				&program,
				&authority,
				&profile,
				ProfileInstruction::UpdateProfile,
				&payload,
			))
			.expect("execute UpdateProfile");

		let account = program.account(&profile).expect("fetch profile account");
		assert_eq!(
			&account.data[NAME_AT..NAME_AT + 33],
			&bounded_text::<33>("after"),
			"name replaced"
		);
		assert_eq!(
			&account.data[BIO_AT..BIO_AT + 129],
			&bounded_text::<129>("new bio"),
			"bio replaced"
		);

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn tags_are_added_shifted_and_bounded() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let (profile, bump) = pda_address(&program_id, &authority);

		program
			.send_instruction(initialize_instruction(
				&program,
				&authority,
				&profile,
				bump,
				"tags",
				"checking the tag list",
			))
			.expect("execute Initialize");

		for tag in [10u64, 20, 30] {
			program
				.send_instruction(with_payload(
					&program,
					&authority,
					&profile,
					ProfileInstruction::AddTag,
					&tag.to_le_bytes(),
				))
				.expect("execute AddTag");
		}

		assert_tags(
			&program.account(&profile).expect("fetch profile"),
			&[10, 20, 30],
		);

		// RemoveTag at index 0 shifts the remaining tags forward.
		let remove = 0u64.to_le_bytes().to_vec();
		program
			.send_instruction(with_payload(
				&program,
				&authority,
				&profile,
				ProfileInstruction::RemoveTag,
				&remove,
			))
			.expect("execute RemoveTag");
		assert_tags(
			&program.account(&profile).expect("fetch profile"),
			&[20, 30],
		);

		// Out-of-range removal must fail with the custom TagNotFound error.
		let error = program
			.send_instruction(with_payload(
				&program,
				&authority,
				&profile,
				ProfileInstruction::RemoveTag,
				&9u64.to_le_bytes(),
			))
			.expect_err("index 9 exceeds the tag count");
		assert_eq!(error.operation(), "execute program instruction");
		eprintln!("tag-not-found error: {}", error.message());

		// Filling the list to capacity must reject the next tag.
		for tag in [40u64, 50, 60, 70, 80, 90] {
			program
				.send_instruction(with_payload(
					&program,
					&authority,
					&profile,
					ProfileInstruction::AddTag,
					&tag.to_le_bytes(),
				))
				.expect("fill tags to capacity");
		}

		let error = program
			.send_instruction(with_payload(
				&program,
				&authority,
				&profile,
				ProfileInstruction::AddTag,
				&100u64.to_le_bytes(),
			))
			.expect_err("the ninth tag overflows capacity");
		assert_eq!(error.operation(), "execute program instruction");
		eprintln!("tag overflow error: {}", error.message());

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn a_stranger_cannot_touch_someone_elses_profile() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let (profile, bump) = pda_address(&program_id, &authority);

		program
			.send_instruction(initialize_instruction(
				&program,
				&authority,
				&profile,
				bump,
				"owner",
				"owner bio",
			))
			.expect("execute Initialize");

		let stranger = Keypair::new();
		program
			.fund(&stranger.pubkey(), 1_000_000_000)
			.expect("fund stranger");

		let instruction = with_payload(
			&program,
			&stranger.pubkey(),
			&profile,
			ProfileInstruction::AddTag,
			&1u64.to_le_bytes(),
		);
		let error = program
			.send_with_signers(instruction, &[&stranger])
			.expect_err("the signer must match the stored authority");
		assert_eq!(error.operation(), "execute program instruction");
		eprintln!("stranger profile error: {}", error.message());

		let account = program.account(&profile).expect("fetch profile account");
		assert_eq!(
			&account.data[TAGS_AT..TAGS_AT + 2],
			0u16.to_le_bytes(),
			"tags remain empty"
		);

		program.stop().expect("stop isolated program test");
	});
}
