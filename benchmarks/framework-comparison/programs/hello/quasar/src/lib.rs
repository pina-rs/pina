//! Quasar half of the hello comparison.
#![no_std]

use quasar_lang::prelude::*;

declare_id!("DCF5KBmtQ9ryDC7mQezKLwuJHem6coVUCmKkw37M9J4A");

#[program]
mod hello_quasar {
	use super::*;

	#[instruction(discriminator = 0)]
	pub fn hello(ctx: Ctx<Hello>) -> Result<(), ProgramError> {
		ctx.accounts.handler()
	}
}

#[derive(Accounts)]
pub struct Hello {
	pub user: Signer,
}

impl Hello {
	#[inline(always)]
	pub fn handler(&self) -> Result<(), ProgramError> {
		log!("Hello, Solana!");
		Ok(())
	}
}
