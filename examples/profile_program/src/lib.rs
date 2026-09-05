//! Profile program — demonstrates wire-compatible, fully initialized bounded
//! fields in on-chain account state.
//!
//! Pina's macro-generated zero-copy boundary intentionally accepts only
//! storage whose complete backing bytes are always initialized. This example
//! retains the established string/vector wire layout with fixed byte arrays
//! and small checked semantic helpers:
//!
//! - **`[u8; 33]` / `[u8; 129]`** — one length byte followed by fully
//!   initialized UTF-8 capacity.
//! - **`[u8; 66]`** — a two-byte little-endian count followed by eight
//!   little-endian `u64` slots.
//! - **`Option<T>`** — fixed-size optional data backed by `PodOption` in the
//!   generated zero-copy view. Used here for an optional favourite tag.
//! - **`PodBool`** — a single-byte boolean for the `active` flag.
//!
//! Every mutation writes a fully initialized array, so reading the complete
//! account backing slice remains sound. Semantic helpers validate lengths and
//! UTF-8 before exposing values.
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
	/// A bounded string field contained invalid UTF-8.
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
/// - `PinaAccount` and zeropod validation for checked zero-copy access.
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
/// | 2      | 33   | bounded name bytes   |
/// | 35     | 129  | bounded bio bytes    |
/// | 164    | 66   | bounded tag bytes    |
/// | 230    | 9    | favorite_tag (PodOption<PodU64>) |
/// | 239    | 1    | active (PodBool) |
/// ```
#[account(discriminator = ProfileAccountType)]
#[pda(seeds = [SEED_PROFILE, authority: Address], bump = bump)]
pub struct ProfileState {
	/// The PDA bump seed, stored on-chain so we don't need to re-derive it.
	pub bump: u8,
	/// One length byte followed by 32 fully initialized UTF-8 bytes.
	pub name: [u8; 33],
	/// One length byte followed by 128 fully initialized UTF-8 bytes.
	pub bio: [u8; 129],
	/// A two-byte count followed by eight little-endian `u64` slots.
	pub tags: [u8; 66],
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
/// Contains the PDA bump seed and fixed-width encodings of the initial name and
/// bio. The name occupies 33 bytes and the bio occupies 129 bytes. Each field
/// starts with a one-byte payload length, followed by its UTF-8 payload and
/// zero padding through the end of the field.
#[instruction(discriminator = ProfileInstruction, variant = Initialize)]
pub struct InitializeInstruction {
	/// The PDA bump seed, computed off-chain.
	pub bump: u8,
	/// The initial display name (length byte plus 32-byte capacity).
	pub name: [u8; 33],
	/// The initial bio (length byte plus 128-byte capacity).
	pub bio: [u8; 129],
}

/// Instruction data for `UpdateProfile`. Replaces both name and bio.
#[instruction(discriminator = ProfileInstruction, variant = UpdateProfile)]
pub struct UpdateProfileInstruction {
	/// The new display name (length byte plus 32-byte capacity).
	pub name: [u8; 33],
	/// The new bio (length byte plus 128-byte capacity).
	pub bio: [u8; 129],
}

const TAG_CAPACITY: usize = 8;
const TAG_PREFIX_BYTES: usize = 2;
const TAG_BYTES: usize = size_of::<u64>();
const TAG_FIELD_BYTES: usize = TAG_PREFIX_BYTES + TAG_CAPACITY * TAG_BYTES;

/// Encode UTF-8 into a fully initialized, one-byte-length-prefixed field.
///
/// `N` includes the prefix byte. The remaining `N - 1` bytes are the maximum
/// payload capacity and unused capacity is always zeroed.
///
/// # Errors
///
/// Returns [`ProgramError::InvalidInstructionData`] when `N` has no prefix
/// byte, or when `value` cannot fit in the one-byte length or fixed capacity.
pub fn encode_bounded_text<const N: usize>(value: &str) -> Result<[u8; N], ProgramError> {
	let capacity = N
		.checked_sub(1)
		.ok_or(ProgramError::InvalidInstructionData)?;
	let length = u8::try_from(value.len()).map_err(|_| ProgramError::InvalidInstructionData)?;

	if value.len() > capacity {
		return Err(ProgramError::InvalidInstructionData);
	}

	let mut bytes = [0u8; N];
	bytes[0] = length;
	let value_end = 1 + value.len();
	bytes[1..value_end].copy_from_slice(value.as_bytes());

	Ok(bytes)
}

fn bounded_text(bytes: &[u8]) -> Result<&str, ProgramError> {
	let Some((&length, capacity)) = bytes.split_first() else {
		return Err(ProgramError::InvalidInstructionData);
	};
	let length = usize::from(length);

	if length > capacity.len() {
		return Err(ProgramError::InvalidInstructionData);
	}

	core::str::from_utf8(&capacity[..length]).map_err(|_| ProfileError::InvalidUtf8.into())
}

fn tag_count(bytes: &[u8; TAG_FIELD_BYTES]) -> Result<usize, ProgramError> {
	let count = usize::from(u16::from_le_bytes([bytes[0], bytes[1]]));

	if count > TAG_CAPACITY {
		return Err(ProgramError::InvalidAccountData);
	}

	Ok(count)
}

fn tag_range(index: usize) -> core::ops::Range<usize> {
	let start = TAG_PREFIX_BYTES + index * TAG_BYTES;
	start..start + TAG_BYTES
}

impl ProfileStateZc {
	/// Return the validated profile name.
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidAccountData`] when the stored length or
	/// UTF-8 payload is invalid.
	pub fn name_text(&self) -> Result<&str, ProgramError> {
		bounded_text(&self.name).map_err(|_| ProgramError::InvalidAccountData)
	}

	/// Return the validated profile bio.
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidAccountData`] when the stored length or
	/// UTF-8 payload is invalid.
	pub fn bio_text(&self) -> Result<&str, ProgramError> {
		bounded_text(&self.bio).map_err(|_| ProgramError::InvalidAccountData)
	}

