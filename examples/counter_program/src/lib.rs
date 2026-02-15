//! Counter program — demonstrates account state management with pina.
//!
//! This example shows how to use pina's macro system to define on-chain state
//! and mutate it across transactions. It covers:
//!
//! - **Account structs** with `#[account]` — zero-copy state with automatic
//!   discriminator fields, `Pod`/`Zeroable` derives, and `TypedBuilder`.
//! - **PDA-based accounts** — the counter is stored at a Program Derived
//!   Address seeded by the authority's public key, so each user gets their own
//!   counter.
//! - **Instruction dispatch** — `#[discriminator]` + `#[instruction]` +
//!   `parse_instruction` for type-safe routing.
//! - **Account validation chains** — `.assert_signer()?.assert_writable()?` for
//!   concise, composable checks.
//! - **`create_program_account`** — pina's CPI helper for PDA account creation.
//!
//! ## Instructions
//!
//! | Variant     | Description                               |
//! |-------------|-------------------------------------------|
//! | `Initialize` | Create a new counter PDA for the signer. |
//! | `Increment`  | Add 1 to the counter value.              |

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

// The on-chain address of this program.
declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

// ---------------------------------------------------------------------------
// Discriminators
// ---------------------------------------------------------------------------

/// Instruction discriminator. Each variant maps to a unique `u8` tag that
/// appears as the first byte of instruction data.
#[discriminator]
pub enum CounterInstruction {
	Initialize = 0,
	Increment = 1,
}

/// Account discriminator. Stored as the first byte of on-chain account data
/// so the program can distinguish between different account types.
///
/// The variant name **must match** the struct name it discriminates (e.g.
/// `CounterState` variant for the `CounterState` struct). This is how the
/// `#[account]` macro links structs to their discriminator values.
#[discriminator]
pub enum CounterAccountType {
	CounterState = 1,
}

// ---------------------------------------------------------------------------
// Account state
// ---------------------------------------------------------------------------

/// On-chain counter state.
///
/// The `#[account]` macro generates:
/// - A discriminator field (`CounterAccountType::Counter`) as the first byte.
/// - `Pod` + `Zeroable` derives for zero-copy (de)serialization.
/// - `HasDiscriminator` linking this struct to `CounterAccountType::Counter`.
/// - `TypedBuilder` for ergonomic construction.
///
/// Layout (10 bytes total):
/// ```text
/// | offset | size | field         |
/// |--------|------|---------------|
/// | 0      | 1    | discriminator |
/// | 1      | 1    | bump          |
/// | 2      | 8    | count (PodU64)|
/// ```
#[account(discriminator = CounterAccountType)]
pub struct CounterState {
	/// The PDA bump seed, stored on-chain so we don't need to re-derive it.
	pub bump: u8,
	/// The current counter value. Uses `PodU64` (a little-endian `u64`
	/// wrapper) for safe alignment in `#[repr(C)]` structs.
	pub count: PodU64,
}

// ---------------------------------------------------------------------------
// Instruction data structs
// ---------------------------------------------------------------------------

/// Instruction data for `Initialize`.
///
/// Contains the PDA bump seed so the client can pass a pre-computed bump
/// (avoids the cost of `find_program_address` on-chain).
#[instruction(discriminator = CounterInstruction, variant = Initialize)]
pub struct InitializeInstruction {
	/// The PDA bump seed, computed off-chain.
	pub bump: u8,
}

/// Instruction data for `Increment`. No extra payload beyond the
/// discriminator byte.
#[instruction(discriminator = CounterInstruction, variant = Increment)]
pub struct IncrementInstruction {}

// ---------------------------------------------------------------------------
// PDA seeds
// ---------------------------------------------------------------------------

/// Seed prefix for counter PDAs.
const COUNTER_SEED: &[u8] = b"counter";

