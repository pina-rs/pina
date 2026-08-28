//! Optional accounts program — demonstrates `Option<&AccountView>` fields.
//!
//! Every instruction keeps a **fixed** account layout: when a caller omits an
//! optional account, the generated clients fill the slot with a readonly meta
//! pointing at this program's own address. On-chain, a slot holding the
//! program ID parses as [`None`].
//!
//! Coverage matrix:
//!
//! | Instruction | Optional field                    | Behaviour when present            |
//! |-------------|-----------------------------------|-----------------------------------|
//! | `Init`      | —                                 | Creates the store PDA (baseline). |
//! | `Touch`     | `store: Option<&mut AccountView>` | Increments the stored counter.    |
//! | `Inspect`   | `store: Option<&AccountView>`     | Logs the observed count.          |
//! | `Inspect`   | `witness: Option<&AccountView>`   | Must be a signer.                 |
//! | `Note`      | `note: Option<&AccountView>`      | Logged as an opaque reference.    |

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

declare_id!("ccdMMVpwebk8NxwJdY4CndxkLKUTM6fkaFUteAfFeci");

// ---------------------------------------------------------------------------
// Discriminators
// ---------------------------------------------------------------------------

#[discriminator]
pub enum OptionalInstruction {
	Init = 0,
	Touch = 1,
	Inspect = 2,
	Note = 3,
}

#[discriminator]
pub enum OptionalAccountKind {
	StoreState = 1,
}

// ---------------------------------------------------------------------------
// Account state
// ---------------------------------------------------------------------------

/// On-chain store state touched through the optional mutable slot.
///
/// Layout (10 bytes): 1 discriminator + 1 bump + 8 count.
#[account(discriminator = OptionalAccountKind)]
#[pda(seeds = [STORE_SEED, authority: Address], bump = bump)]
pub struct StoreState {
	pub bump: u8,
	pub count: u64,
}

// ---------------------------------------------------------------------------
// Instruction data structs
// ---------------------------------------------------------------------------

#[instruction(discriminator = OptionalInstruction::Init)]
pub struct InitInstruction {
	/// The PDA bump seed, computed off-chain.
	pub bump: u8,
}

#[instruction(discriminator = OptionalInstruction::Touch)]
pub struct TouchInstruction {}

#[instruction(discriminator = OptionalInstruction::Inspect)]
pub struct InspectInstruction {}

#[instruction(discriminator = OptionalInstruction::Note)]
pub struct NoteInstruction {}

// ---------------------------------------------------------------------------
// Seeds
// ---------------------------------------------------------------------------

const STORE_SEED: &[u8] = b"store";

// ---------------------------------------------------------------------------
// Accounts structs
// ---------------------------------------------------------------------------

/// Accounts for `Init`. No optional slots — the baseline layout.
#[derive(Accounts, Debug)]
pub struct InitAccounts<'a> {
	/// Pays for account creation and seeds the store PDA.
	pub authority: &'a mut AccountView,
	/// The store PDA account (must be empty — not yet created).
	pub store: &'a mut AccountView,
	/// The system program, required for the create-account CPI.
	pub system_program: &'a AccountView,
}

/// Accounts for `Touch`.
///
/// The store slot is **optional and mutable**: provided values must be
/// writable and hold a valid [`StoreState`]; omitted slots parse as [`None`].
#[derive(Accounts, Debug)]
pub struct TouchAccounts<'a> {
	/// The store's authority. Must sign.
	pub authority: &'a AccountView,
	/// When present, the counter inside is incremented by one.
	pub store: Option<&'a mut AccountView>,
}

/// Accounts for `Inspect`.
///
/// Combines an optional immutable data account with an optional signer so a
/// single instruction exercises both flavours of immutability.
#[derive(Accounts, Debug)]
pub struct InspectAccounts<'a> {
	/// The transaction fee payer; always required.
	pub authority: &'a AccountView,
	/// When present, must be the caller's store PDA.
	pub store: Option<&'a AccountView>,
	/// When present, must have signed the transaction.
	pub witness: Option<&'a AccountView>,
}

/// Accounts for `Note`.
///
/// The note slot is optional, readonly, and carries no type constraints —
/// any account may be attached as context.
#[derive(Accounts, Debug)]
pub struct NoteAccounts<'a> {
	/// The transaction fee payer; always required.
	pub authority: &'a AccountView,
	/// An arbitrary readonly account attached as context.
	pub note: Option<&'a AccountView>,
}

