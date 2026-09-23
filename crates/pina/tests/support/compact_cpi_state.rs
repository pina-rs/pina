use crate::IntoDiscriminator;
use crate::Vec;

#[crate::discriminator(crate = crate)]
pub enum TestCompactKind {
	TestCompactState = 9,
	ValidatedTestCompactState = 10,
	TestCompactBumpState = 11,
}

#[crate::account(
	crate = crate,
	discriminator = TestCompactKind::TestCompactState,
	compact
)]
#[allow(dead_code)]
pub struct TestCompactState {
	pub value: u8,
	pub items: Vec<u64, 4>,
}

#[crate::account(
	crate = crate,
	discriminator = TestCompactKind::TestCompactBumpState,
	compact
)]
#[crate::pda(crate = crate, seeds = [SEED_TEST_COMPACT_BUMP], bump = bump)]
#[allow(dead_code)]
pub struct TestCompactBumpState {
	pub value: u8,
	pub bump: u8,
	pub items: Vec<u64, 2>,
}

/// Seed prefix for [`TestCompactBumpState`] PDAs.
pub const SEED_TEST_COMPACT_BUMP: &[u8] = b"test-compact-bump";

#[cfg(feature = "validation")]
#[crate::account(
	crate = crate,
	discriminator = TestCompactKind::ValidatedTestCompactState,
	compact,
	validate(with = validate_test_compact_state)
)]
pub struct ValidatedTestCompactState {
	pub value: u8,
	pub items: Vec<u8, 1>,
}

#[cfg(feature = "validation")]
fn validate_test_compact_state(value: &ValidatedTestCompactStateRef<'_>) -> crate::ProgramResult {
	if value.value == 7 {
		return Err(crate::ProgramError::Custom(71));
	}

	Ok(())
}
