//! Profile program — demonstrates Pod collections (`PodString`, `PodVec`) in
//! on-chain account state.
//!
//! This example shows how to store variable-length data in zero-copy account
//! layouts using pina's fixed-capacity Pod collections:
//!
//! - **`PodString<N, PFX>`** — a fixed-capacity UTF-8 string with a length
//!   prefix. Used here for the profile `name` (32 bytes) and `bio` (128
//!   bytes). The program validates UTF-8 with `try_as_str()` before storing.
//! - **`PodVec<T, N, PFX>`** — a fixed-capacity vector with a length prefix.
//!   Used here for a list of up to 8 `PodU64` tags.
//! - **`PodBool`** — a single-byte boolean for the `active` flag.
//!
//! Because every field is `Pod` with alignment 1, the whole account is
//! zero-copy: `as_account::<ProfileState>()` casts the raw account bytes
//! directly, with no (de)serialization.
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
	/// A `PodString` field contained invalid UTF-8.
	InvalidUtf8 = 0,
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
/// - `Pod` + `Zeroable` derives for zero-copy (de)serialization.
/// - `HasDiscriminator` linking this struct to
///   `ProfileAccountType::ProfileState`.
/// - `TypedBuilder` for ergonomic construction.
///
/// Layout (231 bytes total):
/// ```text
/// | offset | size | field          |
/// |--------|------|----------------|
/// | 0      | 1    | discriminator  |
/// | 1      | 1    | bump           |
/// | 2      | 33   | name (PodString<32>)  |
/// | 35     | 129  | bio (PodString<128>)  |
/// | 164    | 66   | tags (PodVec<PodU64, 8>) |
/// | 230    | 1    | active (PodBool) |
/// ```
#[account(discriminator = ProfileAccountType)]
pub struct ProfileState {
	/// The PDA bump seed, stored on-chain so we don't need to re-derive it.
	pub bump: u8,
	/// The profile display name. `PodString<32>` = 1 length byte + 32 UTF-8
	/// bytes.
	pub name: PodString<32>,
	/// A longer free-form bio. `PodString<128>` = 1 length byte + 128 UTF-8
	/// bytes.
	pub bio: PodString<128>,
	/// Up to 8 tags. `PodVec<PodU64, 8>` = 2 count bytes + 8 × 8-byte
	/// elements.
	pub tags: PodVec<PodU64, 8>,
	/// Whether the profile is active.
	pub active: PodBool,
}

// ---------------------------------------------------------------------------
// Instruction data structs
// ---------------------------------------------------------------------------

/// Instruction data for `Initialize`.
///
/// Contains the PDA bump seed and the initial name/bio. The `PodString`
/// fields carry their own length prefix, so the client writes
/// `discriminator + bump + len(name) + name + len(bio) + bio`.
#[instruction(discriminator = ProfileInstruction, variant = Initialize)]
pub struct InitializeInstruction {
	/// The PDA bump seed, computed off-chain.
	pub bump: u8,
	/// The initial display name (UTF-8, up to 32 bytes).
	pub name: PodString<32>,
	/// The initial bio (UTF-8, up to 128 bytes).
	pub bio: PodString<128>,
}

/// Instruction data for `UpdateProfile`. Replaces both name and bio.
#[instruction(discriminator = ProfileInstruction, variant = UpdateProfile)]
pub struct UpdateProfileInstruction {
	/// The new display name (UTF-8, up to 32 bytes).
	pub name: PodString<32>,
	/// The new bio (UTF-8, up to 128 bytes).
	pub bio: PodString<128>,
}

/// Instruction data for `AddTag`. Appends a tag to the profile.
#[instruction(discriminator = ProfileInstruction, variant = AddTag)]
pub struct AddTagInstruction {
	/// The tag value to append.
	pub tag: PodU64,
}

/// Instruction data for `RemoveTag`. Removes the tag at `index`.
#[instruction(discriminator = ProfileInstruction, variant = RemoveTag)]
pub struct RemoveTagInstruction {
	/// The zero-based index of the tag to remove.
	pub index: PodU64,
}

// ---------------------------------------------------------------------------
// PDA seeds
// ---------------------------------------------------------------------------

/// Seed prefix for profile PDAs.
const PROFILE_SEED: &[u8] = b"profile";