	/// Replace the profile name and zero every inactive capacity byte.
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidInstructionData`] when `value` exceeds the
	/// fixed name capacity.
	pub fn write_name_text(&mut self, value: &str) -> ProgramResult {
		self.name = encode_bounded_text(value)?;
		Ok(())
	}

	/// Replace the profile bio and zero every inactive capacity byte.
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidInstructionData`] when `value` exceeds the
	/// fixed bio capacity.
	pub fn write_bio_text(&mut self, value: &str) -> ProgramResult {
		self.bio = encode_bounded_text(value)?;
		Ok(())
	}

	/// Return the number of active tags after validating the stored count.
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidAccountData`] when the stored count exceeds
	/// `TAG_CAPACITY`.
	pub fn tag_count(&self) -> Result<usize, ProgramError> {
		tag_count(&self.tags)
	}

	/// Return a copied tag value, or `None` when `index` is out of range.
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidAccountData`] when the stored count exceeds
	/// `TAG_CAPACITY`.
	pub fn tag(&self, index: usize) -> Result<Option<u64>, ProgramError> {
		if index >= self.tag_count()? {
			return Ok(None);
		}

		let range = tag_range(index);
		let bytes: [u8; TAG_BYTES] = self.tags[range]
			.try_into()
			.map_err(|_| ProgramError::InvalidAccountData)?;

		Ok(Some(u64::from_le_bytes(bytes)))
	}

	/// Reset the tag field to its canonical, fully initialized empty state.
	pub fn clear_tags(&mut self) {
		self.tags.fill(0);
	}

	/// Append a tag without ever creating uninitialized backing bytes.
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidAccountData`] for an invalid stored count,
	/// or [`ProfileError::TagOverflow`] when all tag slots are active.
	pub fn push_tag(&mut self, value: u64) -> ProgramResult {
		let count = self.tag_count()?;

		if count == TAG_CAPACITY {
			return Err(ProfileError::TagOverflow.into());
		}

		let next_count = u16::try_from(count + 1).map_err(|_| ProgramError::InvalidAccountData)?;
		self.tags[tag_range(count)].copy_from_slice(&value.to_le_bytes());
		self.tags[..TAG_PREFIX_BYTES].copy_from_slice(&next_count.to_le_bytes());

		Ok(())
	}

	/// Remove a tag while preserving the established contiguous wire layout.
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidAccountData`] for an invalid stored count,
	/// or [`ProfileError::TagNotFound`] when `index` is not active.
	pub fn remove_tag(&mut self, index: usize) -> ProgramResult {
		let count = self.tag_count()?;

		if index >= count {
			return Err(ProfileError::TagNotFound.into());
		}

		let destination = tag_range(index).start;
		let source = tag_range(index + 1).start;
		let active_end = tag_range(count).start;
		let next_count = u16::try_from(count - 1).map_err(|_| ProgramError::InvalidAccountData)?;
		self.tags.copy_within(source..active_end, destination);
		self.tags[tag_range(count - 1)].fill(0);
		self.tags[..TAG_PREFIX_BYTES].copy_from_slice(&next_count.to_le_bytes());

		Ok(())
	}
}