// ---------------------------------------------------------------------------
// Instruction processors
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for InitAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = InitInstruction::try_from_bytes(data)?;
		let authority_key = *self.authority.address();
		let seeds = StoreState::seeds(&authority_key);
		let seeds_with_bump = seeds.with_bump(args.bump);

		self.authority.assert_signer()?;
		self.store
			.assert_empty()?
			.assert_seeds_with_bump(&seeds_with_bump.as_slices(), &ID)?;
		self.system_program.assert_address(&system::ID)?;

		CreateProgramAccountWithBump {
			account: self.store,
			payer: self.authority,
			owner: &ID,
			seeds: &seeds.as_slices(),
			bump: args.bump,
		}
		.invoke::<StoreState>()?;

		let mut store = self.store.as_account_mut::<StoreState>(&ID)?;
		store.bump = args.bump;
		store.count.set(0);

		log!("store initialized");

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for TouchAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = TouchInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;

		match self.store {
			Some(store) => {
				store.assert_not_empty()?.assert_type::<StoreState>(&ID)?;
				StoreState::assert_seeds(store, self.authority.address(), &ID)?;

				let mut state = store.as_account_mut::<StoreState>(&ID)?;
				let next = state
					.count
					.get()
					.checked_add(1)
					.ok_or(ProgramError::ArithmeticOverflow)?;
				state.count.set(next);

				log!("store present: incremented");
			}
			None => {
				log!("store absent: nothing to increment");
			}
		}

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for InspectAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = InspectInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;

		if let Some(witness) = self.witness {
			witness.assert_signer()?;
			log!("witness present");
		} else {
			log!("witness absent");
		}

		match self.store {
			Some(store) => {
				store.assert_type::<StoreState>(&ID)?;
				StoreState::assert_seeds(store, self.authority.address(), &ID)?;
				let state = store.as_account::<StoreState>(&ID)?;
				log!("store count: {}", state.count.get());
			}
			None => {
				log!("store absent");
			}
		}

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for NoteAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = NoteInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;

		if let Some(note) = self.note {
			let _ = note.address();
			log!("note present");
		} else {
			log!("note absent");
		}

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
		accounts: &mut [AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: OptionalInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			OptionalInstruction::Init => {
				InitAccounts::try_from((program_id, accounts))?.process(data)
			}
			OptionalInstruction::Touch => {
				TouchAccounts::try_from((program_id, accounts))?.process(data)
			}
			OptionalInstruction::Inspect => {
				InspectAccounts::try_from((program_id, accounts))?.process(data)
			}
			OptionalInstruction::Note => {
				NoteAccounts::try_from((program_id, accounts))?.process(data)
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
		assert_eq!(OptionalInstruction::Init as u8, 0);
		assert_eq!(OptionalInstruction::Touch as u8, 1);
		assert_eq!(OptionalInstruction::Inspect as u8, 2);
		assert_eq!(OptionalInstruction::Note as u8, 3);
	}

	#[test]
	fn discriminator_roundtrip() {
		for value in 0..=3u8 {
			assert!(OptionalInstruction::try_from(value).is_ok());
		}
		assert!(OptionalInstruction::try_from(99u8).is_err());
	}

	#[test]
	fn store_state_layout() {
		assert_eq!(StoreState::SIZE, 10);
	}

	#[test]
	fn store_state_discriminator() {
		assert!(StoreState::matches_discriminator(&[
			OptionalAccountKind::StoreState as u8
		]));
		assert!(!StoreState::matches_discriminator(&[0u8]));
	}

	#[test]
	fn store_seeds() {
		let authority = Address::new_from_array([7u8; 32]);
		let derived = StoreState::seeds(&authority);
		let seeds = derived.as_slices();
		assert_eq!(seeds.len(), 2);
		assert_eq!(seeds[0], STORE_SEED);
		assert_eq!(seeds[1], authority.as_ref());
	}

	#[test]
	fn init_instruction_layout() {
		let data = [OptionalInstruction::Init as u8, 42u8];
		let ix = InitInstruction::try_from_bytes(&data).unwrap_or_else(|e| panic!("failed: {e:?}"));
		assert_eq!(ix.bump, 42);
		assert_eq!(InitInstruction::SIZE, 2);
	}

	#[test]
	fn empty_instructions_have_discriminator_only() {
		assert_eq!(TouchInstruction::SIZE, 1);
		assert_eq!(InspectInstruction::SIZE, 1);
		assert_eq!(NoteInstruction::SIZE, 1);
		assert!(TouchInstruction::try_from_bytes(&[OptionalInstruction::Touch as u8]).is_ok());
		assert!(InspectInstruction::try_from_bytes(&[OptionalInstruction::Inspect as u8]).is_ok());
		assert!(NoteInstruction::try_from_bytes(&[OptionalInstruction::Note as u8]).is_ok());
	}

	#[test]
	fn program_id_is_valid() {
		assert_ne!(ID, Address::default());
	}
}
