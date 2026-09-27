//! Lean-entrypoint variant of the counter comparison.
//!
//! Identical program semantics to `counter/pina` — same account model, same
//! validation, same `pina` — with exactly one difference: the entrypoint's
//! account budget. `nostd_entrypoint!` accepts the maximum number of
//! accounts the program accepts (pinocchio's `process_entrypoint::<MAX_ACCOUNTS>`
//! generic), and the stock fixture uses the default 255, so its entrypoint
//! carries unrolled account-walking code for 255 accounts. This program
//! accepts at most 4 — both its instructions fit inside that — and the
//! deserializer code shrinks with the constant: 12,720 → 11,680 bytes
//! (−8.2%) at +1/+4 compute units.
//!
//! This fixture is the measurement bed for the lean-entrypoint exploration
//! (`docs/adrs/0000-lean-entrypoint-exploration.md`); it is not part of the
//! published comparison table, which shows the stock default.
#![no_std]

use pina::*;

declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator]
pub enum CounterInstruction {
	Initialize = 0,
	Increment = 1,
}

#[discriminator]
pub enum CounterAccountType {
	CounterState = 1,
}

#[account(discriminator = CounterAccountType)]
#[pda(seeds = [SEED_COUNTER, authority: Address], bump = bump)]
pub struct CounterState {
	pub bump: u8,
	pub count: u64,
}

#[instruction(discriminator = CounterInstruction::Initialize)]
pub struct InitializeInstruction {
	pub bump: u8,
}

#[instruction(discriminator = CounterInstruction::Increment)]
pub struct IncrementInstruction {}

const SEED_COUNTER: &[u8] = b"counter";

#[derive(Accounts, Debug)]
pub struct InitializeAccounts<'a> {
	pub authority: &'a AccountView,
	pub counter: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct IncrementAccounts<'a> {
	pub authority: &'a AccountView,
	pub counter: &'a mut AccountView,
}

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = InitializeInstruction::try_from_bytes(data)?;
		let authority_key = self.authority.address();
		let seeds = CounterState::seeds(authority_key);

		self.authority.assert_signer()?;
		self.counter.assert_empty()?;
		self.system_program.assert_address(&system::ID)?;

		CreateProgramAccountWithUncheckedBump {
			account: self.counter,
			payer: self.authority,
			owner: &ID,
			seeds: &seeds.as_slices(),
			bump: args.bump,
		}
		.invoke_with::<CounterState>(|counter| {
			counter.bump = args.bump;
			counter.count.set(0);
			Ok(())
		})?;

		log!("Counter initialized");

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for IncrementAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = IncrementInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;

		let mut counter = CounterState::load_pda_mut(self.counter, self.authority.address(), &ID)?;
		let current = counter.count.get();
		let next = current
			.checked_add(1)
			.ok_or(ProgramError::ArithmeticOverflow)?;

		counter.count.set(next);

		log!("Counter incremented");

		Ok(())
	}
}

// The widest instruction uses 3 accounts; 4 leaves one slot of headroom for a
// future instruction and still bounds the deserializer's unrolled walking
// code to a small constant.
nostd_entrypoint!(process_instruction, 4);

/// The top-level instruction router, outlined from the entrypoint frame.
#[inline(always)]
pub fn process_instruction(
	program_id: &Address,
	accounts: &mut [AccountView],
	data: &[u8],
) -> ProgramResult {
	let instruction: CounterInstruction = parse_instruction(program_id, &ID, data)?;

	match instruction {
		CounterInstruction::Initialize => {
			InitializeAccounts::try_from((program_id, accounts))?.process(data)
		}
		CounterInstruction::Increment => {
			IncrementAccounts::try_from((program_id, accounts))?.process(data)
		}
	}
}
