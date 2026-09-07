//! Anchor `realloc` parity example ported to pina.
//!
//! The original Anchor fixture creates one global `sample` PDA. This secure
//! adaptation deliberately derives one sample per authority instead. A resize
//! request must therefore prove ownership through both the signer and the
//! canonical `[b"sample", authority]` PDA; it cannot resize an arbitrary
//! program-owned account merely by presenting it as writable.

#![allow(clippy::inline_always)]
#![expect(
	clippy::len_without_is_empty,
	reason = "zeropod generates len accessors for scalar wire fields, not collections"
)]
#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use core::mem::size_of;

use pina::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

const SEED_SAMPLE: &[u8] = b"sample";

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReallocError {
	AccountReallocExceedsLimit = 3016,
	AccountDuplicateReallocs = 3017,
	AccountDataTooSmall = 3018,
	AuthorityMismatch = 3019,
}

#[discriminator]
pub enum ReallocInstruction {
	Realloc = 0,
	Realloc2 = 1,
	Initialize = 2,
}

#[discriminator]
pub enum ReallocAccountType {
	Sample = 1,
}

/// A compact account whose active values occupy only the bytes they need.
#[account(discriminator = ReallocAccountType, compact)]
#[pda(seeds = [SEED_SAMPLE, authority: Address], bump = bump)]
pub struct Sample {
	/// Canonical PDA bump, persisted for inexpensive validation on resize.
	pub bump: u8,
	/// The only signer permitted to resize this sample.
	pub authority: Address,
	/// Dynamically encoded values; unused capacity occupies no account bytes.
	pub values: Vec<u64, 64>,
}

/// Creates the per-authority sample PDA.
#[instruction(discriminator = ReallocInstruction::Initialize)]
pub struct InitializeIx {
	/// The precomputed canonical PDA bump.
	pub bump: u8,
}

/// Resizes the complete account-data buffer to `len` bytes.
///
/// `len` must equal `Sample::projected_bytes` for an active value count.
#[instruction(discriminator = ReallocInstruction::Realloc)]
pub struct ReallocIx {
	pub len: u16,
}

/// Exercises Anchor's duplicate-reallocation guard.
///
/// Both sample accounts must be the same canonical PDA for the signer, so the
/// instruction always rejects with `AccountDuplicateReallocs` before any
/// account is resized. It is intentionally not a two-target mutation API.
#[instruction(discriminator = ReallocInstruction::Realloc2)]
pub struct Realloc2Ix {
	pub len: u16,
}

#[derive(Accounts, Debug)]
pub struct InitializeAccounts<'a> {
	/// Funds creation and becomes the sample's resize authority.
	pub authority: &'a mut AccountView,
	/// Empty PDA derived from `[b"sample", authority]`.
	pub sample: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct ReallocAccounts<'a> {
	/// The sample authority. It pays rent on growth and receives excess rent on
	/// shrink, so it must be writable as well as a signer.
	pub authority: &'a mut AccountView,
	pub sample: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct Realloc2Accounts<'a> {
	pub authority: &'a mut AccountView,
	// These remain immutable Rust borrows because this instruction intentionally
	// rejects duplicate targets before mutation. The explicit writable checks in
	// `validate_sample` preserve the on-chain and IDL constraint while allowing
	// the duplicate-account regression to reach program logic.
	pub sample1: &'a AccountView,
	pub sample2: &'a AccountView,
	pub system_program: &'a AccountView,
}

fn validate_realloc_delta(current_len: usize, target_len: usize) -> ProgramResult {
	if target_len > current_len {
		let delta = target_len - current_len;

		if delta > MAX_PERMITTED_DATA_INCREASE {
			return Err(ReallocError::AccountReallocExceedsLimit.into());
		}
	}

	Ok(())
}

fn target_values_count(target_len: usize) -> Result<usize, ProgramError> {
	let tail_len = target_len
		.checked_sub(Sample::MIN_SIZE)
		.ok_or(ReallocError::AccountDataTooSmall)?;

	if !tail_len.is_multiple_of(size_of::<PodU64>()) {
		return Err(ReallocError::AccountDataTooSmall.into());
	}

	let values_count = tail_len / size_of::<PodU64>();
	let projected_size = Sample::projected_bytes(values_count)
		.map_err(|_| ProgramError::from(ReallocError::AccountDataTooSmall))?;

	if projected_size != target_len {
		return Err(ReallocError::AccountDataTooSmall.into());
	}

	Ok(values_count)
}

fn validate_distinct_realloc_targets(account1: &Address, account2: &Address) -> ProgramResult {
	if account1 == account2 {
		return Err(ReallocError::AccountDuplicateReallocs.into());
	}

	Ok(())
}