/// Build the PDA seeds for a profile account.
///
/// Seeds: `["profile", <authority_address>]`
///
/// With bump: `["profile", <authority_address>, &[bump]]`
#[macro_export]
macro_rules! profile_seeds {
	($authority:expr) => {
		&[PROFILE_SEED, $authority]
	};
	($authority:expr, $bump:expr) => {
		&[PROFILE_SEED, $authority, &[$bump]]
	};
}

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

		// Validate accounts
		self.authority.assert_signer()?;
		self.profile
			.assert_empty()?
			.assert_writable()?
			.assert_seeds_with_bump(
				profile_seeds!(self.authority.address().as_ref(), args.bump),
				&ID,
			)?;
		self.system_program.assert_address(&system::ID)?;

		// Validate UTF-8 before storing anything on-chain
		args.name
			.try_as_str()
			.map_err(|_| ProfileError::InvalidUtf8)?;
		args.bio
			.try_as_str()
			.map_err(|_| ProfileError::InvalidUtf8)?;

		// Create the PDA account
		create_program_account_with_bump::<ProfileState>(
			self.profile,
			self.authority,
			&ID,
			profile_seeds!(self.authority.address().as_ref()),
			args.bump,
		)?;

		// Initialize account data
		let mut profile = self.profile.as_account_mut::<ProfileState>(&ID)?;
		*profile = ProfileState::builder()
			.bump(args.bump)
			.name(args.name)
			.bio(args.bio)
			.tags(PodVec::default())
			.active(PodBool::from_bool(true))
			.build();

		log!("Profile initialized");

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for ProfileAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		// Validate accounts
		self.authority.assert_signer()?;

		let authority_key = self.authority.address();
		self.profile
			.assert_not_empty()?
			.assert_writable()?
			.assert_type::<ProfileState>(&ID)?;

		let bump = {
			let profile = self.profile.as_account::<ProfileState>(&ID)?;
			profile.bump
		};
		let seeds_with_bump = profile_seeds!(authority_key.as_ref(), bump);
		self.profile.assert_seeds_with_bump(seeds_with_bump, &ID)?;

		// Dispatch on the instruction discriminator
		let instruction = ProfileInstruction::try_from(
			*data.first().ok_or(ProgramError::InvalidInstructionData)?,
		)
		.map_err(|_| ProgramError::InvalidInstructionData)?;

		match instruction {
			ProfileInstruction::UpdateProfile => {
				let args = UpdateProfileInstruction::try_from_bytes(data)?;
				args.name
					.try_as_str()
					.map_err(|_| ProfileError::InvalidUtf8)?;
				args.bio
					.try_as_str()
					.map_err(|_| ProfileError::InvalidUtf8)?;

				let mut profile = self.profile.as_account_mut::<ProfileState>(&ID)?;
				profile.name = args.name;
				profile.bio = args.bio;

				log!("Profile updated");
			}
			ProfileInstruction::AddTag => {
				let args = AddTagInstruction::try_from_bytes(data)?;

				let mut profile = self.profile.as_account_mut::<ProfileState>(&ID)?;
				profile
					.tags
					.try_push(args.tag)
					.map_err(|_| ProfileError::TagOverflow)?;

				log!("Tag added");
			}
			ProfileInstruction::RemoveTag => {
				let args = RemoveTagInstruction::try_from_bytes(data)?;
				let index: u64 = args.index.into();

				let mut profile = self.profile.as_account_mut::<ProfileState>(&ID)?;
				let len = profile.tags.len();
				let index = usize::try_from(index).map_err(|_| ProfileError::TagNotFound)?;
				if index >= len {
					return Err(ProfileError::TagNotFound.into());
				}

				// Shift remaining tags left, then pop the (now duplicated)
				// last slot to decrement the length prefix.
				profile.tags.as_mut_slice().copy_within(index + 1.., index);
				profile.tags.pop();

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
			ProfileInstruction::Initialize => InitializeAccounts::try_from(accounts)?.process(data),
			ProfileInstruction::UpdateProfile
			| ProfileInstruction::AddTag
			| ProfileInstruction::RemoveTag => ProfileAccounts::try_from(accounts)?.process(data),
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
		// + 1 (active) = 231 bytes.
		assert_eq!(size_of::<ProfileState>(), 231);
	}

	#[test]
	fn profile_state_discriminator() {
		assert!(ProfileState::matches_discriminator(&[
			ProfileAccountType::ProfileState as u8
		]));
		assert!(!ProfileState::matches_discriminator(&[0u8]));
	}

	#[test]
	fn profile_state_builder() {
		let state = ProfileState::builder()
			.bump(42)
			.name(PodString::default())
			.bio(PodString::default())
			.tags(PodVec::default())
			.active(PodBool::from_bool(true))
			.build();
		assert_eq!(state.bump, 42);
		assert!(state.name.is_empty());
		assert!(state.tags.is_empty());
		assert!(bool::from(state.active));
	}

	#[test]
	fn pod_string_roundtrip() {
		let mut name = PodString::<32>::default();
		assert!(name.try_set("alice").is_ok());
		assert_eq!(
			name.try_as_str().unwrap_or_else(|e| panic!("{e:?}")),
			"alice"
		);
		assert_eq!(name.len(), 5);
		assert_eq!(name.capacity(), 32);
	}

	#[test]
	fn pod_string_rejects_invalid_utf8() {
		let mut name = PodString::<32>::default();
		// 0xff is not valid UTF-8
		assert!(name.try_set("\u{FFFD}").is_ok()); // replacement char is valid
		// Direct byte-level corruption: set the length to 1 and write 0xff
		let mut raw = [0u8; 33];
		raw[0] = 1;
		raw[1] = 0xff;
		let corrupted =
			bytemuck::try_from_bytes::<PodString<32>>(&raw).unwrap_or_else(|e| panic!("{e:?}"));
		assert!(corrupted.try_as_str().is_err());
	}

	#[test]
	fn pod_string_capacity_overflow() {
		let mut name = PodString::<32>::default();
		let long = "x".repeat(33);
		assert!(name.try_set(&long).is_err());
	}

	#[test]
	fn pod_vec_roundtrip() {
		let mut tags = PodVec::<PodU64, 8>::default();
		assert!(tags.try_push(PodU64::from_primitive(1)).is_ok());
		assert!(tags.try_push(PodU64::from_primitive(2)).is_ok());
		assert_eq!(tags.len(), 2);
		assert_eq!(
			u64::from(
				tags.get(0)
					.copied()
					.unwrap_or_else(|| PodU64::from_primitive(0))
			),
			1
		);
		assert_eq!(
			u64::from(
				tags.get(1)
					.copied()
					.unwrap_or_else(|| PodU64::from_primitive(0))
			),
			2
		);
		assert_eq!(tags.pop(), Some(PodU64::from_primitive(2)));
		assert_eq!(tags.len(), 1);
	}

	#[test]
	fn pod_vec_capacity_overflow() {
		let mut tags = PodVec::<PodU64, 8>::default();
		for i in 0..8 {
			assert!(tags.try_push(PodU64::from_primitive(i)).is_ok());
		}
		assert!(tags.try_push(PodU64::from_primitive(8)).is_err());
	}

	#[test]
	fn initialize_instruction_data_layout() {
		// 1 (discriminator) + 1 (bump) + 33 (name) + 129 (bio) = 164 bytes.
		assert_eq!(size_of::<InitializeInstruction>(), 164);
		assert!(InitializeInstruction::matches_discriminator(&[
			ProfileInstruction::Initialize as u8
		]));
	}

	#[test]
	fn update_profile_instruction_data_layout() {
		// 1 (discriminator) + 33 (name) + 129 (bio) = 163 bytes.
		assert_eq!(size_of::<UpdateProfileInstruction>(), 163);
	}

	#[test]
	fn add_tag_instruction_data_layout() {
		// 1 (discriminator) + 8 (tag) = 9 bytes.
		assert_eq!(size_of::<AddTagInstruction>(), 9);
	}

	#[test]
	fn remove_tag_instruction_data_layout() {
		// 1 (discriminator) + 8 (index) = 9 bytes.
		assert_eq!(size_of::<RemoveTagInstruction>(), 9);
	}

	#[test]
	fn initialize_instruction_try_from_bytes() {
		let mut data = std::vec![ProfileInstruction::Initialize as u8, 42u8];
		data.extend_from_slice(&[3u8, b'a', b'l', b'i']); // len=3, "ali"
		data.extend_from_slice(&[0u8; 29]); // remaining name capacity
		data.extend_from_slice(&[0u8; 129]); // empty bio
		let ix = InitializeInstruction::try_from_bytes(&data)
			.unwrap_or_else(|e| panic!("failed: {e:?}"));
		assert_eq!(ix.bump, 42);
		assert_eq!(
			ix.name.try_as_str().unwrap_or_else(|e| panic!("{e:?}")),
			"ali"
		);
	}

	#[test]
	fn profile_seeds_macro() {
		let authority = [1u8; 32];
		let seeds = profile_seeds!(&authority);
		assert_eq!(seeds.len(), 2);
		assert_eq!(seeds[0], b"profile");
		assert_eq!(seeds[1], &authority);
	}

	#[test]
	fn profile_seeds_with_bump_macro() {
		let authority = [1u8; 32];
		let seeds = profile_seeds!(&authority, 42);
		assert_eq!(seeds.len(), 3);
		assert_eq!(seeds[0], b"profile");
		assert_eq!(seeds[1], &authority);
		assert_eq!(seeds[2], &[42u8]);
	}

	#[test]
	fn program_id_is_valid() {
		assert_ne!(ID, Address::default());
	}
}
