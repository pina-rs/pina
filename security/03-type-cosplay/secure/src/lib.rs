//! SECURE: Discriminator check prevents type cosplay.
//!
//! This program validates the discriminator before deserialization, preventing
//! one account type from being used where another is expected.

#![no_std]

#[cfg(all(not(any(target_os = "solana", target_arch = "bpf")), not(test)))]
extern crate std;

use pina::*;

declare_id!("3D1LXPeY2R6TZzEP5TVwaTYLwZ1XWxGw2MaWYC9UhbJ8");

#[discriminator]
pub enum AppInstruction {
	AdminAction = 0,
}

#[discriminator]
pub enum AppAccount {
	UserProfile = 1,
	AdminConfig = 2,
}

#[account(discriminator = AppAccount)]
pub struct UserProfile {
	pub authority: Address,
	pub points: PodU64,
}

#[account(discriminator = AppAccount)]
pub struct AdminConfig {
	pub authority: Address,
	pub fee: PodU64,
}

#[instruction(discriminator = AppInstruction, variant = AdminAction)]
pub struct AdminActionInstruction {
	pub new_fee: PodU64,
}

#[derive(Accounts, Debug)]
pub struct AdminActionAccounts<'a> {
	pub authority: &'a AccountView,
	pub config: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for AdminActionAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = AdminActionInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;

		// SECURE: assert_type checks discriminator + owner + size.
		// A UserProfile account will be rejected because its discriminator
		// (1) doesn't match AdminConfig's discriminator (2).
		self.config.assert_type::<AdminConfig>(&ID)?;

		let config = self.config.as_account::<AdminConfig>(&ID)?;
		self.authority.assert_address(&config.authority)?;

		let config_mut = self.config.as_account_mut::<AdminConfig>(&ID)?;
		config_mut.fee = args.new_fee;

		Ok(())
	}
}