fn validate_sample(sample: AccountView, authority: &Address) -> ProgramResult {
	sample
		.assert_not_empty()?
		.assert_writable()?
		.assert_owner(&ID)?;

	let (bump, stored_authority) =
		sample.with_compact_account::<Sample, _>(&ID, |state| Ok((state.bump, state.authority)))?;

	let seeds = Sample::seeds(authority);
	let canonical_bump = sample.assert_canonical_bump(&seeds.as_slices(), &ID)?;
	if canonical_bump != bump {
		return Err(ProgramError::InvalidSeeds);
	}
	Sample::assert_seeds(&sample, authority, &ID)?;

	// The PDA check is the primary authority control. Retain the stored value as
	// defense in depth against accidental writes from future program instructions.
	if stored_authority != *authority {
		return Err(ReallocError::AuthorityMismatch.into());
	}

	Ok(())
}

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = InitializeIx::try_from_bytes(data)?;
		let authority_key = *self.authority.address();
		let seeds = Sample::seeds(&authority_key);
		let seeds_with_bump = seeds.with_bump(args.bump);

		self.authority.assert_signer()?.assert_writable()?;
		let canonical_bump = self.sample.assert_canonical_bump(&seeds.as_slices(), &ID)?;
		if canonical_bump != args.bump {
			return Err(ProgramError::InvalidSeeds);
		}
		self.sample
			.assert_empty()?
			.assert_writable()?
			.assert_seeds_with_bump(&seeds_with_bump.as_slices(), &ID)?;
		self.system_program.assert_address(&system::ID)?;

		CreateCompactProgramAccountWithBump {
			account: self.sample,
			payer: self.authority,
			owner: &ID,
			seeds: &seeds.as_slices(),
			bump: args.bump,
			space: Sample::MIN_SIZE,
		}
		.invoke::<Sample>()?;

		self.sample
			.with_compact_account_mut::<Sample, _>(&ID, |sample| {
				sample.bump = args.bump;
				sample.authority = authority_key;
				Ok(())
			})?;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for ReallocAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = ReallocIx::try_from_bytes(data)?;
		let target_len = usize::from(args.len.get());
		let authority_key = *self.authority.address();

		self.authority.assert_signer()?.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;
		validate_sample(*self.sample, &authority_key)?;
		let count = target_values_count(target_len)?;
		validate_realloc_delta(self.sample.data_len(), target_len)?;

		let mut values = [PodU64::from(0); Sample::VALUES_CAPACITY];
		for (index, value) in values.iter_mut().take(count).enumerate() {
			value.set(u64::try_from(index).map_err(|_| ProgramError::InvalidArgument)?);
		}

		ResizeCompactAccount {
			account: self.sample,
			rent_account: self.authority,
			target_size: target_len,
			program_id: &ID,
		}
		.invoke::<Sample, _>(|data| {
			let mut sample = Sample::try_from_bytes_mut(data)?;
			sample
				.set_values(&values[..count])
				.map_err(|_| ProgramError::InvalidAccountData)?;
			sample
				.commit()
				.map_err(|_| ProgramError::InvalidAccountData)?;

			Ok(())
		})?;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for Realloc2Accounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		// Realloc2 is only a duplicate-target regression. Its legacy `len` field
		// remains in the wire format but must not reintroduce a second mutation path.
		let _ = Realloc2Ix::try_from_bytes(data)?;
		let authority_key = *self.authority.address();

		self.authority.assert_signer()?.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;
		// Keep this direct so the IDL records the mutable-account constraint even
		// though these are immutable Rust borrows for duplicate alias safety.
		self.sample1.assert_writable()?;
		self.sample2.assert_writable()?;
		validate_sample(*self.sample1, &authority_key)?;
		validate_sample(*self.sample2, &authority_key)?;

		validate_distinct_realloc_targets(self.sample1.address(), self.sample2.address())
	}
}

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
		let instruction: ReallocInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			ReallocInstruction::Initialize => {
				InitializeAccounts::try_from((program_id, accounts))?.process(data)
			}
			ReallocInstruction::Realloc => {
				ReallocAccounts::try_from((program_id, accounts))?.process(data)
			}
			ReallocInstruction::Realloc2 => {
				Realloc2Accounts::try_from((program_id, accounts))?.process(data)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let wrong_program_id: Address = [5u8; 32].into();
		let data = [ReallocInstruction::Realloc as u8];
		let result = parse_instruction::<ReallocInstruction>(&wrong_program_id, &ID, &data);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}

	#[test]
	fn realloc_instruction_roundtrip() {
		let mut bytes = [0u8; ReallocIx::SIZE];
		let ix = ReallocIx::initialize(&mut bytes).unwrap_or_else(|e| panic!("encode: {e:?}"));
		ix.len.set(Sample::MIN_SIZE as u16);
		let parsed = ReallocIx::try_from_bytes(&bytes).unwrap_or_else(|e| panic!("decode: {e:?}"));
		assert_eq!(usize::from(parsed.len.get()), Sample::MIN_SIZE);
	}

	#[test]
	fn sample_pda_is_authority_bound() {
		let authority: Address = [1u8; 32].into();
		let attacker: Address = [2u8; 32].into();
		let (authority_sample, _) = Sample::find_pda(&authority, &ID);
		let (attacker_sample, _) = Sample::find_pda(&attacker, &ID);

		assert_ne!(authority_sample, attacker_sample);
	}

	#[test]
	fn validate_realloc_delta_allows_small_growth() {
		assert!(validate_realloc_delta(100, 200).is_ok());
		assert!(validate_realloc_delta(200, 100).is_ok());
	}

	#[test]
	fn validate_realloc_delta_rejects_growth_beyond_limit() {
		let result = validate_realloc_delta(100, 100 + MAX_PERMITTED_DATA_INCREASE + 1);
		assert!(matches!(
			result,
			Err(ProgramError::Custom(code)) if code == ReallocError::AccountReallocExceedsLimit as u32
		));
	}

	#[test]
	fn target_values_count_rejects_truncating_the_sample_header() {
		let result = target_values_count(Sample::MIN_SIZE - 1);
		assert!(matches!(
			result,
			Err(ProgramError::Custom(code)) if code == ReallocError::AccountDataTooSmall as u32
		));
	}

	#[test]
	fn target_values_count_uses_generated_size_boundaries_and_capacity() {
		let three_values =
			Sample::projected_bytes(3).unwrap_or_else(|error| panic!("project size: {error:?}"));
		assert_eq!(target_values_count(Sample::MIN_SIZE), Ok(0));
		assert_eq!(target_values_count(three_values), Ok(3));

		for target in [Sample::MIN_SIZE + 1, Sample::MAX_SIZE + size_of::<PodU64>()] {
			let result = target_values_count(target);
			assert!(matches!(
				result,
				Err(ProgramError::Custom(code))
					if code == ReallocError::AccountDataTooSmall as u32
			));
		}
	}

	#[test]
	fn sample_compact_codec_roundtrips_active_values() {
		let target_size =
			Sample::projected_bytes(3).unwrap_or_else(|error| panic!("project size: {error:?}"));
		let mut backing = [0u8; Sample::MAX_SIZE];
		let data = &mut backing[..target_size];
		let values = [PodU64::from(3), PodU64::from(5), PodU64::from(8)];
		let encoded_size = {
			let mut sample = Sample::initialize(&mut *data)
				.unwrap_or_else(|error| panic!("initialize: {error:?}"));
			assert_eq!(sample.encoded_size(), Sample::MIN_SIZE);
			sample.bump = 7;
			sample.authority = Address::new_from_array([9; 32]);
			sample
				.set_values(&values)
				.unwrap_or_else(|error| panic!("set values: {error:?}"));
			assert_eq!(sample.projected_size(), target_size);
			let encoded_size = sample
				.commit()
				.unwrap_or_else(|error| panic!("commit: {error:?}"));
			assert_eq!(sample.encoded_size(), encoded_size);
			encoded_size
		};

		assert_eq!(encoded_size, target_size);
		let sample =
			Sample::try_from_bytes(&*data).unwrap_or_else(|error| panic!("decode: {error:?}"));
		assert_eq!(sample.encoded_size(), target_size);
		assert_eq!(sample.bump, 7);
		assert_eq!(sample.authority, Address::new_from_array([9; 32]));
		assert_eq!(sample.values(), values);
	}

	#[test]
	fn validate_distinct_realloc_targets_rejects_duplicates() {
		let same: Address = [2u8; 32].into();
		let result = validate_distinct_realloc_targets(&same, &same);
		assert!(matches!(
			result,
			Err(ProgramError::Custom(code)) if code == ReallocError::AccountDuplicateReallocs as u32
		));
	}

	#[test]
	fn validate_distinct_realloc_targets_accepts_distinct_accounts() {
		let first: Address = [2u8; 32].into();
		let second: Address = [3u8; 32].into();
		assert!(validate_distinct_realloc_targets(&first, &second).is_ok());
	}
}
