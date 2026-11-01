//! Quasar half of the counter comparison. The state struct keeps the same
//! `discriminator, bump, count` order as the Pina and Pinocchio programs so the
//! account is the same ten bytes in all three.
#![no_std]

use quasar_lang::prelude::*;

declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[account(discriminator = 1)]
#[seeds(b"counter", authority: Address)]
pub struct CounterState {
	pub bump: u8,
	pub count: u64,
}

#[derive(Accounts)]
pub struct Initialize {
	pub authority: Signer,
	#[account(init(payer = authority), seeds(authority))]
	pub counter: Pda<Account<CounterState>>,
	pub system_program: Program<SystemProgram>,
}

#[derive(Accounts)]
pub struct Increment {
	pub authority: Signer,
	#[account(mut, seeds(authority))]
	pub counter: Pda<Account<CounterState>>,
}

#[program]
mod counter_quasar {
	use super::*;

	#[instruction(0)]
	pub fn initialize(mut ctx: Ctx<Initialize>) -> Result<()> {
		let bump = ctx.accounts.counter.bump();
		ctx.accounts
			.counter
			.set_inner(&CounterState { bump, count: 0 })?;
		solana_program_log::log("Counter initialized");
		Ok(())
	}

	#[instruction(1)]
	pub fn increment(mut ctx: Ctx<Increment>) -> Result<()> {
		let bump = ctx.accounts.counter.bump();
		let count = ctx.accounts.counter.count();
		let count = count
			.checked_add(1)
			.ok_or(ProgramError::ArithmeticOverflow)?;
		ctx.accounts
			.counter
			.set_inner(&CounterState { bump, count })?;
		solana_program_log::log("Counter incremented");
		Ok(())
	}
}
