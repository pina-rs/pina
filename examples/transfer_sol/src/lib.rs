//! Transfer SOL — demonstrates CPI and direct lamport transfers with pina.
//!
//! This example shows two ways to move SOL between accounts:
//!
//! 1. **CPI Transfer** (`TransferInstruction::CpiTransfer`) — calls the system
//!    program's `Transfer` instruction via Cross-Program Invocation. This is
//!    the standard approach when the sender is an externally-owned account (a
//!    wallet). Requires the system program account in the transaction.
//!
//! 2. **Direct Transfer** (`TransferInstruction::DirectTransfer`) — directly
//!    manipulates lamport balances using pina's `LamportTransfer` trait. This
//!    works **only when the sender is owned by this program**, since only the
//!    owning program may debit an account's lamports.
//!
//! ## Key pina features demonstrated
//!
//! - **Instruction data payloads** — the `#[instruction]` macro creates
//!   zero-copy structs with a discriminator and typed fields (`PodU64`).
//! - **`system::instructions::Transfer`** — pina re-exports pinocchio-system's
//!   struct-based CPI helpers.
//! - **`LamportTransfer` trait** — `account.send(lamports, recipient)` for
//!   program-owned account transfers without CPI.
//! - **Custom error types** — `#[error]` macro for program-specific errors with
//!   automatic `ProgramError::Custom` conversion.

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

// ---------------------------------------------------------------------------
// Program ID
// ---------------------------------------------------------------------------

declare_id!("BuXKn8EiVMKF8zYThuea3xhLq3jUHTTwDDLfCoehq7WG");

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Custom errors for this program.
///
/// The `#[error]` attribute macro:
/// - Assigns each variant a unique `u32` code (starting from the given value).
/// - Implements `From<TransferError> for ProgramError` so you can use `?` to
///   propagate these as `ProgramError::Custom(code)`.
#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferError {
	/// The sender does not have enough lamports for the transfer.
	InsufficientFunds = 0,
}

// ---------------------------------------------------------------------------
// Discriminators
// ---------------------------------------------------------------------------

/// Instruction discriminator for the two transfer methods.
#[discriminator]
pub enum TransferInstruction {
	/// Transfer SOL via CPI to the system program.
	CpiTransfer = 0,
	/// Transfer SOL by directly mutating lamport balances.
	DirectTransfer = 1,
}

// ---------------------------------------------------------------------------
// Instruction data
// ---------------------------------------------------------------------------

/// Instruction data for `CpiTransfer`.
///
/// Layout:
/// ```text
/// | offset | size | field         |
/// |--------|------|---------------|
/// | 0      | 1    | discriminator |
/// | 1      | 8    | amount (u64)  |
/// ```
#[instruction(discriminator = TransferInstruction, variant = CpiTransfer)]
pub struct CpiTransferInstruction {
	/// Amount of lamports to transfer.
	pub amount: PodU64,
}

/// Instruction data for `DirectTransfer`.
///
/// Same layout as `CpiTransferInstruction` but with a different discriminator
/// byte.
#[instruction(discriminator = TransferInstruction, variant = DirectTransfer)]
pub struct DirectTransferInstruction {
	/// Amount of lamports to transfer.
	pub amount: PodU64,
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

/// Accounts for the CPI transfer instruction.
///
/// Requires the system program so the CPI can invoke `system::Transfer`.
#[derive(Accounts, Debug)]
pub struct CpiTransferAccounts<'a> {
	/// The sender. Must be a signer and writable (lamports will be debited).
	pub sender: &'a AccountView,
	/// The recipient. Must be writable (lamports will be credited).
	pub recipient: &'a AccountView,
	/// The system program.
	pub system_program: &'a AccountView,
}

/// Accounts for the direct transfer instruction.
///
/// No system program needed — this program directly modifies lamport balances.
/// The sender account **must be owned by this program** for direct transfers.
#[derive(Accounts, Debug)]
pub struct DirectTransferAccounts<'a> {
	/// The sender. Must be owned by this program, writable, and a signer.
	pub sender: &'a AccountView,
	/// The recipient. Must be writable.
	pub recipient: &'a AccountView,
}