/// Build the PDA seeds for a counter account.
///
/// Seeds: `["counter", <authority_address>]`
///
/// With bump: `["counter", <authority_address>, &[bump]]`
#[macro_export]
macro_rules! counter_seeds {
	($authority:expr) => {
		&[COUNTER_SEED, $authority]
	};
	($authority:expr, $bump:expr) => {
		&[COUNTER_SEED, $authority, &[$bump]]
	};
}

// ---------------------------------------------------------------------------
// Accounts structs
// ---------------------------------------------------------------------------

/// Accounts for the `Initialize` instruction.
///
/// `#[derive(Accounts)]` generates `TryFromAccountInfos` which maps positional
/// accounts to named fields.
#[derive(Accounts, Debug)]
pub struct InitializeAccounts<'a> {
	/// The wallet creating the counter. Pays for account creation and becomes
	/// the authority whose address seeds the PDA.
	pub authority: &'a AccountView,
	/// The counter PDA account (must be empty — not yet created).
	pub counter: &'a AccountView,
	/// The system program, required for `CreateAccount` CPI.
	pub system_program: &'a AccountView,
}

/// Accounts for the `Increment` instruction.
#[derive(Accounts, Debug)]
pub struct IncrementAccounts<'a> {
	/// The counter's authority. Must sign to prove ownership.
	pub authority: &'a AccountView,
	/// The counter PDA account (must already exist and be writable).
	pub counter: &'a AccountView,
}

