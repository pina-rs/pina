use crate::IntoDiscriminator;
use crate::Vec;

#[crate::discriminator(crate = crate)]
pub enum TestCompactKind {
	TestCompactState = 9,
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
