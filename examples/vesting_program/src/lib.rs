//! Vesting schedule with cliff and linear unlock, built with pina.
//!
//! This example demonstrates a complete vesting lifecycle:
//! - initialize a vesting schedule and fund its PDA-owned vault ATA
//! - release the vested portion of the allocation on claim
//! - return the unclaimed remainder to the admin on cancellation
//!
//! `Claim` reads the Clock sysvar, so a claim before the cliff is rejected and
//! the vested entitlement grows linearly to `end_ts`. Token release uses
//! `TransferChecked` signed by the vesting PDA, and `Cancel` refunds the
//! remaining vault balance and closes the vault.

#![allow(missing_docs)]
#![allow(clippy::inline_always)]
#![no_std]

// On native builds the cdylib target needs std for unwinding and panic
// handling. On BPF, `nostd_entrypoint!()` provides the panic handler and
// allocator. Tests link against std automatically.
#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("FEa5fqN6NACrhWUZSBdGKybJKNxkdw8cdLvRvTARsFHh");

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
		let instruction: VestingInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			VestingInstruction::Initialize => {
				InitializeAccounts::try_from((program_id, accounts))?.process(data)
			}
			VestingInstruction::Claim => {
				ClaimAccounts::try_from((program_id, accounts))?.process(data)
			}
			VestingInstruction::Cancel => {
				CancelAccounts::try_from((program_id, accounts))?.process(data)
			}
		}
	}
}

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VestingError {
	/// The schedule is malformed: its window is empty, unordered, or fully elapsed.
	InvalidSchedule = 0,
	/// The claim exceeds what has vested so far.
	ClaimTooLarge = 1,
	/// The vesting account was already cancelled and holds nothing to claim.
	AlreadyCancelled = 2,
	/// The schedule has not reached its cliff, so nothing has vested yet.
	CliffNotReached = 3,
	/// The vault holds fewer tokens than the claim must release.
	InsufficientVaultBalance = 4,
}

#[discriminator]
pub enum VestingInstruction {
	Initialize = 0,
	Claim = 1,
	Cancel = 2,
}

#[discriminator]
pub enum VestingAccountType {
	VestingState = 1,
}

#[account(discriminator = VestingAccountType)]
#[pda(seeds = [SEED_VESTING_PREFIX, admin: Address, beneficiary: Address, mint: Address], bump = bump)]
pub struct VestingState {
	pub admin: Address,
	pub beneficiary: Address,
	pub mint: Address,
	pub total_amount: u64,
	pub claimed_amount: u64,
	pub start_ts: u64,
	pub cliff_ts: u64,
	pub end_ts: u64,
	pub cancelled: bool,
	pub bump: u8,
}

#[instruction(discriminator = VestingInstruction::Initialize)]
pub struct InitializeInstruction {
	pub total_amount: u64,
	pub start_ts: u64,
	pub cliff_ts: u64,
	pub end_ts: u64,
	pub bump: u8,
}

#[instruction(discriminator = VestingInstruction::Claim)]
pub struct ClaimInstruction {
	pub amount: u64,
}

#[instruction(discriminator = VestingInstruction::Cancel)]
pub struct CancelInstruction {}

#[derive(Accounts, Debug)]
pub struct InitializeAccounts<'a> {
	pub admin: &'a mut AccountView,
	pub beneficiary: &'a AccountView,
	pub mint: &'a AccountView,
	pub vesting_state: &'a mut AccountView,
	pub vault: &'a AccountView,
	pub associated_token_program: &'a AccountView,
	pub system_program: &'a AccountView,
	pub token_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct ClaimAccounts<'a> {
	pub beneficiary: &'a mut AccountView,
	pub mint: &'a AccountView,
	pub vesting_state: &'a mut AccountView,
	pub beneficiary_ata: &'a AccountView,
	pub vault: &'a mut AccountView,
	pub associated_token_program: &'a AccountView,
	pub system_program: &'a AccountView,
	pub token_program: &'a AccountView,
	pub clock: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct CancelAccounts<'a> {
	pub admin: &'a mut AccountView,
	pub mint: &'a AccountView,
	pub vesting_state: &'a mut AccountView,
	pub admin_ata: &'a AccountView,
	pub vault: &'a mut AccountView,
	pub associated_token_program: &'a AccountView,
	pub system_program: &'a AccountView,
	pub token_program: &'a AccountView,
}

