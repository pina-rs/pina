//! INSECURE: Missing discriminator check (type cosplay).
//!
//! This program deserializes account data without checking the discriminator,
//! allowing one account type to be used where another is expected.

#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("7B26CfS1bHscau2L7dYzTzyGf2hGtdkL1agmh5EjEnJm");

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
		self.config.assert_owner(&ID)?;

		// BUG: No discriminator check! Uses raw bytemuck cast.
		// An attacker can pass a UserProfile account (same size) as AdminConfig.
		// Since UserProfile.authority == attacker's address, the authority check
		// passes and they gain admin access.
		let data = self.config.try_borrow()?;
		let config: &AdminConfig =
			bytemuck::try_from_bytes(&data).or(Err(ProgramError::InvalidAccountData))?;

		self.authority.assert_address(&config.authority)?;

		let config_mut = self.config.as_account_mut::<AdminConfig>(&ID)?;
		config_mut.fee = args.new_fee;

		Ok(())
	}
}
