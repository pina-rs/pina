//! INSECURE: Accepts non-canonical bump seeds.
//!
//! This program accepts any user-provided bump without verifying it is the
//! canonical bump for the PDA.

#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("DcTtzzD9CmafKGwKmTQPyTB1V9uydi7dN3sHSdkwvwif");

#[discriminator]
pub enum StoreInstruction {
	Create = 0,
}

#[discriminator]
pub enum StoreAccount {
	Data = 1,
}

#[account(discriminator = StoreAccount)]
pub struct Data {
	pub authority: Address,
	pub value: u64,
	pub bump: u8,
}

#[instruction(discriminator = StoreInstruction, variant = Create)]
pub struct CreateInstruction {
	pub value: u64,
	pub bump: u8,
}

const SEED: &[u8] = b"data";

#[derive(Accounts, Debug)]
pub struct CreateAccounts<'a> {
	pub authority: &'a AccountView,
	pub data: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for CreateAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = CreateInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		self.system_program.assert_address(&system::ID)?;
		self.data.assert_empty()?.assert_writable()?;

		// BUG: Uses assert_seeds_with_bump with user-provided bump.
		// An attacker can provide a non-canonical bump to create a
		// second valid PDA from the same seeds.
		let seeds_with_bump = &[
			SEED,
			self.authority.address().as_ref(),
			core::slice::from_ref(&args.bump),
		];
		self.data.assert_seeds_with_bump(seeds_with_bump, &ID)?;

		let seeds = &[SEED, self.authority.address().as_ref()];
		CreateProgramAccountWithBump {
			account: self.data,
			payer: self.authority,
			owner: &ID,
			seeds,
			bump: args.bump,
		}
		.invoke::<Data>()?;

		let mut data_account = self.data.as_account_mut::<Data>(&ID)?;
		data_account.authority = *self.authority.address();
		data_account.value = args.value;
		data_account.bump = args.bump;

		Ok(())
	}
}
