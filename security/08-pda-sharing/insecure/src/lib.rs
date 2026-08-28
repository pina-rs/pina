//! INSECURE: Shared PDA seeds across account types.
//!
//! Both account types use the same seed prefix, allowing PDA collisions.

#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("FWPy92oq7ngP8ask397rjmjgAughKTW71adbXePmHhTr");

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
	pub setting: u64,
	pub bump: u8,
}

#[account(discriminator = AppAccount)]
pub struct UserVault {
	pub authority: Address,
	pub balance: u64,
	pub bump: u8,
}

#[instruction(discriminator = AppInstruction, variant = CreateConfig)]
pub struct CreateConfigInstruction {
	pub setting: u64,
}

#[instruction(discriminator = AppInstruction, variant = CreateVault)]
pub struct CreateVaultInstruction {}

// BUG: Both account types use the same seed prefix "state".
// This means UserConfig and UserVault for the same user derive
// to the same PDA address.
const SEED: &[u8] = b"state";

#[derive(Accounts, Debug)]
pub struct CreateConfigAccounts<'a> {
	pub authority: &'a AccountView,
	pub config: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for CreateConfigAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = CreateConfigInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		self.system_program.assert_address(&system::ID)?;
		self.config.assert_empty()?.assert_writable()?;

		let seeds = &[SEED, self.authority.address().as_ref()];
		let (_address, bump) = CreateProgramAccount {
			account: self.config,
			payer: self.authority,
			owner: &ID,
			seeds,
		}
		.invoke::<UserConfig>()?;

		let mut config = self.config.as_account_mut::<UserConfig>(&ID)?;
		config.authority = *self.authority.address();
		config.setting = args.setting;
		config.bump = bump;

		Ok(())
	}
}
