//! A minimal Pina program with one migration-aware account and one instruction.
//!
//! This fixture exists to be modified by an evaluating agent. It is deliberately
//! small so that the *process* of changing a schema — snapshot, transition,
//! build, check — is the interesting part rather than the application logic.

#![allow(missing_docs)]
#![no_std]

use pina::*;

declare_id!("Counter111111111111111111111111111111111111");

/// Instruction discriminators.
#[discriminator(entrypoint, migrations_max_lamports = MAX_MIGRATION_LAMPORTS)]
pub enum CounterInstruction {
	/// Increments the stored counter.
	Increment = 0,
	/// Resets the stored counter to zero.
	Reset = 1,
}

/// Account discriminators.
#[discriminator]
pub enum CounterAccount {
	/// The single counter this program stores.
	Counter = 1,
}

/// Rent-exemption head room for on-demand growth.
///
/// Roughly 6,960 lamports per grown byte, and a whole stale ladder has to be
/// funded in one transaction rather than one step.
const MAX_MIGRATION_LAMPORTS: u64 = 20_000;

/// The stored counter.
#[account(discriminator = CounterAccount::Counter, compact)]
pub struct Counter {
	/// The signer allowed to increment this counter.
	pub authority: Address,
	/// The current count.
	pub count: u64,
	/// A short label for the counter.
	pub label: String<8>,
}

/// Increments the counter by `amount`.
#[instruction(discriminator = CounterInstruction::Increment)]
pub struct IncrementInstruction {
	/// How much to add.
	pub amount: u64,
}

/// Resets the counter, discarding its current value.
#[instruction(discriminator = CounterInstruction::Reset)]
pub struct ResetInstruction {
	/// Reserved so the payload stays non-empty.
	pub reserved: u8,
}

/// Accounts for `Increment`.
#[derive(Accounts)]
pub struct IncrementAccounts<'a> {
	/// The counter authority, which also funds any growth.
	#[pina(validate(signer))]
	pub authority: &'a mut AccountView,
	/// The counter to update.
	pub counter: &'a mut AccountView,
	/// Optional payer used when the counter must be migrated first.
	#[pina(validate(signer))]
	pub migration_payer: Option<&'a mut AccountView>,
	/// The system program, required alongside `migration_payer`.
	pub system_program: Option<&'a AccountView>,
}

/// Accounts for `Reset`.
#[derive(Accounts)]
pub struct ResetAccounts<'a> {
	/// The counter authority, which also funds any growth.
	#[pina(validate(signer))]
	pub authority: &'a mut AccountView,
	/// The counter to reset.
	pub counter: &'a mut AccountView,
}

impl<'a> ProcessAccountInfos<'a> for IncrementAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let instruction = IncrementInstruction::try_from_bytes(data)?;
		let authority_key = *self.authority.address();

		self.authority.assert_signer()?;
		self.counter.assert_owner(&ID)?.assert_writable()?;

		let payer = self.migration_payer.map(|account| &*account);
		if let Some(system_program) = self.system_program {
			system_program.assert_address(&system::ID)?;
		}

		MigrateAccount {
			account: self.counter,
			payer,
			program_id: &ID,
			max_lamports: Some(MAX_MIGRATION_LAMPORTS),
		}
		.invoke::<Counter>()?;

		let next = self.counter.with_compact_account::<Counter, _>(&ID, |counter| {
			if counter.authority != authority_key {
				return Err(ProgramError::InvalidAccountData);
			}
			Ok(counter.count.get().saturating_add(instruction.amount.get()))
		})?;

		UpdateResizableAccount {
			account: self.counter,
			rent_account: self.authority,
			program_id: &ID,
			patch: CounterPatch::new().count(next),
		}
		.invoke::<Counter>()?;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for ResetAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = ResetInstruction::try_from_bytes(data)?;
		let authority_key = *self.authority.address();

		self.authority.assert_signer()?;
		self.counter.assert_owner(&ID)?.assert_writable()?;

		MigrateAccount {
			account: self.counter,
			payer: None,
			program_id: &ID,
			max_lamports: None,
		}
		.invoke::<Counter>()?;

		self.counter.with_compact_account::<Counter, _>(&ID, |counter| {
			if counter.authority != authority_key {
				return Err(ProgramError::InvalidAccountData);
			}
			Ok(())
		})?;

		UpdateResizableAccount {
			account: self.counter,
			rent_account: self.authority,
			program_id: &ID,
			patch: CounterPatch::new().count(0),
		}
		.invoke::<Counter>()?;

		Ok(())
	}
}

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use super::*;

	nostd_entrypoint!(process_instruction);

	#[inline(always)]
	pub fn process_instruction(
		program_id: &Address,
		accounts: &mut [AccountView],
		data: &[u8],
	) -> ProgramResult {
		CounterInstruction::process_instruction(program_id, accounts, data)
	}
}
