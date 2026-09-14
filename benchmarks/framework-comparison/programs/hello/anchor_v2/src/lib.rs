//! Anchor v2 (`lang-v2`) half of the hello comparison.
use anchor_lang::prelude::*;

declare_id!("DCF5KBmtQ9ryDC7mQezKLwuJHem6coVUCmKkw37M9J4A");

#[program]
mod hello_anchor_v2 {
	use super::*;

	pub fn hello(ctx: &mut Context<Hello>) -> Result<()> {
		ctx.accounts.handler()
	}
}

#[derive(Accounts)]
pub struct Hello {
	#[account(signer)]
	pub user: Signer,
}

impl Hello {
	#[inline(always)]
	pub fn handler(&self) -> Result<()> {
		msg!("Hello, Solana!");
		Ok(())
	}
}
