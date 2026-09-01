use pina::*;

const COUNTER_SEED: &[u8] = b"counter";

#[discriminator]
#[derive(Debug)]
pub enum PdaDisc {
	CounterState = 1,
	AllSeedState = 2,
	TodoState = 3,
}

#[account(crate = pina, discriminator = PdaDisc, variant = CounterState)]
#[pda(crate = pina, seeds = [COUNTER_SEED, authority: Address], bump = bump)]
pub struct CounterState {
	pub authority: Address,
	pub bump: u8,
}

#[account(crate = pina, discriminator = PdaDisc, variant = AllSeedState)]
#[pda(
	crate = ::pina,
	seeds = [
		b"test",
		authority: Address,
		amount: u64,
		side: u8,
		tag: [u8; 8],
		width: u16,
		height: u32,
	],
	bump = bump,
)]
pub struct AllSeedState {
	pub authority: Address,
	pub amount: PodU64,
	pub side: u8,
	pub tag: [u8; 8],
	pub width: PodU16,
	pub height: PodU32,
	pub bump: u8,
}

#[pda(crate = pina, seeds = [b"vault", user: Address])]
pub struct VaultState {
	pub user: Address,
}

#[account(crate = pina, discriminator = PdaDisc, variant = TodoState)]
#[pda(seeds = [b"todo", owner: Address], bump = bump)]
pub struct TodoState {
	pub owner: Address,
	pub bump: u8,
}

#[pda(crate = pina, seeds = [b"authority"])]
pub struct AuthorityState {}

#[pda(crate = pina, seeds = [b"numeric", nonce: u64, tag: [u8; 8]])]
pub struct NumericState {
	pub nonce: u64,
	pub tag: [u8; 8],
}
