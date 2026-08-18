use profile_program_client::generated::accounts::ProfileState;
use profile_program_client::generated::instructions::Initialize;
use profile_program_client::generated::instructions::InitializeInstructionData;

const PROFILE_LEN: usize = 240;

fn valid_profile_bytes() -> [u8; PROFILE_LEN] {
	let mut data = [0u8; PROFILE_LEN];
	data[0] = 1;
	data[1] = 42;
	data[2] = 1;
	data[3] = b'A';
	data[239] = 1;
	data
}

#[test]
fn profile_state_validates_semantic_pod_collections() {
	let data = valid_profile_bytes();
	let profile =
		ProfileState::from_bytes(&data).unwrap_or_else(|error| panic!("parse failed: {error}"));

	assert_eq!(profile.name.as_str(), "A");
	assert_eq!(profile.bio.as_str(), "");
	assert!(profile.tags.is_empty());
	assert!(profile.favorite_tag.is_none());
}

#[test]
fn profile_state_rejects_invalid_collection_data() {
	let mut invalid_name_length = valid_profile_bytes();
	invalid_name_length[2] = 33;
	assert!(ProfileState::from_bytes(&invalid_name_length).is_err());

	let mut invalid_name_utf8 = valid_profile_bytes();
	invalid_name_utf8[3] = 0xff;
	assert!(ProfileState::from_bytes(&invalid_name_utf8).is_err());

	let mut invalid_tags_length = valid_profile_bytes();
	invalid_tags_length[164] = 9;
	assert!(ProfileState::from_bytes(&invalid_tags_length).is_err());

	let mut invalid_option_tag = valid_profile_bytes();
	invalid_option_tag[230] = 2;
	assert!(ProfileState::from_bytes(&invalid_option_tag).is_err());
}

#[test]
fn profile_state_ignores_inactive_collection_capacity() {
	let mut data = valid_profile_bytes();
	data[2] = 0;
	data[3..35].fill(0xff);
	data[35] = 0;
	data[36..164].fill(0xff);
	data[164] = 0;
	data[165] = 0;
	data[166..230].fill(0xff);
	data[231..239].fill(0xff);

	assert!(ProfileState::from_bytes(&data).is_ok());
}

#[test]
fn profile_state_reads_some_option_value() {
	let mut data = valid_profile_bytes();
	data[230] = 1;
	data[231..239].copy_from_slice(&42u64.to_le_bytes());

	let profile =
		ProfileState::from_bytes(&data).unwrap_or_else(|error| panic!("parse failed: {error}"));
	assert_eq!(profile.favorite_tag.get().map(u64::from), Some(42));
}

#[test]
fn instruction_builder_owns_the_discriminator() {
	let data = InitializeInstructionData::new(|data| {
		data.discriminator = u8::MAX;
		data.bump = 42;
	})
	.unwrap_or_else(|error| panic!("instruction data failed: {error}"));
	let instruction =
		Initialize::new(solana_pubkey::Pubkey::new_from_array([7; 32])).instruction(data);

	assert_eq!(instruction.data[0], 0);
	assert_eq!(instruction.data[1], 42);
}
