//! INSECURE: Missing account data matching.
//!
//! This program reads escrow state but doesn't verify that the accounts passed
//! in the transaction match the addresses stored in the escrow.

#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("61UXsiatjgsDxdViLPJqLR2mybnnHhuEywKJ22c22gQV");

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

		let _escrow = self.escrow.as_account::<EscrowState>(&ID)?;

		// BUG: No check that self.maker matches escrow.maker!
		// An attacker can pass their own address as `maker` and
		// receive the funds instead of the real maker.

		log!("Transfer to maker (unverified)");

		Ok(())
	}
}
