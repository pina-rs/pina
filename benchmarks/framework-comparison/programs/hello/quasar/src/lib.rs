//! Quasar half of the hello comparison.
#![no_std]

use quasar_lang::prelude::*;

declare_id!("DCF5KBmtQ9ryDC7mQezKLwuJHem6coVUCmKkw37M9J4A");

#[derive(Accounts)]
pub struct Hello {
	pub user: Signer,
}

#[program]
mod hello_quasar {
	use super::*;

	#[instruction(0)]
	pub fn hello(_ctx: Ctx<Hello>) -> Result<()> {
		solana_program_log::log("Hello, Solana!");
		Ok(())
	}
}
