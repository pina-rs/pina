//! SECURE: Canonical bump seed enforced.
//!
//! This program uses `assert_seeds()` which finds the canonical bump
//! automatically, preventing non-canonical PDA creation.

#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("FfSMbugrnQhTpzXuEqDxYcXzmB24ZijXqQ1mZUdKXeXn");

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
	pub value: PodU64,
	pub bump: u8,
}

#[instruction(discriminator = StoreInstruction, variant = Create)]
pub struct CreateInstruction {
	pub value: PodU64,
}

const SEED: &[u8] = b"data";

#[derive(Accounts, Debug)]
pub struct CreateAccounts<'a> {
	pub authority: &'a AccountView,
	pub data: &'a AccountView,
	pub system_program: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for CreateAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = CreateInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		self.system_program.assert_address(&system::ID)?;
		self.data.assert_empty()?.assert_writable()?;

		// SECURE: assert_seeds finds the canonical bump and verifies the address.
		// Only the canonical PDA will be accepted.
		let seeds = &[SEED, self.authority.address().as_ref()];
		self.data.assert_seeds(seeds, &ID)?;

		// create_program_account also finds the canonical bump internally.
		let (_address, bump) =
			create_program_account::<Data>(self.data, self.authority, &ID, seeds)?;

		let data_account = self.data.as_account_mut::<Data>(&ID)?;
		*data_account = Data::builder()
			.authority(*self.authority.address())
			.value(args.value)
			.bump(bump)
			.build();

		Ok(())
	}
}