// ---------------------------------------------------------------------------
// Processors
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for CpiTransferAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = CpiTransferInstruction::try_from_bytes(data)?;
		let amount: u64 = args.amount.into();

		// --- Validate accounts ---

		// Sender must sign and be writable.
		self.sender.assert_signer()?.assert_writable()?;

		// Recipient must be writable to receive lamports.
		self.recipient.assert_writable()?;

		// Verify the system program address.
		self.system_program.assert_address(&system::ID)?;

		// Check the sender has enough lamports.
		if self.sender.lamports() < amount {
			return Err(TransferError::InsufficientFunds.into());
		}

		// --- Execute the CPI transfer ---
		//
		// `system::instructions::Transfer` is a struct-based CPI helper from
		// pinocchio-system, re-exported by pina. Calling `.invoke()` on it
		// issues a CPI to the system program's transfer instruction.
		system::instructions::Transfer {
			from: self.sender,
			to: self.recipient,
			lamports: amount,
		}
		.invoke()?;

		log!("CPI transfer complete");

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for DirectTransferAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = DirectTransferInstruction::try_from_bytes(data)?;
		let amount: u64 = args.amount.into();

		// --- Validate accounts ---

		// Sender must sign, be writable, and be owned by this program.
		// Only the owning program can debit an account's lamports.
		self.sender
			.assert_signer()?
			.assert_writable()?
			.assert_owner(&ID)?;

		// Recipient must be writable.
		self.recipient.assert_writable()?;

		// Check the sender has enough lamports.
		if self.sender.lamports() < amount {
			return Err(TransferError::InsufficientFunds.into());
		}

		// --- Execute the direct transfer ---
		//
		// `LamportTransfer::send` directly modifies the lamport balances
		// of both accounts. This avoids the overhead of a CPI but only
		// works when the sender is owned by the calling program.
		self.sender.send(amount, self.recipient)?;

		log!("Direct transfer complete");

		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Entrypoint
// ---------------------------------------------------------------------------

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use pina::*;

	use super::*;

	nostd_entrypoint!(process_instruction);

	#[inline(always)]
	pub fn process_instruction(
		program_id: &Address,
		accounts: &[AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: TransferInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			TransferInstruction::CpiTransfer => {
				CpiTransferAccounts::try_from(accounts)?.process(data)
			}
			TransferInstruction::DirectTransfer => {
				DirectTransferAccounts::try_from(accounts)?.process(data)
			}
		}
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn discriminator_values() {
		assert_eq!(TransferInstruction::CpiTransfer as u8, 0);
		assert_eq!(TransferInstruction::DirectTransfer as u8, 1);
	}

	#[test]
	fn discriminator_roundtrip() {
		assert!(TransferInstruction::try_from(0u8).is_ok());
		assert!(TransferInstruction::try_from(1u8).is_ok());
		assert!(TransferInstruction::try_from(99u8).is_err());
	}

	#[test]
	fn cpi_transfer_instruction_layout() {
		// CpiTransferInstruction: 1 (discriminator) + 8 (amount) = 9 bytes.
		assert_eq!(size_of::<CpiTransferInstruction>(), 9);
		assert!(CpiTransferInstruction::matches_discriminator(&[
			TransferInstruction::CpiTransfer as u8
		]));
	}

	#[test]
	fn direct_transfer_instruction_layout() {
		// DirectTransferInstruction: 1 (discriminator) + 8 (amount) = 9 bytes.
		assert_eq!(size_of::<DirectTransferInstruction>(), 9);
		assert!(DirectTransferInstruction::matches_discriminator(&[
			TransferInstruction::DirectTransfer as u8
		]));
	}

	#[test]
	fn cpi_transfer_instruction_deserialize() {
		let mut data = [0u8; 9];
		data[0] = TransferInstruction::CpiTransfer as u8;
		// Amount = 1_000_000 in little-endian.
		data[1..9].copy_from_slice(&1_000_000u64.to_le_bytes());

		let ix = CpiTransferInstruction::try_from_bytes(&data)
			.unwrap_or_else(|e| panic!("failed: {e:?}"));
		assert_eq!(u64::from(ix.amount), 1_000_000);
	}

	#[test]
	fn direct_transfer_instruction_deserialize() {
		let mut data = [0u8; 9];
		data[0] = TransferInstruction::DirectTransfer as u8;
		data[1..9].copy_from_slice(&500_000u64.to_le_bytes());

		let ix = DirectTransferInstruction::try_from_bytes(&data)
			.unwrap_or_else(|e| panic!("failed: {e:?}"));
		assert_eq!(u64::from(ix.amount), 500_000);
	}

	#[test]
	fn wrong_discriminator_detected_cpi() {
		// `matches_discriminator` detects wrong discriminators. The actual
		// dispatch check happens in `parse_instruction` at the entrypoint.
		let mut data = [0u8; 9];
		data[0] = TransferInstruction::DirectTransfer as u8; // Wrong for CPI.
		assert!(!CpiTransferInstruction::matches_discriminator(&data));
	}

	#[test]
	fn wrong_discriminator_detected_direct() {
		let mut data = [0u8; 9];
		data[0] = TransferInstruction::CpiTransfer as u8; // Wrong for direct.
		assert!(!DirectTransferInstruction::matches_discriminator(&data));
	}

	#[test]
	fn error_values() {
		// TransferError variants map to ProgramError::Custom codes.
		let err: ProgramError = TransferError::InsufficientFunds.into();
		assert!(matches!(err, ProgramError::Custom(_)));
	}

	#[test]
	fn program_id_is_valid() {
		assert_ne!(ID, Address::default());
	}
}
