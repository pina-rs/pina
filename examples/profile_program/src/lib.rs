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

		// Validate accounts
		self.authority.assert_signer()?;
		self.profile.assert_empty()?.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;

		// Create the PDA account
		//
		// The seeds bind `authority` and this handler requires that authority to
		// sign, so a noncanonical bump could only duplicate the signer's own
		// profile. Every read and update requires the same signer and validates
		// through the stored bump, so the duplicate stays private to them.
		CreateProgramAccountWithUncheckedBump {
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