/// Seed prefix for vesting PDAs.
const SEED_VESTING_PREFIX: &[u8] = b"vesting";

/// The Clock sysvar, read to enforce the cliff and the linear unlock.
const CLOCK_SYSVAR_ID: Address = address!("SysvarC1ock11111111111111111111111111111111");

const SPL_PROGRAM_IDS: [Address; 2] = [token::ID, token_2022::ID];

fn validate_schedule(start_ts: u64, cliff_ts: u64, end_ts: u64) -> ProgramResult {
	if start_ts > cliff_ts || cliff_ts > end_ts {
		return Err(VestingError::InvalidSchedule.into());
	}

	Ok(())
}

/// Return the total entitled at `now`, rounding down to the token base unit.
///
/// Rounding down favors the schedule's issuer: the beneficiary can never be
/// released more than the linear fraction supports, and the final base unit
/// arrives only once `now` reaches `end_ts`. The intermediate is `u128` so a
/// large allocation cannot overflow the multiplication.
fn vested_amount(
	total_amount: u64,
	start_ts: u64,
	end_ts: u64,
	now: i64,
) -> Result<u64, ProgramError> {
	// A clock value before the schedule start vests nothing. `now` is signed
	// because the sysvar is, and a schedule is expressed in unix seconds.
	let Ok(now) = u64::try_from(now) else {
		return Ok(0);
	};

	if now >= end_ts {
		return Ok(total_amount);
	}
	if now <= start_ts {
		return Ok(0);
	}

	let elapsed = now - start_ts;
	let window = end_ts - start_ts;

	let vested = u128::from(total_amount)
		.checked_mul(u128::from(elapsed))
		.ok_or(ProgramError::ArithmeticOverflow)?
		/ u128::from(window);

	u64::try_from(vested).map_err(|_| ProgramError::ArithmeticOverflow)
}

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = InitializeInstruction::try_from_bytes(data)?;
		let start_ts = args.start_ts.get();
		let cliff_ts = args.cliff_ts.get();
		let end_ts = args.end_ts.get();

		validate_schedule(start_ts, cliff_ts, end_ts)?;

		let admin_address = *self.admin.address();
		let beneficiary_address = *self.beneficiary.address();
		let mint_address = *self.mint.address();
		let vesting_seeds =
			VestingState::seeds(&admin_address, &beneficiary_address, &mint_address);

		self.admin.assert_signer()?;
		self.mint.assert_owners(&SPL_PROGRAM_IDS)?;
		self.associated_token_program
			.assert_address(&associated_token_account::ID)?;
		self.system_program.assert_address(&system::ID)?;
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		// The address check lives in the `Create` CPI below: the associated token
		// program derives the same `[wallet, token_program, mint]` seeds and
		// rejects a mismatch with `InvalidSeeds` before it creates anything.
		self.vault.assert_empty()?.assert_writable()?;

		// The seeds bind `admin`, `beneficiary`, and `mint`, and `Initialize`
		// requires the admin to sign, so only that admin can duplicate its own
		// schedule. `Claim` and `Cancel` re-derive these seeds from the stored
		// fields and require the matching party to sign, so a duplicate stays
		// scoped to the signers who could already create one.
		CreateProgramAccountWithUncheckedBump {
			account: self.vesting_state,
			payer: self.admin,
			owner: &ID,
			seeds: &vesting_seeds.as_slices(),
			bump: args.bump,
		}
		.invoke_with::<VestingState>(|vesting_state| {
			vesting_state.admin = admin_address;
			vesting_state.beneficiary = beneficiary_address;
			vesting_state.mint = mint_address;
			vesting_state.total_amount = args.total_amount;
			vesting_state.claimed_amount.set(0);
			vesting_state.start_ts = args.start_ts;
			vesting_state.cliff_ts = args.cliff_ts;
			vesting_state.end_ts = args.end_ts;
			vesting_state.cancelled.set(false);
			vesting_state.bump = args.bump;

			Ok(())
		})?;

		associated_token_account::instructions::Create {
			account: self.vault,
			funding_account: self.admin,
			wallet: self.vesting_state,
			mint: self.mint,
			system_program: self.system_program,
			token_program: self.token_program,
		}
		.invoke()?;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for ClaimAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = ClaimInstruction::try_from_bytes(data)?;
		let amount = args.amount.get();

		self.beneficiary.assert_signer()?;
		self.mint.assert_owners(&SPL_PROGRAM_IDS)?;
		self.associated_token_program
			.assert_address(&associated_token_account::ID)?;
		self.system_program.assert_address(&system::ID)?;
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		self.vesting_state.assert_not_empty()?;
		self.vault
			.assert_not_empty()?
			.assert_writable()?
			.assert_owners(&SPL_PROGRAM_IDS)?;
		// The address check lives in the `CreateIdempotent` CPI below: the
		// associated token program derives the same seeds and rejects a mismatch
		// with `InvalidSeeds` before its idempotent branch.
		self.beneficiary_ata.assert_writable()?;
		self.clock.assert_sysvar(&CLOCK_SYSVAR_ID)?;

		let (
			admin,
			beneficiary,
			mint,
			cancelled,
			claimed_amount,
			total_amount,
			start_ts,
			cliff_ts,
			end_ts,
			bump,
		) = {
			let vesting_state = self.vesting_state.as_account::<VestingState>(&ID)?;
			self.beneficiary
				.assert_address(&vesting_state.beneficiary)?;
			self.mint.assert_address(&vesting_state.mint)?;

			(
				vesting_state.admin,
				vesting_state.beneficiary,
				vesting_state.mint,
				vesting_state.cancelled.get(),
				vesting_state.claimed_amount.get(),
				vesting_state.total_amount.get(),
				vesting_state.start_ts.get(),
				vesting_state.cliff_ts.get(),
				vesting_state.end_ts.get(),
				vesting_state.bump,
			)
		};
		// Verify the vesting state is the PDA for the admin, beneficiary, and
		// mint. The parse above already captured `bump`, so this performs the
		// identical single-derivation check without re-parsing the account.
		VestingState::assert_stored_bump(
			self.vesting_state,
			bump,
			&admin,
			&beneficiary,
			&mint,
			&ID,
		)?;

		if cancelled {
			return Err(VestingError::AlreadyCancelled.into());
		}

		let now = sysvars::clock::Clock::from_account_view(self.clock)?.unix_timestamp;
		// The cliff gates the first release; before it nothing has vested.
		let Ok(now_u64) = u64::try_from(now) else {
			return Err(VestingError::CliffNotReached.into());
		};
		if now_u64 < cliff_ts {
			return Err(VestingError::CliffNotReached.into());
		}

		let next_claimed = claimed_amount
			.checked_add(amount)
			.ok_or(ProgramError::ArithmeticOverflow)?;
		// Two upper bounds, in order of specificity: the schedule's total and
		// what the linear curve has released by `now`. The second is the one
		// that makes the schedule meaningful.
		if next_claimed > total_amount {
			return Err(VestingError::ClaimTooLarge.into());
		}
		let vested = vested_amount(total_amount, start_ts, end_ts, now)?;
		if next_claimed > vested {
			return Err(VestingError::ClaimTooLarge.into());
		}
		// The payout is signed by the schedule, so the source must provably be
		// the schedule's own vault: this load derives the associated token
		// address and rejects a mismatch, which is the check that stops the
		// schedule's signature from draining a caller-supplied account. It is
		// the vault's only derivation on this path.
		let vault_balance = self
			.vault
			.as_associated_token_account(
				self.vesting_state.address(),
				self.mint.address(),
				self.token_program.address(),
			)?
			.amount();
		if vault_balance < amount {
			return Err(VestingError::InsufficientVaultBalance.into());
		}

		let mut vesting_state = self.vesting_state.as_account_mut::<VestingState>(&ID)?;
		vesting_state.claimed_amount.set(next_claimed);
		drop(vesting_state);

		associated_token_account::instructions::CreateIdempotent {
			funding_account: self.beneficiary,
			account: self.beneficiary_ata,
			wallet: self.beneficiary,
			mint: self.mint,
			system_program: self.system_program,
			token_program: self.token_program,
		}
		.invoke()?;

		// Release the claimed amount out of the vault. The vault is the PDA's
		// own ATA, so the vesting state signs for it.
		let mint_decimals = {
			let mint = self
				.mint
				.as_token_mint_for_program(self.token_program.address())?
				.assert_no_extensions()?;
			mint.decimals()
		};
		let vesting_seeds = VestingState::seeds(&admin, &beneficiary, &mint).with_bump(bump);
		let signer = vesting_seeds.to_signer();
		let signers = [signer.as_signer()];

		token::instructions::TransferChecked::new(
			self.vault,
			self.mint,
			self.beneficiary_ata,
			self.vesting_state,
			amount,
			mint_decimals,
		)
		.invoke_signed_with_program(&signers, self.token_program.address())?;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for CancelAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = CancelInstruction::try_from_bytes(data)?;

		self.admin.assert_signer()?.assert_writable()?;
		self.mint.assert_owners(&SPL_PROGRAM_IDS)?;
		self.associated_token_program
			.assert_address(&associated_token_account::ID)?;
		self.system_program.assert_address(&system::ID)?;
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		self.vesting_state.assert_not_empty()?;
		self.vault
			.assert_not_empty()?
			.assert_writable()?
			.assert_owners(&SPL_PROGRAM_IDS)?;
		// The address check lives in the `CreateIdempotent` CPI below: the
		// associated token program derives the same seeds and rejects a mismatch
		// with `InvalidSeeds` before its idempotent branch.
		self.admin_ata.assert_writable()?;

		let (admin, beneficiary, mint, cancelled, bump) = {
			let vesting_state = self.vesting_state.as_account::<VestingState>(&ID)?;

			self.admin.assert_address(&vesting_state.admin)?;
			self.mint.assert_address(&vesting_state.mint)?;

			(
				vesting_state.admin,
				vesting_state.beneficiary,
				vesting_state.mint,
				vesting_state.cancelled.get(),
				vesting_state.bump,
			)
		};

		// Verify the vesting state is the PDA for the admin, beneficiary, and
		// mint. The parse above already captured `bump`, so this performs the
		// identical single-derivation check without re-parsing the account.
		VestingState::assert_stored_bump(
			self.vesting_state,
			bump,
			&admin,
			&beneficiary,
			&mint,
			&ID,
		)?;

		if cancelled {
			return Err(VestingError::AlreadyCancelled.into());
		}

		// The refund is signed by the schedule, so the source must provably be
		// the schedule's own vault: this load derives the associated token
		// address and rejects a mismatch, which is the check that stops the
		// schedule's signature from draining a caller-supplied account. It is
		// the vault's only derivation on this path. It also reads the balance
		// the vault must return before the schedule is marked cancelled, so a
		// later failure cannot leave it flagged as refunded.
		let remaining = self
			.vault
			.as_associated_token_account(
				self.vesting_state.address(),
				self.mint.address(),
				self.token_program.address(),
			)?
			.amount();

		let mut vesting_state = self.vesting_state.as_account_mut::<VestingState>(&ID)?;
		vesting_state.cancelled.set(true);
		drop(vesting_state);

		associated_token_account::instructions::CreateIdempotent {
			funding_account: self.admin,
			account: self.admin_ata,
			wallet: self.admin,
			mint: self.mint,
			system_program: self.system_program,
			token_program: self.token_program,
		}
		.invoke()?;

		let mint_decimals = {
			let mint = self
				.mint
				.as_token_mint_for_program(self.token_program.address())?
				.assert_no_extensions()?;
			mint.decimals()
		};
		let vesting_seeds = VestingState::seeds(&admin, &beneficiary, &mint).with_bump(bump);
		let signer = vesting_seeds.to_signer();
		let signers = [signer.as_signer()];

		// Return everything the beneficiary has not claimed, then close the
		// vault so the rent follows the refund. A zero balance skips the
		// transfer because SPL rejects a zero-amount move.
		if remaining > 0 {
			token::instructions::TransferChecked::new(
				self.vault,
				self.mint,
				self.admin_ata,
				self.vesting_state,
				remaining,
				mint_decimals,
			)
			.invoke_signed_with_program(&signers, self.token_program.address())?;
		}

		token::instructions::CloseAccount::new(self.vault, self.admin, self.vesting_state)
			.invoke_signed_with_program(&signers, self.token_program.address())?;

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn vested_amount_is_linear_and_rounds_down() {
		// Fully elapsed: everything is available.
		assert_eq!(vested_amount(1_000, 0, 100, 100).unwrap_or_default(), 1_000);
		assert_eq!(vested_amount(1_000, 0, 100, 250).unwrap_or_default(), 1_000);
		// Before the window: nothing.
		assert_eq!(vested_amount(1_000, 100, 200, 100).unwrap_or_default(), 0);
		assert_eq!(vested_amount(1_000, 100, 200, 50).unwrap_or_default(), 0);
		// Halfway: half, by integer division.
		assert_eq!(vested_amount(1_000, 0, 100, 50).unwrap_or_default(), 500);
		// A fractional result floors, never rounds up.
		assert_eq!(vested_amount(1_000, 0, 3, 1).unwrap_or_default(), 333);
		assert_eq!(vested_amount(1_000, 0, 3, 2).unwrap_or_default(), 666);
		// A negative clock reading vests nothing rather than underflowing.
		assert_eq!(vested_amount(1_000, 0, 100, -5).unwrap_or_default(), 0);
	}

	#[test]
	fn vested_amount_handles_a_large_allocation_without_overflow() {
		// u64::MAX * elapsed would overflow a u64; the u128 intermediate keeps
		// the multiplication exact.
		let total = u64::MAX;
		let half = vested_amount(total, 0, 2, 1).unwrap_or_default();
		assert_eq!(half, total / 2);
		let full = vested_amount(total, 0, 2, 2).unwrap_or_default();
		assert_eq!(full, total);
	}

	#[test]
	fn discriminator_values() {
		assert_eq!(VestingInstruction::Initialize as u8, 0);
		assert_eq!(VestingInstruction::Claim as u8, 1);
		assert_eq!(VestingInstruction::Cancel as u8, 2);
	}

	#[test]
	fn instruction_roundtrip() {
		let mut bytes = [0u8; ClaimInstruction::SIZE];
		ClaimInstruction::initialize(&mut bytes, |instruction| {
			instruction.amount.set(10);
			Ok(())
		})
		.unwrap_or_else(|error| panic!("initialize failed: {error:?}"));
		let parsed = ClaimInstruction::try_from_bytes(&bytes)
			.unwrap_or_else(|e| panic!("decode failed: {e:?}"));
		assert_eq!(parsed.amount.get(), 10);
	}

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let wrong_program_id: Address = [9u8; 32].into();
		let data = [VestingInstruction::Initialize as u8, 0];
		let result = parse_instruction::<VestingInstruction>(&wrong_program_id, &ID, &data);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}
}
