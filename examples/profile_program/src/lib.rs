//! Profile program — demonstrates `PinaPod`'s bounded fields in fixed account
//! state.
//!
//! Pina validates the complete `PinaPod` representation before exposing a
//! zero-copy view:
//!
//! - **`String<32>` / `String<128>`** — UTF-8 text with a one-byte length
//!   prefix and fixed inline capacity.
//! - **`Vec<u64, 8>`** — up to eight `u64` values with a two-byte count.
//! - **`Option<T>`** — fixed-size optional data backed by `PodOption` in the
//!   generated zero-copy view. Used here for an optional favourite tag.
//! - **`PodBool`** — a single-byte boolean for the `active` flag.
//!
//! ## Instructions
//!
//! | Variant         | Description                                  |
//! |-----------------|----------------------------------------------|
//! | `Initialize`    | Create a new profile PDA for the signer.     |
//! | `UpdateProfile` | Replace the profile name and bio.            |
//! | `AddTag`        | Append a tag to the profile's tag list.      |
//! | `RemoveTag`     | Remove the tag at the given index.           |

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
declare_id!("6oW4PDgWpZGWqAEZNvqnAtQi8GotATsxxjCLYQpZJhHL");

// ---------------------------------------------------------------------------
// Discriminators
// ---------------------------------------------------------------------------

/// Instruction discriminator. Each variant maps to a unique `u8` tag that
/// appears as the first byte of instruction data.
#[discriminator]
pub enum ProfileInstruction {
	Initialize = 0,
	UpdateProfile = 1,
	AddTag = 2,
	RemoveTag = 3,
}

/// Account discriminator. Stored as the first byte of on-chain account data
/// so the program can distinguish between different account types.
#[discriminator]
pub enum ProfileAccountType {
	ProfileState = 1,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Custom program errors.
#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileError {
	/// The tag list is full (capacity 8).
	TagOverflow = 1,
	/// The tag index is out of range.
	TagNotFound = 2,
}

// ---------------------------------------------------------------------------
// Account state
// ---------------------------------------------------------------------------

/// On-chain profile state.
///
/// The `#[account]` macro generates:
/// - A discriminator field (`ProfileAccountType::ProfileState`) as the first
///   byte.
/// - `PinaAccount` and `PinaPod` validation for checked zero-copy access.
/// - `HasDiscriminator` linking this account to
///   `ProfileAccountType::ProfileState`.
/// - `initialize` and `try_from_bytes` helpers for caller-owned storage.
///
/// Layout (240 bytes total):
/// ```text
/// | offset | size | field          |
/// |--------|------|----------------|
/// | 0      | 1    | discriminator  |
/// | 1      | 1    | bump           |
/// | 2      | 33   | name (`String<32>`)  |
/// | 35     | 129  | bio (`String<128>`)  |
/// | 164    | 66   | tags (`Vec<u64, 8>`) |
/// | 230    | 9    | favorite_tag (PodOption<PodU64>) |
/// | 239    | 1    | active (PodBool) |
/// ```
#[account(discriminator = ProfileAccountType)]
#[pda(seeds = [SEED_PROFILE, authority: Address], bump = bump)]
pub struct ProfileState {
	/// The PDA bump seed, stored on-chain so we don't need to re-derive it.
	pub bump: u8,
	/// UTF-8 display name with 32 bytes of inline capacity.
	pub name: String<32>,
	/// UTF-8 biography with 128 bytes of inline capacity.
	pub bio: String<128>,
	/// Up to eight tags stored inline.
	pub tags: Vec<u64, 8>,
	/// An optional favourite tag. The generated view uses a one-byte tag and
	/// an eight-byte value slot, even when the option is `None`.
	pub favorite_tag: Option<u64>,
	/// Whether the profile is active.
	pub active: bool,
}

// ---------------------------------------------------------------------------
// Instruction data structs
// ---------------------------------------------------------------------------

