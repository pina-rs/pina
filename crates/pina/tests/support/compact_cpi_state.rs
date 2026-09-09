use crate::IntoDiscriminator;
use crate::Vec;

#[crate::discriminator(crate = crate)]
pub enum TestCompactKind {
	TestCompactState = 9,
	ValidatedTestCompactState = 10,
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
