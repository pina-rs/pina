//! SECURE: Initialization guard prevents reinitialization.
//!
//! This program calls `assert_empty()` before creating accounts, preventing
//! reinitialization of existing accounts.

#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("7ytRhSjEj3Ex8KS2bs6ftN4BxHUCpug7jmqsdLmUFbFg");

#[discriminator]
pub enum ConfigInstruction {
	Initialize = 0,
}

#[discriminator]
pub enum ConfigAccount {
	Config = 1,
}

#[account(discriminator = ConfigAccount)]
pub struct Config {
	pub authority: Address,
	pub value: PodU64,
	pub bump: u8,
}

#[instruction(discriminator = ConfigInstruction, variant = Initialize)]
pub struct InitializeInstruction {
	pub value: PodU64,
	pub bump: u8,
}

const SEED: &[u8] = b"config";

#[derive(Accounts, Debug)]
pub struct InitializeAccounts<'a> {
	pub authority: &'a AccountView,
	pub config: &'a AccountView,
	pub system_program: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = InitializeInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		self.system_program.assert_address(&system::ID)?;

		// SECURE: Verify the account is empty before initialization.
		// Returns AccountAlreadyInitialized if the account has data.
		self.config.assert_empty()?.assert_writable()?;

		let seeds = &[SEED, self.authority.address().as_ref()];
		create_program_account_with_bump::<Config>(
			self.config,
			self.authority,
			&ID,
			seeds,
			args.bump,
		)?;

		let config = self.config.as_account_mut::<Config>(&ID)?;
		*config = Config::builder()
			.authority(*self.authority.address())
			.value(args.value)
			.bump(args.bump)
			.build();

		Ok(())
	}
}