/// Instruction data for `Initialize`.
///
/// Contains the PDA bump seed and bounded initial name and bio.
#[instruction(discriminator = ProfileInstruction, variant = Initialize)]
pub struct InitializeInstruction {
	/// The PDA bump seed, computed off-chain.
	pub bump: u8,
	/// The initial display name.
	pub name: String<32>,
	/// The initial bio.
	pub bio: String<128>,
}

/// Instruction data for `UpdateProfile`. Replaces both name and bio.
#[instruction(discriminator = ProfileInstruction, variant = UpdateProfile)]
pub struct UpdateProfileInstruction {
	/// The new display name.
	pub name: String<32>,
	/// The new bio.
	pub bio: String<128>,
}

/// Instruction data for `AddTag`. Appends a tag to the profile.
#[instruction(discriminator = ProfileInstruction, variant = AddTag)]
pub struct AddTagInstruction {
	/// The tag value to append.
	pub tag: u64,
}

/// Instruction data for `RemoveTag`. Removes the tag at `index`.
#[instruction(discriminator = ProfileInstruction, variant = RemoveTag)]
pub struct RemoveTagInstruction {
	/// The zero-based index of the tag to remove.
	pub index: u64,
}

// ---------------------------------------------------------------------------
// PDA seeds
// ---------------------------------------------------------------------------

/// Seed prefix for profile PDAs.
const SEED_PROFILE: &[u8] = b"profile";

// ---------------------------------------------------------------------------
// Accounts structs
// ---------------------------------------------------------------------------

/// Accounts for the `Initialize` instruction.
#[derive(Accounts, Debug)]
pub struct InitializeAccounts<'a> {
	/// The wallet creating the profile. Pays for account creation and becomes
	/// the authority whose address seeds the PDA.
	pub authority: &'a mut AccountView,
	/// The profile PDA account (must be empty — not yet created).
	pub profile: &'a mut AccountView,
	/// The system program, required for `CreateAccount` CPI.
	pub system_program: &'a AccountView,
}

/// Accounts for the `UpdateProfile`, `AddTag`, and `RemoveTag` instructions.
#[derive(Accounts, Debug)]
pub struct ProfileAccounts<'a> {
	/// The profile's authority. Must sign to prove ownership.
	pub authority: &'a AccountView,
	/// The profile PDA account (must already exist and be writable).
	pub profile: &'a mut AccountView,
}

