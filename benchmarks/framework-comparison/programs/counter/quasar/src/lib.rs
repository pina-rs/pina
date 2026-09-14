//! Quasar half of the counter comparison. The state struct keeps the same
//! `discriminator, bump, count` order as the Pina and Pinocchio programs so the
//! account is the same ten bytes in all three.
#![no_std]

use quasar_lang::prelude::*;

declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[program]
mod counter_quasar {
	use super::*;

	#[instruction(discriminator = 0)]
	pub fn initialize(ctx: Ctx<Initialize>) -> Result<(), ProgramError> {
		ctx.accounts.handler(&ctx.bumps)
	}

	#[instruction(discriminator = 1)]
	pub fn increment(ctx: Ctx<Increment>) -> Result<(), ProgramError> {
		ctx.accounts.handler()
	}
}

#[account(discriminator = 1, set_inner)]
#[seeds(b"counter", authority: Address)]
pub struct CounterState {
	pub bump: u8,
	pub count: u64,
}

#[derive(Accounts)]
pub struct Initialize {
	pub authority: Signer,
	#[account(init, payer = authority, address = CounterState::seeds(authority.address()))]
	pub counter: Account<CounterState>,
	pub system_program: Program<SystemProgram>,
}

impl Initialize {
	#[inline(always)]
	pub fn handler(&mut self, bumps: &InitializeBumps) -> Result<(), ProgramError> {
		self.counter.set_inner(CounterStateInner {
			bump: bumps.counter,
			count: 0,
		});
		log!("Counter initialized");
		Ok(())
	}
}

#[derive(Accounts)]
pub struct Increment {
	pub authority: Signer,
	#[account(mut, address = CounterState::seeds(authority.address()))]
	pub counter: Account<CounterState>,
}

impl Increment {
	#[inline(always)]
	pub fn handler(&mut self) -> Result<(), ProgramError> {
		let bump = self.counter.bump;
		let count = self
			.counter
			.count
			.checked_add(1)
			.ok_or(ProgramError::ArithmeticOverflow)?;
		self.counter.set_inner(CounterStateInner {
			bump,
			count: count.into(),
		});
		log!("Counter incremented");
		Ok(())
	}
}
