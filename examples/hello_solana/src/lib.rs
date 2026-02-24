//! Hello Solana — minimal pina program example.
//!
//! This is the simplest possible Solana program built with pina. It
//! demonstrates the basic structure every pina program follows:
//!
//! 1. Declare a program ID with [`declare_id!`].
//! 2. Define an instruction discriminator enum with [`#[discriminator]`].
//! 3. Define instruction data structs with [`#[instruction]`].
//! 4. Define an accounts struct with [`#[derive(Accounts)]`].
//! 5. Implement [`ProcessAccountInfos`] to hold the instruction logic.
//! 6. Wire up the entrypoint with [`nostd_entrypoint!`] and
//!    [`parse_instruction`].
//!
//! When invoked, this program simply logs "Hello, Solana!".

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

// The on-chain address of this program. Replace with your own deployed
// program ID in production.
declare_id!("DCF5KBmtQ9ryDC7mQezKLwuJHem6coVUCmKkw37M9J4A");

// ---------------------------------------------------------------------------
// Instruction discriminator
// ---------------------------------------------------------------------------

/// Every pina program defines a discriminator enum that maps instruction
/// variants to numeric tags. The `#[discriminator]` attribute macro generates:
///
/// - A `#[repr(u8)]` representation (configurable to u16/u32/u64).
/// - `TryFrom<u8>` so raw bytes can be parsed into variants.
/// - `IntoDiscriminator` for the framework's type-safe dispatch.
#[discriminator]
pub enum HelloInstruction {
	/// The only instruction this program supports.
	Hello = 0,
}

// ---------------------------------------------------------------------------
// Instruction data
// ---------------------------------------------------------------------------

/// The `#[instruction]` attribute macro generates:
///
/// - A discriminator field as the first byte of the struct.
/// - `HasDiscriminator` implementation linking this struct to
///   `HelloInstruction::Hello`.
/// - `Pod` and `Zeroable` derives for zero-copy deserialization.
/// - `TypedBuilder` for ergonomic construction in tests.
///
/// `HelloInstructionData` has no payload fields — only the discriminator byte
/// is needed to identify the instruction.
#[instruction(discriminator = HelloInstruction, variant = Hello)]
pub struct HelloInstructionData {}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

/// The accounts required by the Hello instruction.
///
/// `#[derive(Accounts)]` generates a `TryFromAccountInfos` implementation that
/// destructures a `&[AccountView]` slice into named fields, returning
/// `ProgramError::NotEnoughAccountKeys` if too few accounts are provided.
#[derive(Accounts, Debug)]
pub struct HelloAccounts<'a> {
	/// The user invoking the program. Must be a signer so we can trust the
	/// address is authentic.
	pub user: &'a AccountView,
}

// ---------------------------------------------------------------------------
// Instruction processor
// ---------------------------------------------------------------------------

/// Implement `ProcessAccountInfos` to define what happens when the Hello
/// instruction is executed.
///
/// The `process` method receives the raw instruction data (already validated
/// by `parse_instruction` in the entrypoint). Here we:
///
/// 1. Validate the discriminator via `HelloInstructionData::try_from_bytes`.
/// 2. Assert the user account is a signer.
/// 3. Log a greeting.
impl<'a> ProcessAccountInfos<'a> for HelloAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		// Validate instruction data (checks the discriminator byte).
		let _ = HelloInstructionData::try_from_bytes(data)?;

		// Validate that the user signed the transaction.
		self.user.assert_signer()?;

		// Log a greeting to the Solana runtime. Log messages are visible in
		// transaction logs on explorers and during testing.
		log!("Hello, Solana!");

		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Entrypoint (only compiled for on-chain BPF builds)
// ---------------------------------------------------------------------------

/// The entrypoint module is gated behind the `bpf-entrypoint` feature so that
/// tests and CPI consumers can use this crate as a library without pulling in
/// the BPF entrypoint machinery.
#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use pina::*;

	use super::*;

	// `nostd_entrypoint!` wires up:
	// - `program_entrypoint!` — the BPF entrypoint.
	// - `no_allocator!` — no heap allocator (zero allocation).
	// - `nostd_panic_handler!` — minimal panic handler for `no_std`.
	nostd_entrypoint!(process_instruction);

	/// The top-level instruction router.
	///
	/// 1. `parse_instruction` reads the discriminator byte from `data` and
	///    converts it into `HelloInstruction`, verifying the program ID.
	/// 2. The `match` dispatches to the appropriate accounts struct which
	///    handles validation and execution.
	#[inline(always)]
	pub fn process_instruction(
		program_id: &Address,
		accounts: &[AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: HelloInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			HelloInstruction::Hello => HelloAccounts::try_from(accounts)?.process(data),
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
	fn discriminator_hello_value() {
		// Verify the Hello variant has discriminator value 0.
		assert_eq!(HelloInstruction::Hello as u8, 0);
	}

	#[test]
	fn discriminator_roundtrip() {
		// Parse a discriminator byte back into the enum variant.
		let data = [0u8];
		let parsed = HelloInstruction::try_from(data[0]);
		assert!(parsed.is_ok());
	}

	#[test]
	fn discriminator_invalid_byte_fails() {
		// An invalid discriminator byte should fail to parse.
		let result = HelloInstruction::try_from(99u8);
		assert!(result.is_err());
	}

	#[test]
	fn instruction_data_has_discriminator() {
		// Verify that HelloInstructionData carries the correct discriminator.
		assert!(HelloInstructionData::matches_discriminator(&[0u8]));
		assert!(!HelloInstructionData::matches_discriminator(&[1u8]));
	}

	#[test]
	fn instruction_data_try_from_bytes() {
		// The instruction data is exactly 1 byte (just the discriminator).
		let data = [0u8];
		let result = HelloInstructionData::try_from_bytes(&data);
		assert!(result.is_ok());
	}

	#[test]
	fn instruction_data_wrong_discriminator_detected() {
		// `matches_discriminator` can detect wrong discriminators, but
		// `try_from_bytes` only checks the data layout (discriminator
		// dispatch happens in `parse_instruction` at the entrypoint level).
		assert!(!HelloInstructionData::matches_discriminator(&[1u8]));
	}

	#[test]
	fn program_id_is_valid() {
		// Verify the program ID was parsed correctly from the base58 string.
		assert_ne!(ID, Address::default());
	}
}
