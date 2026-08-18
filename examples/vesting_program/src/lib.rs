//! Token vesting and lockup scaffold built with pina.
//!
//! This example keeps the core production-shaped contract in place:
//! - initialize a vesting schedule
//! - create a PDA-owned vault ATA
//! - claim vested amounts
//! - cancel a remaining schedule
//!
//! The first scaffold focuses on account structure, validation chains, and
//! deterministic IDL extraction. Token transfer wiring can be layered in later.

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
	use pina::*;

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
			VestingInstruction::Initialize => InitializeAccounts::try_from(accounts)?.process(data),
			VestingInstruction::Claim => ClaimAccounts::try_from(accounts)?.process(data),
			VestingInstruction::Cancel => CancelAccounts::try_from(accounts)?.process(data),
		}
	}
}

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VestingError {
	InvalidSchedule = 0,
	ClaimTooLarge = 1,
	AlreadyCancelled = 2,
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
#[pda(seeds = [VESTING_SEED_PREFIX, admin: Address, beneficiary: Address, mint: Address], bump = bump)]
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
	pub vault: &'a AccountView,
	pub associated_token_program: &'a AccountView,
	pub system_program: &'a AccountView,
	pub token_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct CancelAccounts<'a> {
	pub admin: &'a AccountView,
	pub mint: &'a AccountView,
	pub vesting_state: &'a mut AccountView,
	pub vault: &'a AccountView,
	pub token_program: &'a AccountView,
}

/// Seed prefix for vesting PDAs.
const VESTING_SEED_PREFIX: &[u8] = b"vesting";

const SPL_PROGRAM_IDS: [Address; 2] = [token::ID, token_2022::ID];

fn validate_schedule(start_ts: u64, cliff_ts: u64, end_ts: u64) -> ProgramResult {
	if start_ts > cliff_ts || cliff_ts > end_ts {
		return Err(VestingError::InvalidSchedule.into());
	}

	Ok(())
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
		let vesting_seeds_with_bump = vesting_seeds.with_bump(args.bump);

		self.admin.assert_signer()?;
		self.mint.assert_owners(&SPL_PROGRAM_IDS)?;
		self.associated_token_program
			.assert_address(&associated_token_account::ID)?;
		self.system_program.assert_address(&system::ID)?;
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		self.vesting_state
			.assert_empty()?
			.assert_seeds_with_bump(&vesting_seeds_with_bump.as_slices(), &ID)?;
		self.vault
			.assert_empty()?
			.assert_writable()?
			.assert_associated_token_address(
				self.vesting_state.address(),
				self.mint.address(),
				self.token_program.address(),
			)?;

		create_program_account_with_bump::<VestingState>(
			self.vesting_state,
			self.admin,
			&ID,
			&vesting_seeds.as_slices(),
			args.bump,
		)?;

		let mut vesting_state = self.vesting_state.as_account_mut::<VestingState>(&ID)?;
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
		drop(vesting_state);

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
		self.vesting_state
			.assert_not_empty()?
			.assert_type::<VestingState>(&ID)?;
		self.vault
			.assert_not_empty()?
			.assert_writable()?
			.assert_owners(&SPL_PROGRAM_IDS)?
			.assert_associated_token_address(
				self.vesting_state.address(),
				self.mint.address(),
				self.token_program.address(),
			)?;
		self.beneficiary_ata
			.assert_writable()?
			.assert_associated_token_address(
				self.beneficiary.address(),
				self.mint.address(),
				self.token_program.address(),
			)?;

		let (admin, beneficiary, mint, cancelled, claimed_amount, total_amount) = {
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
			)
		};
		// Verify the vesting state is the PDA for the admin, beneficiary, and
		// mint, using the stored bump field (avoids re-deriving the canonical
		// bump on-chain).
		VestingState::assert_seeds(self.vesting_state, &admin, &beneficiary, &mint, &ID)?;

		if cancelled {
			return Err(VestingError::AlreadyCancelled.into());
		}
		let next_claimed = claimed_amount
			.checked_add(amount)
			.ok_or(ProgramError::ArithmeticOverflow)?;
		if next_claimed > total_amount {
			return Err(VestingError::ClaimTooLarge.into());
		}

		let mut vesting_state = self.vesting_state.as_account_mut::<VestingState>(&ID)?;
		vesting_state.claimed_amount.set(next_claimed);

		associated_token_account::instructions::CreateIdempotent {
			funding_account: self.beneficiary,
			account: self.beneficiary_ata,
			wallet: self.beneficiary,
			mint: self.mint,
			system_program: self.system_program,
			token_program: self.token_program,
		}
		.invoke()?;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for CancelAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = CancelInstruction::try_from_bytes(data)?;

		self.admin.assert_signer()?;
		self.mint.assert_owners(&SPL_PROGRAM_IDS)?;
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		self.vesting_state
			.assert_not_empty()?
			.assert_type::<VestingState>(&ID)?;
		self.vault
			.assert_not_empty()?
			.assert_writable()?
			.assert_owners(&SPL_PROGRAM_IDS)?
			.assert_associated_token_address(
				self.vesting_state.address(),
				self.mint.address(),
				self.token_program.address(),
			)?;

		let (admin, beneficiary, mint, cancelled) = {
			let vesting_state = self.vesting_state.as_account::<VestingState>(&ID)?;

			self.admin.assert_address(&vesting_state.admin)?;
			self.mint.assert_address(&vesting_state.mint)?;

			(
				vesting_state.admin,
				vesting_state.beneficiary,
				vesting_state.mint,
				vesting_state.cancelled.get(),
			)
		};

		// Verify the vesting state is the PDA for the admin, beneficiary, and
		// mint, using the stored bump field (avoids re-deriving the canonical
		// bump on-chain).
		VestingState::assert_seeds(self.vesting_state, &admin, &beneficiary, &mint, &ID)?;

		if cancelled {
			return Err(VestingError::AlreadyCancelled.into());
		}

		let mut vesting_state = self.vesting_state.as_account_mut::<VestingState>(&ID)?;
		vesting_state.cancelled.set(true);

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn discriminator_values() {
		assert_eq!(VestingInstruction::Initialize as u8, 0);
		assert_eq!(VestingInstruction::Claim as u8, 1);
		assert_eq!(VestingInstruction::Cancel as u8, 2);
	}

	#[test]
	fn instruction_roundtrip() {
		let mut bytes = [0u8; ClaimInstruction::SIZE];
		ClaimInstruction::initialize(&mut bytes)
			.unwrap_or_else(|error| panic!("initialize failed: {error:?}"))
			.amount
			.set(10);
		let parsed = ClaimInstruction::try_from_bytes(&bytes)
			.unwrap_or_else(|e| panic!("decode failed: {e:?}"));
		assert_eq!(parsed.amount.get(), 10);
	}

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let wrong_program_id: Address = [9u8; 32].into();
		let data = [VestingInstruction::Initialize as u8];
		let result = parse_instruction::<VestingInstruction>(&wrong_program_id, &ID, &data);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}
}
