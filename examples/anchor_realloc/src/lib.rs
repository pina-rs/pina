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

use pina::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

const SAMPLE_SEED: &[u8] = b"sample";

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

/// The authenticated header at the start of every resizable sample account.
///
/// Bytes after this fixed header are deliberately not exposed as a typed
/// collection. The example tests allocation and rent behaviour, not a data
/// serialization format; only the header participates in program logic.
#[account(discriminator = ReallocAccountType)]
#[pda(seeds = [SAMPLE_SEED, authority: Address], bump = bump)]
pub struct Sample {
	/// Canonical PDA bump, persisted for inexpensive validation on resize.
	pub bump: u8,
	/// The only signer permitted to resize this sample.
	pub authority: Address,
}

/// Creates the per-authority sample PDA.
#[instruction(discriminator = ReallocInstruction::Initialize)]
pub struct InitializeIx {
	/// The precomputed canonical PDA bump.
	pub bump: u8,
}

/// Resizes the complete account-data buffer to `len` bytes.
///
/// `len` includes the fixed [`Sample`] header and therefore cannot be smaller
/// than [`Sample::SIZE`].
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

fn validate_realloc_delta(current_len: usize, new_len: usize) -> ProgramResult {
	if new_len > current_len {
		let delta = new_len - current_len;

		if delta > MAX_PERMITTED_DATA_INCREASE {
			return Err(ReallocError::AccountReallocExceedsLimit.into());
		}
	}

	Ok(())
}

fn validate_target_len(target_len: usize) -> ProgramResult {
	if target_len < Sample::SIZE {
		return Err(ReallocError::AccountDataTooSmall.into());
	}

	Ok(())
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

	// `Sample` is a fixed header followed by untyped realloc capacity. Pina's
	// `assert_type` deliberately requires an exact account length, which is the
	// right default for fixed accounts but would reject every successful resize.
	// Validate only the fixed prefix here; slicing it to `Sample::SIZE` keeps
	// zeropod's checked view bounded to the declared header.
	let data = sample.try_borrow()?;
	let header = data
		.get(..Sample::SIZE)
		.ok_or(ProgramError::AccountDataTooSmall)?;
	if !Sample::matches_discriminator(header) {
		return Err(ProgramError::InvalidAccountData);
	}
	<Sample as ZeroPodFixed>::validate(header).map_err(|_| ProgramError::InvalidAccountData)?;
	let state = <Sample as ZeroPodFixed>::from_bytes(header)
		.map_err(|_| ProgramError::InvalidAccountData)?;
	let bump = state.bump;
	let stored_authority = state.authority;
	drop(data);

	let seeds = Sample::seeds(authority);
	let canonical_bump = sample.assert_canonical_bump(&seeds.as_slices(), &ID)?;
	if canonical_bump != bump {
		return Err(ProgramError::InvalidSeeds);
	}
	let seeds_with_bump = seeds.with_bump(bump);
	sample.assert_seeds_with_bump(&seeds_with_bump.as_slices(), &ID)?;

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

		CreateProgramAccountWithBump {
			account: self.sample,
			payer: self.authority,
			owner: &ID,
			seeds: &seeds.as_slices(),
			bump: args.bump,
		}
		.invoke::<Sample>()?;

		let mut sample = self.sample.as_account_mut::<Sample>(&ID)?;
		sample.bump = args.bump;
		sample.authority = authority_key;

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
		validate_target_len(target_len)?;
		validate_realloc_delta(self.sample.data_len(), target_len)?;

		ReallocAccount {
			account: self.sample,
			payer: self.authority,
			new_size: target_len,
			program_id: &ID,
		}
		.invoke()
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
		ix.len.set(Sample::SIZE as u16);
		let parsed = ReallocIx::try_from_bytes(&bytes).unwrap_or_else(|e| panic!("decode: {e:?}"));
		assert_eq!(usize::from(parsed.len.get()), Sample::SIZE);
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
	fn validate_target_len_rejects_truncating_the_sample_header() {
		let result = validate_target_len(Sample::SIZE - 1);
		assert!(matches!(
			result,
			Err(ProgramError::Custom(code)) if code == ReallocError::AccountDataTooSmall as u32
		));
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
}
