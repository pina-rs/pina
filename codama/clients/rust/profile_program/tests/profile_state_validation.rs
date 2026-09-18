use profile_program_client::generated::accounts::ProfileState;
use profile_program_client::generated::instructions::Initialize;
use profile_program_client::generated::instructions::InitializeInstructionData;

fn valid_profile_bytes() -> [u8; ProfileState::LEN] {
	// Envelope: discriminator, migration version, bump, then the payload
	// (`name` prefix at 3, `tags` prefix at 165, `active` at 240).
	let mut data = [0u8; ProfileState::LEN];
	data[0] = 1;
	data[1] = 0;
	data[2] = 42;
	data[3] = 1;
	data[4] = b'A';
	data[240] = 1;
	data
}

#[test]
fn profile_state_exposes_fully_initialized_bounded_fields() {
	let data = valid_profile_bytes();
	let profile =
		ProfileState::from_bytes(&data).unwrap_or_else(|error| panic!("parse failed: {error}"));

	assert_eq!(profile.name.as_str(), "A");
	assert_eq!(profile.bio.as_str(), "");
	assert!(profile.tags.is_empty());
	assert!(profile.favorite_tag.is_none());
}

#[test]
fn bounded_fields_reject_invalid_wire_values() {
	let mut invalid_name_length = valid_profile_bytes();
	invalid_name_length[3] = 33;
	assert!(ProfileState::from_bytes(&invalid_name_length).is_err());

	let mut invalid_name_utf8 = valid_profile_bytes();
	invalid_name_utf8[4] = 0xff;
	assert!(ProfileState::from_bytes(&invalid_name_utf8).is_err());

	let mut invalid_tags_length = valid_profile_bytes();
	invalid_tags_length[165] = 9;
	assert!(ProfileState::from_bytes(&invalid_tags_length).is_err());

	let mut invalid_option_tag = valid_profile_bytes();
	invalid_option_tag[231] = 2;
	assert!(ProfileState::from_bytes(&invalid_option_tag).is_err());
}

#[test]
fn profile_state_ignores_inactive_capacity_bytes() {
	let mut data = valid_profile_bytes();
	data[3] = 0;
	data[4..36].fill(0xff);
	data[36] = 0;
	data[37..165].fill(0xff);
	data[165] = 0;
	data[166] = 0;
	data[167..231].fill(0xff);
	data[232..240].fill(0xff);

	let profile =
		ProfileState::from_bytes(&data).unwrap_or_else(|error| panic!("parse failed: {error}"));
	assert_eq!(profile.name.as_str(), "");
	assert_eq!(profile.bio.as_str(), "");
	assert!(profile.tags.is_empty());
	assert!(data[4..36].iter().all(|byte| *byte == 0xff));
	assert!(data[37..165].iter().all(|byte| *byte == 0xff));
	assert!(data[167..231].iter().all(|byte| *byte == 0xff));
}

#[test]
fn profile_state_reads_some_option_value() {
	let mut data = valid_profile_bytes();
	data[231] = 1;
	data[232..240].copy_from_slice(&42u64.to_le_bytes());

	let profile =
		ProfileState::from_bytes(&data).unwrap_or_else(|error| panic!("parse failed: {error}"));
	assert_eq!(profile.favorite_tag.get().map(u64::from), Some(42));
}

#[test]
fn instruction_builder_owns_the_discriminator_and_derives_profile() {
	let data = InitializeInstructionData::new(|data| {
		data.discriminator = u8::MAX;
		data.bump = 42;
		data.name
			.try_set("A")
			.unwrap_or_else(|error| panic!("name should fit: {error}"));
	})
	.unwrap_or_else(|error| panic!("instruction data failed: {error}"));
	let authority = solana_pubkey::Pubkey::new_from_array([7; 32]);
	let expected_profile = solana_pubkey::Pubkey::find_program_address(
		&[b"profile", authority.as_ref()],
		&profile_program_client::programs::PROFILE_PROGRAM_ID,
	)
	.0;
	let instruction = Initialize::new(authority).instruction(data);

	assert_eq!(instruction.data[0], 0);
	assert_eq!(instruction.data[1], 0);
	assert_eq!(instruction.data[2], 42);
	assert_eq!(&instruction.data[3..5], &[1, b'A']);
	assert_eq!(instruction.accounts[1].pubkey, expected_profile);
}