// ---------------------------------------------------------------------------
// Instruction processors
// ---------------------------------------------------------------------------

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		// Parse instruction and prepare PDA seeds
		let args = InitializeInstruction::try_from_bytes(data)?;
		let authority_key = *self.authority.address();
		let seeds = ProfileState::seeds(&authority_key);
		let seeds_with_bump = seeds.with_bump(args.bump);

		// Validate accounts
		self.authority.assert_signer()?;
		let canonical_bump = self
			.profile
			.assert_canonical_bump(&seeds.as_slices(), &ID)?;
		if canonical_bump != args.bump {
			return Err(ProgramError::InvalidSeeds);
		}
		self.profile
			.assert_empty()?
			.assert_writable()?
			.assert_seeds_with_bump(&seeds_with_bump.as_slices(), &ID)?;
		self.system_program.assert_address(&system::ID)?;

		// Create the PDA account
		CreateProgramAccountWithBump {
			account: self.profile,
			payer: self.authority,
			owner: &ID,
			seeds: &seeds.as_slices(),
			bump: args.bump,
		}
		.invoke_with::<ProfileState>(|profile| {
			profile.bump = args.bump;
			profile.name = args.name;
			profile.bio = args.bio;
			profile.tags.clear();
			profile.favorite_tag.clear();
			profile.active.set(true);
			Ok(())
		})?;

		log!("Profile initialized");

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for ProfileAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		// Load and validate the profile once. The guard keeps the validated
		// representation borrowed while the stored bump is checked and the
		// selected mutation is applied.
		self.authority.assert_signer()?;
		let mut profile = ProfileState::load_pda_mut(self.profile, self.authority.address(), &ID)?;

		// Dispatch on the instruction discriminator
		let instruction = ProfileInstruction::try_from(
			*data.first().ok_or(ProgramError::InvalidInstructionData)?,
		)
		.map_err(|_| ProgramError::InvalidInstructionData)?;

		match instruction {
			ProfileInstruction::UpdateProfile => {
				let args = UpdateProfileInstruction::try_from_bytes(data)?;
				profile.name = args.name;
				profile.bio = args.bio;

				log!("Profile updated");
			}
			ProfileInstruction::AddTag => {
				let args = AddTagInstruction::try_from_bytes(data)?;
				profile
					.tags
					.try_push(args.tag.get())
					.map_err(|_| ProfileError::TagOverflow)?;

				log!("Tag added");
			}
			ProfileInstruction::RemoveTag => {
				let args = RemoveTagInstruction::try_from_bytes(data)?;
				let index = args.index.get();
				let index = usize::try_from(index).map_err(|_| ProfileError::TagNotFound)?;
				profile
					.tags
					.remove(index)
					.ok_or(ProfileError::TagNotFound)?;

				log!("Tag removed");
			}
			ProfileInstruction::Initialize => {
				return Err(ProgramError::InvalidInstructionData);
			}
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
		let instruction: ProfileInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			ProfileInstruction::Initialize => {
				InitializeAccounts::try_from((program_id, accounts))?.process(data)
			}
			ProfileInstruction::UpdateProfile
			| ProfileInstruction::AddTag
			| ProfileInstruction::RemoveTag => {
				ProfileAccounts::try_from((program_id, accounts))?.process(data)
			}
		}
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	extern crate std;

	use super::*;

	#[test]
	fn discriminator_values() {
		assert_eq!(ProfileInstruction::Initialize as u8, 0);
		assert_eq!(ProfileInstruction::UpdateProfile as u8, 1);
		assert_eq!(ProfileInstruction::AddTag as u8, 2);
		assert_eq!(ProfileInstruction::RemoveTag as u8, 3);
	}

	#[test]
	fn discriminator_roundtrip() {
		assert!(ProfileInstruction::try_from(0u8).is_ok());
		assert!(ProfileInstruction::try_from(3u8).is_ok());
		assert!(ProfileInstruction::try_from(99u8).is_err());
	}

	#[test]
	fn profile_state_layout() {
		// 1 (discriminator) + 1 (bump) + 33 (name) + 129 (bio) + 66 (tags)
		// + 9 (favorite tag) + 1 (active) = 240 bytes.
		assert_eq!(ProfileState::SIZE, 240);
	}

	#[test]
	fn profile_state_discriminator() {
		assert!(ProfileState::matches_discriminator(&[
			ProfileAccountType::ProfileState as u8
		]));
		assert!(!ProfileState::matches_discriminator(&[0u8]));
	}

	#[test]
	fn profile_state_initialization() {
		let mut bytes = [0u8; ProfileState::SIZE];
		let state = ProfileState::initialize(&mut bytes, |state| {
			state.bump = 42;
			state.active.set(true);
			Ok(())
		})
		.unwrap();
		assert_eq!(state.bump, 42);
		assert_eq!(state.name.as_str(), "");
		assert_eq!(state.tags.len(), 0);
		assert!(state.favorite_tag.is_none());
		assert!(state.active.get());
	}

	#[test]
	fn bounded_string_roundtrip() {
		let empty = String::<32>::default();
		let name = String::<32>::try_from("alice")
			.unwrap_or_else(|error| panic!("encoding failed: {error:?}"));

		assert_eq!(empty.as_str(), "");
		assert_eq!(name.as_str(), "alice");
		assert_eq!(size_of::<String<32>>(), 33);
	}

	#[test]
	fn bounded_fields_preserve_wire_layout() {
		let mut bytes = [0u8; ProfileState::SIZE];
		ProfileState::initialize(&mut bytes, |state| {
			state.name.try_set("alice")?;
			state.bio.try_set("hi")?;
			state.tags.try_set([7u64, 9u64])?;
			Ok(())
		})
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));

		assert_eq!(bytes[2], 5);
		assert_eq!(&bytes[3..8], b"alice");
		assert!(bytes[8..35].iter().all(|byte| *byte == 0));
		assert_eq!(bytes[35], 2);
		assert_eq!(&bytes[36..38], b"hi");
		assert!(bytes[38..164].iter().all(|byte| *byte == 0));
		assert_eq!(&bytes[164..166], 2u16.to_le_bytes());
		assert_eq!(&bytes[166..174], 7u64.to_le_bytes());
		assert_eq!(&bytes[174..182], 9u64.to_le_bytes());
		assert!(bytes[182..230].iter().all(|byte| *byte == 0));
	}

	#[test]
	fn bounded_string_rejects_invalid_utf8() {
		let mut bytes = [0u8; ProfileState::SIZE];
		ProfileState::initialize(&mut bytes, |_| Ok(())).unwrap();
		bytes[2] = 1;
		bytes[3] = 0xff;

		assert!(matches!(
			ProfileState::try_from_bytes(&bytes),
			Err(ProgramError::InvalidAccountData)
		));
	}

	#[test]
	fn bounded_string_rejects_length_over_capacity() {
		let mut bytes = [0u8; ProfileState::SIZE];
		ProfileState::initialize(&mut bytes, |_| Ok(())).unwrap();
		bytes[2] = 33;

		assert!(matches!(
			ProfileState::try_from_bytes(&bytes),
			Err(ProgramError::InvalidAccountData)
		));
	}

	#[test]
	fn bounded_tags_roundtrip() {
		let mut bytes = [0u8; ProfileState::SIZE];
		let state = ProfileState::initialize(&mut bytes, |_| Ok(())).unwrap();
		state.tags.try_push(1u64).unwrap();
		state.tags.try_push(2u64).unwrap();

		assert_eq!(state.tags.len(), 2);
		assert_eq!(state.tags.get(0).map(PodU64::get), Some(1));
		assert_eq!(state.tags.get(1).map(PodU64::get), Some(2));
		assert_eq!(state.tags.remove(0).map(|tag| tag.get()), Some(1));
		assert_eq!(state.tags.len(), 1);
		assert_eq!(state.tags.get(0).map(PodU64::get), Some(2));
	}

	#[test]
	fn bounded_tags_reject_capacity_overflow() {
		let mut bytes = [0u8; ProfileState::SIZE];
		let state = ProfileState::initialize(&mut bytes, |_| Ok(())).unwrap();
		for tag in 0..8u64 {
			state.tags.try_push(tag).unwrap();
		}

		assert_eq!(state.tags.try_push(8u64), Err(PinaPodError::Overflow));
	}

	#[test]
	fn bounded_tags_reject_length_over_capacity() {
		let mut bytes = [0u8; ProfileState::SIZE];
		ProfileState::initialize(&mut bytes, |_| Ok(())).unwrap();
		bytes[164..166].copy_from_slice(&9u16.to_le_bytes());

		assert!(matches!(
			ProfileState::try_from_bytes(&bytes),
			Err(ProgramError::InvalidAccountData)
		));
	}

	#[test]
	fn initialize_instruction_data_layout() {
		// 1 (discriminator) + 1 (bump) + 33 (name) + 129 (bio) = 164 bytes.
		assert_eq!(InitializeInstruction::SIZE, 164);
		assert!(InitializeInstruction::matches_discriminator(&[
			ProfileInstruction::Initialize as u8
		]));
	}

	#[test]
	fn update_profile_instruction_data_layout() {
		// 1 (discriminator) + 33 (name) + 129 (bio) = 163 bytes.
		assert_eq!(UpdateProfileInstruction::SIZE, 163);
	}

	#[test]
	fn add_tag_instruction_data_layout() {
		// 1 (discriminator) + 8 (tag) = 9 bytes.
		assert_eq!(AddTagInstruction::SIZE, 9);
	}

	#[test]
	fn remove_tag_instruction_data_layout() {
		// 1 (discriminator) + 8 (index) = 9 bytes.
		assert_eq!(RemoveTagInstruction::SIZE, 9);
	}

	#[test]
	fn initialize_instruction_try_from_bytes() {
		let mut data = [0u8; InitializeInstruction::SIZE];
		InitializeInstruction::initialize(&mut data, |initialized| {
			initialized.bump = 42;
			initialized.name.try_set("ali")?;
			Ok(())
		})
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
		let ix = InitializeInstruction::try_from_bytes(&data)
			.unwrap_or_else(|e| panic!("failed: {e:?}"));
		assert_eq!(ix.bump, 42);
		assert_eq!(ix.name.as_str(), "ali");
	}

	#[test]
	fn initialize_instruction_reports_invalid_utf8() {
		let mut data = [0u8; InitializeInstruction::SIZE];
		InitializeInstruction::initialize(&mut data, |_| Ok(()))
			.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
		data[2] = 1;
		data[3] = 0xff;

		assert!(matches!(
			InitializeInstruction::try_from_bytes(&data),
			Err(ProgramError::InvalidInstructionData)
		));
	}

	#[test]
	fn semantic_mutations_preserve_valid_profile_storage() {
		let mut bytes = [0u8; ProfileState::SIZE];

		{
			ProfileState::initialize(&mut bytes, |state| {
				state.name.try_set("alice")?;
				state.bio.try_set("hello")?;
				Ok(())
			})
			.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
		}
		{
			let state = ProfileState::try_from_bytes(&bytes)
				.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
			assert_eq!(state.name.as_str(), "alice");
			assert_eq!(state.bio.as_str(), "hello");
		}

		{
			let state = ProfileState::try_from_bytes_mut(&mut bytes)
				.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
			state
				.tags
				.try_push(7u64)
				.unwrap_or_else(|error| panic!("tag push failed: {error:?}"));
			state.favorite_tag.set(Some(PodU64::from(7)));
			state.active.set(true);
		}
		{
			let state = ProfileState::try_from_bytes(&bytes)
				.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
			assert_eq!(state.tags.len(), 1);
			assert_eq!(state.tags.get(0).map(PodU64::get), Some(7));
			assert_eq!(state.favorite_tag.get(), Some(PodU64::from(7)));
			assert!(state.active.get());
		}

		{
			let state = ProfileState::try_from_bytes_mut(&mut bytes)
				.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
			let removed = state.tags.remove(0);
			assert_eq!(removed.map(|tag| tag.get()), Some(7));
			state.tags.clear();
			state.favorite_tag.clear();
			state.active.set(false);
		}
		{
			let state = ProfileState::try_from_bytes(&bytes)
				.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
			assert_eq!(state.tags.len(), 0);
			assert_eq!(state.favorite_tag.get(), None);
			assert!(!state.active.get());
		}
	}

	#[test]
	fn profile_seeds() {
		let authority = Address::new_from_array([1u8; 32]);
		let seeds = ProfileState::seeds(&authority);
		let slices = seeds.as_slices();
		assert_eq!(slices.len(), 2);
		assert_eq!(slices[0], b"profile");
		assert_eq!(slices[1], authority.as_ref());
	}

	#[test]
	fn profile_seeds_with_bump() {
		let authority = Address::new_from_array([1u8; 32]);
		let seeds = ProfileState::seeds(&authority);
		let with_bump = seeds.with_bump(42);
		let slices = with_bump.as_slices();
		assert_eq!(slices.len(), 3);
		assert_eq!(slices[0], b"profile");
		assert_eq!(slices[1], authority.as_ref());
		assert_eq!(slices[2], &[42u8]);
	}

	#[test]
	fn program_id_is_valid() {
		assert_ne!(ID, Address::default());
	}
}
