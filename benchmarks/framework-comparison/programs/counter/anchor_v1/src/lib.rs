//! Anchor v1 counter: PDA seeded by `b"counter" + authority`, u64 count,
//! `initialize` + `increment`.
use anchor_lang::prelude::*;

declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[program]
pub mod counter_anchor_v1 {
	use super::*;

	pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
		ctx.accounts.counter.count = 0;
		ctx.accounts.counter.bump = ctx.bumps.counter;
		msg!("Counter initialized");
		Ok(())
	}

	pub fn increment(ctx: Context<Increment>) -> Result<()> {
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
}

#[derive(Accounts)]
pub struct Initialize<'info> {
	#[account(mut)]
	pub authority: Signer<'info>,
	#[account(
		init,
		payer = authority,
		// 8-byte Anchor discriminator + u64 count + u8 bump.
		space = 17,
		seeds = [b"counter", authority.key().as_ref()],
		bump
	)]
	pub counter: Account<'info, CounterState>,
	pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Increment<'info> {
	// The authority only contributes PDA seeds here, so it is a read-only
	// signer, matching the other fixtures.
	pub authority: Signer<'info>,
	#[account(mut, seeds = [b"counter", authority.key().as_ref()], bump)]
	pub counter: Account<'info, CounterState>,
}
