//! Kani proofs for macro-generated discriminators and fixed zero-copy storage.

use crate::*;

#[pina_macros::discriminator(crate = crate, primitive = u8, final)]
enum ProofKind {
	State = 3,
	Alternate = 7,
}

#[pina_macros::account(crate = crate, discriminator = ProofKind, variant = State)]
struct ProofState {
	pub enabled: bool,
	pub maybe_enabled: Option<bool>,
	pub maybe_count: Option<u16>,
}

const ENABLED_OFFSET: usize = 1;
const MAYBE_ENABLED_TAG_OFFSET: usize = 2;
const MAYBE_ENABLED_VALUE_OFFSET: usize = 3;
const MAYBE_COUNT_TAG_OFFSET: usize = 4;

fn bytes_are_valid(bytes: &[u8; ProofState::SIZE]) -> bool {
	bytes[0] == ProofKind::State as u8
		&& bytes[ENABLED_OFFSET] <= 1
		&& bytes[MAYBE_ENABLED_TAG_OFFSET] <= 1
		&& (bytes[MAYBE_ENABLED_TAG_OFFSET] == 0 || bytes[MAYBE_ENABLED_VALUE_OFFSET] <= 1)
		&& bytes[MAYBE_COUNT_TAG_OFFSET] <= 1
}

#[kani::proof]
fn quick_arbitrary_fixed_account_bytes_are_rejected_or_safely_borrowed() {
	let bytes: [u8; ProofState::SIZE] = kani::any();
	let result = ProofState::try_from_bytes(&bytes);

	assert_eq!(result.is_ok(), bytes_are_valid(&bytes));

	if let Ok(state) = result {
		assert_eq!(state.enabled.get(), bytes[ENABLED_OFFSET] == 1);
		assert_eq!(
			state.maybe_enabled.get().map(|value| value.get()),
			match bytes[MAYBE_ENABLED_TAG_OFFSET] {
				0 => None,
				1 => Some(bytes[MAYBE_ENABLED_VALUE_OFFSET] == 1),
				_ => unreachable!(),
			}
		);
	}
}

#[kani::proof]
fn quick_every_wrong_fixed_account_length_is_rejected() {
	let bytes: [u8; ProofState::SIZE + 1] = kani::any();
	let len: usize = kani::any();
	kani::assume(len <= bytes.len());
	let result = ProofState::try_from_bytes(&bytes[..len]);

	if len != ProofState::SIZE {
		assert!(result.is_err());
	}
}

#[kani::proof]
fn quick_failed_mutable_validation_leaves_bytes_unchanged() {
	let mut bytes: [u8; ProofState::SIZE] = kani::any();
	let before = bytes;
	let result = ProofState::try_from_bytes_mut(&mut bytes);

	if result.is_err() {
		assert_eq!(bytes, before);
	}
}

#[kani::proof]
fn quick_discriminator_enum_accepts_exactly_declared_tags() {
	let tag: u8 = kani::any();
	let bytes = [tag];
	let result = ProofKind::discriminator_from_bytes(&bytes);

	assert_eq!(result.is_ok(), tag == 3 || tag == 7);
}

#[kani::proof]
fn quick_initialized_fixed_accounts_are_canonical() {
	let mut bytes: [u8; ProofState::SIZE] = kani::any();
	let result = ProofState::initialize(&mut bytes, |_| Ok(()));

	assert!(result.is_ok());
	assert!(bytes_are_valid(&bytes));
	assert_eq!(bytes[0], ProofKind::State as u8);
	assert!(bytes[1..].iter().all(|byte| *byte == 0));
}
