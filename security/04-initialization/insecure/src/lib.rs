//! INSECURE: Missing initialization guard.
//!
//! This program creates an account without checking if it's already
//! initialized, allowing reinitialization attacks.

#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("2xJkdLvVicH1SLCp9hAHGLngGY6kZHWBr656U4sRDRvy");

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

		// BUG: No `assert_empty()` check!
		// If the account already exists, this will overwrite its state,
		// potentially changing the authority to the attacker's address.
		self.config.assert_writable()?;

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
