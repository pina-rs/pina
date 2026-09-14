//! Anchor v2 counter: PDA seeded by `b"counter" + authority`, u64 count,
//! `initialize` + `increment`.
use anchor_lang::prelude::*;

declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[program]
mod counter_anchor_v2 {
	use super::*;

	pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
		ctx.accounts.counter.count = 0;
		ctx.accounts.counter.bump = ctx.bumps.counter;
		msg!("Counter initialized");
		Ok(())
	}

	pub fn increment(ctx: &mut Context<Increment>) -> Result<()> {
		let counter = &mut ctx.accounts.counter;
		counter.count = counter
			.count
			.checked_add(1)
			.ok_or(ProgramError::ArithmeticOverflow)?;
		msg!("Counter incremented");
		Ok(())
	}
}

#[account]
pub struct CounterState {
	pub count: u64,
	pub bump: u8,
	pub _pad: [u8; 7],
}

#[derive(Accounts)]
pub struct Initialize {
	#[account(mut)]
	pub authority: Signer,
	#[account(init, payer = authority, seeds = [b"counter", authority.address().as_ref()], bump)]
	pub counter: Account<CounterState>,
	pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct Increment {
	// The authority only contributes PDA seeds here, so it is a read-only
	// signer. Every other fixture declares it the same way, which keeps the
	// four frameworks on identical accounts.
	pub authority: Signer,
	#[account(mut, seeds = [b"counter", authority.address().as_ref()], bump)]
	pub counter: Account<CounterState>,
}