// ---------------------------------------------------------------------------
// Instruction processors
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = InitializeInstruction::try_from_bytes(data)?;
		let authority_key = self.authority.address();

		// Build the PDA seeds with the bump from instruction data.
		let seeds = counter_seeds!(authority_key.as_ref());
		let seeds_with_bump = counter_seeds!(authority_key.as_ref(), args.bump);

		// --- Validate accounts ---

		// The authority must sign the transaction and is the rent payer.
		self.authority.assert_signer()?;

		// The counter account must be empty (not yet allocated) and writable.
		// We also verify the PDA derivation matches the expected seeds + bump.
		self.counter
			.assert_empty()?
			.assert_writable()?
			.assert_seeds_with_bump(seeds_with_bump, &ID)?;

		// Verify the system program address.
		self.system_program.assert_address(&system::ID)?;

		// --- Create the PDA account ---

		// `create_program_account_with_bump` issues a `CreateAccount` CPI
		// to the system program, allocating `size_of::<CounterState>()` bytes
		// and assigning ownership to this program.
		create_program_account_with_bump::<CounterState>(
			self.counter,
			self.authority,
			&ID,
			seeds,
			args.bump,
		)?;

		// --- Initialize account data ---

		// `as_account_mut` deserializes the raw account bytes into a mutable
		// reference to `CounterState`, verifying the owner and discriminator.
		let counter = self.counter.as_account_mut::<CounterState>(&ID)?;
		*counter = CounterState::builder()
			.bump(args.bump)
			.count(PodU64::from_primitive(0))
			.build();

		log!("Counter initialized");

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for IncrementAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		// Validate the instruction discriminator.
		let _ = IncrementInstruction::try_from_bytes(data)?;

		// --- Validate accounts ---

		// The authority must sign to prove they own this counter.
		self.authority.assert_signer()?;

		// The counter must exist, be writable, be the correct type, and
		// derive from the expected PDA seeds.
		let authority_key = self.authority.address();
		self.counter
			.assert_not_empty()?
			.assert_writable()?
			.assert_type::<CounterState>(&ID)?;

		// Read the current state to get the bump for seed verification.
		let counter = self.counter.as_account::<CounterState>(&ID)?;
		let seeds_with_bump = counter_seeds!(authority_key.as_ref(), counter.bump);
		self.counter.assert_seeds_with_bump(seeds_with_bump, &ID)?;

		// --- Mutate state ---

		let counter = self.counter.as_account_mut::<CounterState>(&ID)?;
		let current: u64 = counter.count.into();
		counter.count = PodU64::from_primitive(current + 1);

		log!("Counter incremented");

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
		let instruction: CounterInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			CounterInstruction::Initialize => InitializeAccounts::try_from(accounts)?.process(data),
			CounterInstruction::Increment => IncrementAccounts::try_from(accounts)?.process(data),
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
		assert_eq!(CounterInstruction::Initialize as u8, 0);
		assert_eq!(CounterInstruction::Increment as u8, 1);
	}

	#[test]
	fn discriminator_roundtrip() {
		assert!(CounterInstruction::try_from(0u8).is_ok());
		assert!(CounterInstruction::try_from(1u8).is_ok());
		assert!(CounterInstruction::try_from(99u8).is_err());
	}

	#[test]
	fn counter_state_layout() {
		// CounterState: 1 (discriminator) + 1 (bump) + 8 (count) = 10 bytes.
		assert_eq!(size_of::<CounterState>(), 10);
	}

	#[test]
	fn counter_state_discriminator() {
		assert!(CounterState::matches_discriminator(&[
			CounterAccountType::CounterState as u8
		]));
		assert!(!CounterState::matches_discriminator(&[0u8]));
	}

	#[test]
	fn counter_state_builder() {
		let state = CounterState::builder()
			.bump(42)
			.count(PodU64::from_primitive(100))
			.build();
		assert_eq!(state.bump, 42);
		assert_eq!(u64::from(state.count), 100);
	}

	#[test]
	fn counter_state_deserialize_roundtrip() {
		let state = CounterState::builder()
			.bump(7)
			.count(PodU64::from_primitive(999))
			.build();

		// Serialize to bytes via bytemuck.
		let bytes: &[u8] = bytemuck::bytes_of(&state);
		assert_eq!(bytes.len(), 10);

		// Deserialize back.
		let deserialized = CounterState::try_from_bytes(bytes)
			.unwrap_or_else(|e| panic!("deserialization failed: {e:?}"));
		assert_eq!(deserialized.bump, 7);
		assert_eq!(u64::from(deserialized.count), 999);
	}

	#[test]
	fn initialize_instruction_data_layout() {
		// InitializeInstruction: 1 (discriminator) + 1 (bump) = 2 bytes.
		assert_eq!(size_of::<InitializeInstruction>(), 2);
		assert!(InitializeInstruction::matches_discriminator(&[
			CounterInstruction::Initialize as u8
		]));
	}

	#[test]
	fn increment_instruction_data_layout() {
		// IncrementInstruction: 1 byte (discriminator only).
		assert_eq!(size_of::<IncrementInstruction>(), 1);
		assert!(IncrementInstruction::matches_discriminator(&[
			CounterInstruction::Increment as u8
		]));
	}

	#[test]
	fn initialize_instruction_try_from_bytes() {
		let data = [CounterInstruction::Initialize as u8, 42u8]; // discriminator + bump
		let ix = InitializeInstruction::try_from_bytes(&data)
			.unwrap_or_else(|e| panic!("failed: {e:?}"));
		assert_eq!(ix.bump, 42);
	}

	#[test]
	fn increment_instruction_try_from_bytes() {
		let data = [CounterInstruction::Increment as u8];
		let result = IncrementInstruction::try_from_bytes(&data);
		assert!(result.is_ok());
	}

	#[test]
	fn counter_seeds_macro() {
		let authority = [1u8; 32];
		let seeds = counter_seeds!(&authority);
		assert_eq!(seeds.len(), 2);
		assert_eq!(seeds[0], b"counter");
		assert_eq!(seeds[1], &authority);
	}

	#[test]
	fn counter_seeds_with_bump_macro() {
		let authority = [1u8; 32];
		let seeds = counter_seeds!(&authority, 42);
		assert_eq!(seeds.len(), 3);
		assert_eq!(seeds[0], b"counter");
		assert_eq!(seeds[1], &authority);
		assert_eq!(seeds[2], &[42u8]);
	}

	#[test]
	fn program_id_is_valid() {
		assert_ne!(ID, Address::default());
	}
}