impl InitializeInstructionZc {
	/// Return the validated initial profile name.
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidInstructionData`] when the encoded length
	/// exceeds the fixed capacity, or [`ProfileError::InvalidUtf8`] when the
	/// active payload is not valid UTF-8.
	pub fn name_text(&self) -> Result<&str, ProgramError> {
		bounded_text(&self.name)
	}

	/// Return the validated initial profile bio.
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidInstructionData`] when the encoded length
	/// exceeds the fixed capacity, or [`ProfileError::InvalidUtf8`] when the
	/// active payload is not valid UTF-8.
	pub fn bio_text(&self) -> Result<&str, ProgramError> {
		bounded_text(&self.bio)
	}

	/// Encode an initial profile name into fully initialized storage.
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidInstructionData`] when `value` exceeds the
	/// fixed name capacity.
	pub fn write_name_text(&mut self, value: &str) -> ProgramResult {
		self.name = encode_bounded_text(value)?;
		Ok(())
	}

	/// Encode an initial profile bio into fully initialized storage.
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidInstructionData`] when `value` exceeds the
	/// fixed bio capacity.
	pub fn write_bio_text(&mut self, value: &str) -> ProgramResult {
		self.bio = encode_bounded_text(value)?;
		Ok(())
	}
}

impl UpdateProfileInstructionZc {
	/// Return the validated replacement profile name.
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidInstructionData`] when the encoded length
	/// exceeds the fixed capacity, or [`ProfileError::InvalidUtf8`] when the
	/// active payload is not valid UTF-8.
	pub fn name_text(&self) -> Result<&str, ProgramError> {
		bounded_text(&self.name)
	}

	/// Return the validated replacement profile bio.
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidInstructionData`] when the encoded length
	/// exceeds the fixed capacity, or [`ProfileError::InvalidUtf8`] when the
	/// active payload is not valid UTF-8.
	pub fn bio_text(&self) -> Result<&str, ProgramError> {
		bounded_text(&self.bio)
	}

	/// Encode a replacement profile name into fully initialized storage.
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidInstructionData`] when `value` exceeds the
	/// fixed name capacity.
	pub fn write_name_text(&mut self, value: &str) -> ProgramResult {
		self.name = encode_bounded_text(value)?;
		Ok(())
	}

	/// Encode a replacement profile bio into fully initialized storage.
	///
	/// # Errors
	///
	/// Returns [`ProgramError::InvalidInstructionData`] when `value` exceeds the
	/// fixed bio capacity.
	pub fn write_bio_text(&mut self, value: &str) -> ProgramResult {
		self.bio = encode_bounded_text(value)?;
		Ok(())
	}
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

		// Validate UTF-8 before storing anything on-chain (boundary validation
		// already guarantees this, but keep the explicit check for clarity).
		let _ = args.name_text()?;
		let _ = args.bio_text()?;

		// Create the PDA account
		CreateProgramAccountWithBump {
			account: self.profile,
			payer: self.authority,
			owner: &ID,
			seeds: &seeds.as_slices(),
			bump: args.bump,
		}
		.invoke::<ProfileState>()?;

		// Initialize account data
		let mut profile = self.profile.as_account_mut::<ProfileState>(&ID)?;
		// These fixed arrays came from initialized instruction bytes and retain the
		// established bounded-field wire format without importing uninitialized
		// inactive collection capacity.
		profile.bump = args.bump;
		profile.name = args.name;
		profile.bio = args.bio;
		profile.clear_tags();
		profile.favorite_tag.clear();
		profile.active.set(true);

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

		// Verify the profile is the PDA for the authority, using the stored
		// bump field (avoids re-deriving the canonical bump on-chain).
		ProfileState::assert_seeds(self.profile, authority_key, &ID)?;

		// Dispatch on the instruction discriminator
		let instruction = ProfileInstruction::try_from(
			*data.first().ok_or(ProgramError::InvalidInstructionData)?,
		)
		.map_err(|_| ProgramError::InvalidInstructionData)?;

		match instruction {
			ProfileInstruction::UpdateProfile => {
				let args = UpdateProfileInstruction::try_from_bytes(data)?;
				let _ = args.name_text()?;
				let _ = args.bio_text()?;

				let mut profile = self.profile.as_account_mut::<ProfileState>(&ID)?;
				profile.name = args.name;
				profile.bio = args.bio;

				log!("Profile updated");
			}
			ProfileInstruction::AddTag => {
				let args = AddTagInstruction::try_from_bytes(data)?;

				let mut profile = self.profile.as_account_mut::<ProfileState>(&ID)?;
				profile.push_tag(args.tag.get())?;

				log!("Tag added");
			}
			ProfileInstruction::RemoveTag => {
				let args = RemoveTagInstruction::try_from_bytes(data)?;
				let index = args.index.get();

				let mut profile = self.profile.as_account_mut::<ProfileState>(&ID)?;
				let index = usize::try_from(index).map_err(|_| ProfileError::TagNotFound)?;
				profile.remove_tag(index)?;

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
		let state = ProfileState::initialize(&mut bytes).unwrap();
		state.bump = 42;
		state.active.set(true);
		assert_eq!(state.bump, 42);
		assert_eq!(state.name_text().unwrap(), "");
		assert_eq!(state.tag_count().unwrap(), 0);
		assert!(state.favorite_tag.is_none());
		assert!(state.active.get());
	}

	#[test]
	fn bounded_string_roundtrip() {
		let empty = encode_bounded_text::<33>("")
			.unwrap_or_else(|error| panic!("empty encoding failed: {error:?}"));
		let name = encode_bounded_text::<33>("alice")
			.unwrap_or_else(|error| panic!("encoding failed: {error:?}"));

		assert_eq!(empty, [0u8; 33]);
		assert_eq!(bounded_text(&name), Ok("alice"));
		assert!(name[6..].iter().all(|byte| *byte == 0));
	}

	#[test]
	fn bounded_string_rejects_invalid_utf8() {
		let mut bytes = [0u8; ProfileState::SIZE];
		let state = ProfileState::initialize(&mut bytes).unwrap();
		state.name[0] = 1;
		state.name[1] = 0xff;

		assert_eq!(state.name_text(), Err(ProgramError::InvalidAccountData));
	}

	#[test]
	fn bounded_string_rejects_length_over_capacity() {
		let mut name = [0u8; 33];
		name[0] = 33;

		assert_eq!(
			bounded_text(&name),
			Err(ProgramError::InvalidInstructionData)
		);
	}

	#[test]
	fn bounded_tags_roundtrip() {
		let mut bytes = [0u8; ProfileState::SIZE];
		let state = ProfileState::initialize(&mut bytes).unwrap();
		state.push_tag(1).unwrap();
		state.push_tag(2).unwrap();

		assert_eq!(state.tag_count(), Ok(2));
		assert_eq!(state.tag(0), Ok(Some(1)));
		assert_eq!(state.tag(1), Ok(Some(2)));
		state.remove_tag(0).unwrap();
		assert_eq!(state.tag_count(), Ok(1));
		assert_eq!(state.tag(0), Ok(Some(2)));
	}

	#[test]
	fn bounded_tags_reject_capacity_overflow() {
		let mut bytes = [0u8; ProfileState::SIZE];
		let state = ProfileState::initialize(&mut bytes).unwrap();
		for i in 0..8 {
			state.push_tag(i).unwrap();
		}

		assert_eq!(state.push_tag(8), Err(ProfileError::TagOverflow.into()));
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
		let initialized = InitializeInstruction::initialize(&mut data)
			.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
		initialized.bump = 42;
		initialized
			.write_name_text("ali")
			.unwrap_or_else(|error| panic!("name encoding failed: {error:?}"));
		let ix = InitializeInstruction::try_from_bytes(&data)
			.unwrap_or_else(|e| panic!("failed: {e:?}"));
		assert_eq!(ix.bump, 42);
		assert_eq!(ix.name_text(), Ok("ali"));
	}

	#[test]
	fn initialize_instruction_reports_invalid_utf8() {
		let mut data = [0u8; InitializeInstruction::SIZE];
		let initialized = InitializeInstruction::initialize(&mut data)
			.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
		initialized.name[0] = 1;
		initialized.name[1] = 0xff;

		assert_eq!(
			initialized.name_text(),
			Err(ProfileError::InvalidUtf8.into())
		);
	}

	#[test]
	fn semantic_mutations_preserve_valid_profile_storage() {
		let mut bytes = [0u8; ProfileState::SIZE];

		{
			let state = ProfileState::initialize(&mut bytes)
				.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
			state
				.write_name_text("alice")
				.unwrap_or_else(|error| panic!("name write failed: {error:?}"));
			state
				.write_bio_text("hello")
				.unwrap_or_else(|error| panic!("bio write failed: {error:?}"));
		}
		{
			let state = ProfileState::try_from_bytes(&bytes)
				.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
			assert_eq!(state.name_text(), Ok("alice"));
			assert_eq!(state.bio_text(), Ok("hello"));
		}

		{
			let state = ProfileState::try_from_bytes_mut(&mut bytes)
				.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
			state
				.push_tag(7)
				.unwrap_or_else(|error| panic!("tag push failed: {error:?}"));
			state.favorite_tag.set(Some(PodU64::from(7)));
			state.active.set(true);
		}
		{
			let state = ProfileState::try_from_bytes(&bytes)
				.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
			assert_eq!(state.tag_count(), Ok(1));
			assert_eq!(state.tag(0), Ok(Some(7)));
			assert_eq!(state.favorite_tag.get(), Some(PodU64::from(7)));
			assert!(state.active.get());
		}

		{
			let state = ProfileState::try_from_bytes_mut(&mut bytes)
				.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
			state
				.remove_tag(0)
				.unwrap_or_else(|error| panic!("tag removal failed: {error:?}"));
			state.clear_tags();
			state.favorite_tag.clear();
			state.active.set(false);
		}
		{
			let state = ProfileState::try_from_bytes(&bytes)
				.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
			assert_eq!(state.tag_count(), Ok(0));
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
