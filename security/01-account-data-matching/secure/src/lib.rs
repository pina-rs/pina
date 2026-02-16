//! SECURE: Account data matching enforced.
//!
//! This program verifies that accounts passed in the transaction match the
//! addresses stored in the on-chain escrow state.

#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("Ap7X7aFpm9sdxuzrcia2oD1WpjNHc4qdC8KaL3DFrzwe");

#[discriminator]
pub enum EscrowInstruction {
	Take = 0,
}

#[discriminator]
pub enum EscrowAccount {
	EscrowState = 1,
}

#[account(discriminator = EscrowAccount)]
pub struct EscrowState {
	pub maker: Address,
	pub amount: PodU64,
}

#[instruction(discriminator = EscrowInstruction, variant = Take)]
pub struct TakeInstruction {}

#[derive(Accounts, Debug)]
pub struct TakeAccounts<'a> {
	pub taker: &'a AccountView,
	pub maker: &'a AccountView,
	pub escrow: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for TakeAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let _ = TakeInstruction::try_from_bytes(data)?;

		self.taker.assert_signer()?;
		self.escrow
			.assert_not_empty()?
			.assert_type::<EscrowState>(&ID)?;

		let escrow = self.escrow.as_account::<EscrowState>(&ID)?;

		// SECURE: Verify the maker account matches the stored address.
		self.maker.assert_address(&escrow.maker)?;

		log!("Transfer to verified maker");

		Ok(())
	}
}
