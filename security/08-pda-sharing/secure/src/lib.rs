//! SECURE: Namespaced PDA seeds prevent sharing.
//!
//! Each account type uses a unique seed prefix, ensuring distinct PDA
//! address spaces.

#![allow(dead_code)]
#![no_std]

#[cfg(all(not(any(target_os = "solana", target_arch = "bpf")), not(test)))]
extern crate std;

use pina::*;

declare_id!("9eeUTrtMUGT4rJx3QPKYRqTnpr9wJpaeUMUJkM8wQmxH");

#[discriminator]
pub enum AppInstruction {
	CreateConfig = 0,
	CreateVault = 1,
}

#[discriminator]
pub enum AppAccount {
	UserConfig = 1,
	UserVault = 2,
}

#[account(discriminator = AppAccount)]
pub struct UserConfig {
	pub authority: Address,
	pub setting: PodU64,
	pub bump: u8,
}

#[account(discriminator = AppAccount)]
pub struct UserVault {
	pub authority: Address,
	pub balance: PodU64,
	pub bump: u8,
}

#[instruction(discriminator = AppInstruction, variant = CreateConfig)]
pub struct CreateConfigInstruction {
	pub setting: PodU64,
}

#[instruction(discriminator = AppInstruction, variant = CreateVault)]
pub struct CreateVaultInstruction {}

// SECURE: Each account type has its own seed prefix.
const CONFIG_SEED: &[u8] = b"config";
const VAULT_SEED: &[u8] = b"vault";

#[derive(Accounts, Debug)]
pub struct CreateConfigAccounts<'a> {
	pub authority: &'a AccountView,
	pub config: &'a AccountView,
	pub system_program: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for CreateConfigAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = CreateConfigInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		self.system_program.assert_address(&system::ID)?;
		self.config.assert_empty()?.assert_writable()?;

		// SECURE: Uses type-specific seed prefix "config".
		let seeds = &[CONFIG_SEED, self.authority.address().as_ref()];
		self.config.assert_seeds(seeds, &ID)?;

		let (_address, bump) =
			create_program_account::<UserConfig>(self.config, self.authority, &ID, seeds)?;

		let config = self.config.as_account_mut::<UserConfig>(&ID)?;
		*config = UserConfig::builder()
			.authority(*self.authority.address())
			.setting(args.setting)
			.bump(bump)
			.build();

		Ok(())
	}
}
