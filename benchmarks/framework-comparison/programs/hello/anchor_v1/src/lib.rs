//! Anchor v1 half of the hello comparison.
use anchor_lang::prelude::*;

declare_id!("DCF5KBmtQ9ryDC7mQezKLwuJHem6coVUCmKkw37M9J4A");

#[program]
pub mod hello_anchor_v1 {
	use super::*;

	pub fn hello(_ctx: Context<Hello>) -> Result<()> {
		msg!("Hello, Solana!");
		Ok(())
	}
}

#[derive(Accounts)]
pub struct Hello<'info> {
	pub user: Signer<'info>,
}
