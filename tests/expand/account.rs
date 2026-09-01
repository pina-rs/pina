use pina::*;

#[discriminator]
#[derive(Debug)]
pub enum AccountDisc {
	ConfigState = 1,
	GameState = 2,
	DataAccount = 3,
	BalanceAccount = 4,
	Custom = 5,
	LargeState = 6,
}

#[account(crate = pina, discriminator = AccountDisc)]
pub struct ConfigState {
	pub version: u8,
	pub bump: u8,
}

#[account(crate = pina, discriminator = AccountDisc)]
#[derive(Debug)]
pub struct GameState {
	pub score: u8,
	pub level: u8,
}

#[account(crate = pina, discriminator = AccountDisc)]
pub struct DataAccount {
	pub authority: [u8; 32],
	pub data: [u8; 64],
	pub flags: [u8; 4],
}

#[account(crate = pina, discriminator = AccountDisc)]
pub struct BalanceAccount {
	pub owner: [u8; 32],
	pub amount: PodU64,
	pub decimals: u8,
	pub is_frozen: PodBool,
}

#[account(crate = pina, discriminator = AccountDisc, variant = Custom)]
pub struct MyStruct {
	pub value: u8,
}

#[account(crate = pina, discriminator = AccountDisc)]
pub struct LargeState {
	pub authority: [u8; 32],
	pub bump: u8,
	pub treasury_bump: u8,
	pub mint_bump: u8,
	pub version: u8,
	pub padding: [u8; 3],
	pub total_supply: PodU64,
	pub name: [u8; 32],
}
