use pina::*;

#[discriminator]
pub enum TestAccountType {
	TestAccount = 0,
}

/// Seed prefix for test PDAs.
const TEST_SEED: &[u8] = b"test";

#[account(discriminator = TestAccountType)]
#[pda(seeds = [TEST_SEED, authority: Address, amount: u64, side: u8, tag: [u8; 8]], bump = bump)]
pub struct TestAccount {
	pub authority: Address,
	pub amount: PodU64,
	pub side: u8,
	pub tag: [u8; 8],
	pub bump: u8,
}

fn main() {
	let authority = Address::new_from_array([1u8; 32]);
	let seeds = TestAccount::seeds(&authority, 42, 1, [0u8; 8]);
	let _ = seeds.as_slices();
	let _ = seeds.with_bump(1).as_slices();
	let _ = TestAccount::try_find_pda(&authority, 42, 1, [0u8; 8], &authority);
}
