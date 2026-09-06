//! A focused compact-account lifecycle example.
//!
//! [`Journal`] stores a fixed header and only the active bytes of its trailing
//! entries and markers. [`ResizeAccounts`] demonstrates the required operation ordering:
//! allocate before growing the tail, commit the new prefix before shrinking the
//! account, and skip reallocation when the encoded size does not change.

#![allow(clippy::inline_always)]
#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("85qGHkkBAdE61PZSNF9R6UYakqw8d5eonqi4jbFLaSTn");

const SEED_JOURNAL: &[u8] = b"compact-journal";

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactAccountError {
	CapacityExceeded = 7000,
	IndexOutOfBounds = 7001,
	AuthorityMismatch = 7002,
}

#[discriminator]
pub enum CompactInstruction {
	Initialize = 0,
	Resize = 1,
	Write = 2,
}

#[discriminator]
pub enum CompactAccountType {
	Journal = 1,
}

/// A compact account with two independently encoded dynamic fields.
///
/// The one-byte discriminator, bump, authority, revision, and two vector
/// prefixes always occupy [`Self::HEADER_SIZE`] bytes. Each active row adds an
/// eight-byte entry and a one-byte marker, up to [`Self::MAX_SIZE`].
#[account(discriminator = CompactAccountType, compact)]
#[pda(seeds = [SEED_JOURNAL, authority: Address], bump = bump)]
pub struct Journal {
	/// Canonical PDA bump.
	pub bump: u8,
	/// Signer permitted to mutate and fund this journal.
	pub authority: Address,
	/// Number of successful resize or write operations.
	pub revision: u32,
	/// Active entries. Unused capacity consumes no account bytes.
	pub entries: Vec<u64, 8>,
	/// One marker per entry, stored as a second compact tail.
	pub markers: PodVec<u8, 8, 8>,
}

pub const MAX_ENTRIES: usize = Journal::ENTRIES_CAPACITY;

#[instruction(discriminator = CompactInstruction::Initialize)]
pub struct InitializeIx {
	pub bump: u8,
	/// Initial number of active entries. They are filled with `0..entry_count`.
	pub entry_count: u8,
}

#[instruction(discriminator = CompactInstruction::Resize)]
pub struct ResizeIx {
	/// New logical length. Growth appends each new entry's index as its value.
	pub entry_count: u8,
}

#[instruction(discriminator = CompactInstruction::Write)]
pub struct WriteIx {
	pub index: u8,
	pub value: u64,
}

#[derive(Accounts, Debug)]
pub struct InitializeAccounts<'a> {
	/// Funds rent and becomes the journal authority.
	pub authority: &'a mut AccountView,
	/// Empty canonical journal PDA.
	pub journal: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct ResizeAccounts<'a> {
	/// Funds growth and receives the rent refund from shrinking.
	pub authority: &'a mut AccountView,
	pub journal: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct WriteAccounts<'a> {
	pub authority: &'a AccountView,
	pub journal: &'a mut AccountView,
}

fn validate_journal(journal: AccountView, authority: &Address) -> ProgramResult {
	journal
		.assert_not_empty()?
		.assert_writable()?
		.assert_owner(&ID)?;

	let (bump, stored_authority) = journal
		.with_compact_account::<Journal, _>(&ID, |state| Ok((state.bump, state.authority)))?;
	Journal::assert_seeds(&journal, authority, &ID)?;
	let canonical_bump =
		journal.assert_canonical_bump(&Journal::seeds(authority).as_slices(), &ID)?;
	if bump != canonical_bump {
		return Err(ProgramError::InvalidSeeds);
	}
	if stored_authority != *authority {
		return Err(CompactAccountError::AuthorityMismatch.into());
	}

	Ok(())
}

fn next_revision(revision: u32) -> Result<u32, ProgramError> {
	revision
		.checked_add(1)
		.ok_or(ProgramError::ArithmeticOverflow)
}

fn initialized_entries(entry_count: usize) -> [PodU64; MAX_ENTRIES] {
	let mut entries = [PodU64::ZERO; MAX_ENTRIES];
	for (index, entry) in entries.iter_mut().take(entry_count).enumerate() {
		entry.set(index as u64);
	}
	entries
}

fn initialized_markers(entry_count: usize) -> [u8; MAX_ENTRIES] {
	let mut markers = [0; MAX_ENTRIES];
	for (index, marker) in markers.iter_mut().take(entry_count).enumerate() {
		*marker = index as u8;
	}
	markers
}

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = InitializeIx::try_from_bytes(data)?;
		let entry_count = usize::from(args.entry_count);
		let space = Journal::projected_bytes(entry_count, entry_count)
			.map_err(|_| ProgramError::from(CompactAccountError::CapacityExceeded))?;
		let authority_key = *self.authority.address();
		let seeds = Journal::seeds(&authority_key);
		let seeds_with_bump = seeds.with_bump(args.bump);

		self.authority.assert_signer()?.assert_writable()?;
		let canonical_bump = self
			.journal
			.assert_canonical_bump(&seeds.as_slices(), &ID)?;
		if args.bump != canonical_bump {
			return Err(ProgramError::InvalidSeeds);
		}
		self.journal
			.assert_empty()?
			.assert_writable()?
			.assert_seeds_with_bump(&seeds_with_bump.as_slices(), &ID)?;
		self.system_program.assert_address(&system::ID)?;

		CreateCompactProgramAccountWithBump {
			account: self.journal,
			payer: self.authority,
			owner: &ID,
			seeds: &seeds.as_slices(),
			bump: args.bump,
			space,
		}
		.invoke::<Journal>()?;

		let entries = initialized_entries(entry_count);
		let markers = initialized_markers(entry_count);
		let encoded_size = {
			let mut data = self.journal.try_borrow_mut()?;
			let mut journal = Journal::try_from_bytes_mut(&mut data)?;
			journal.bump = args.bump;
			journal.authority = authority_key;
			journal.revision.set(0);
			journal
				.set_entries(&entries[..entry_count])
				.map_err(|_| ProgramError::InvalidAccountData)?;
			journal
				.set_markers(&markers[..entry_count])
				.map_err(|_| ProgramError::InvalidAccountData)?;
			journal
				.commit()
				.map_err(|_| ProgramError::InvalidAccountData)?
		};
		if encoded_size != space {
			return Err(ProgramError::InvalidAccountData);
		}

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for ResizeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = ResizeIx::try_from_bytes(data)?;
		let target_count = usize::from(args.entry_count);
		let target_size = Journal::projected_bytes(target_count, target_count)
			.map_err(|_| ProgramError::from(CompactAccountError::CapacityExceeded))?;
		let authority_key = *self.authority.address();

		self.authority.assert_signer()?.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;
		validate_journal(*self.journal, &authority_key)?;

		let (mut entries, current_count, revision) = self
			.journal
			.with_compact_account::<Journal, _>(&ID, |journal| {
				let current = journal.entries();
				let mut entries = [PodU64::ZERO; MAX_ENTRIES];
				entries[..current.len()].copy_from_slice(current);
				Ok((entries, current.len(), journal.revision.get()))
			})?;
		for (index, entry) in entries
			.iter_mut()
			.enumerate()
			.take(target_count)
			.skip(current_count)
		{
			entry.set(index as u64);
		}
		let markers = initialized_markers(target_count);

		if target_size > self.journal.data_len() {
			ReallocCompactAccount {
				account: self.journal,
				payer: self.authority,
				new_size: target_size,
				program_id: &ID,
			}
			.invoke::<Journal>()?;
		}

		let encoded_size = {
			let mut data = self.journal.try_borrow_mut()?;
			let mut journal = Journal::try_from_bytes_mut(&mut data)?;
			journal.revision.set(next_revision(revision)?);
			journal
				.set_entries(&entries[..target_count])
				.map_err(|_| ProgramError::InvalidAccountData)?;
			journal
				.set_markers(&markers[..target_count])
				.map_err(|_| ProgramError::InvalidAccountData)?;
			journal
				.commit()
				.map_err(|_| ProgramError::InvalidAccountData)?
		};
		if encoded_size != target_size {
			return Err(ProgramError::InvalidAccountData);
		}

		if target_size < self.journal.data_len() {
			ReallocCompactAccount {
				account: self.journal,
				payer: self.authority,
				new_size: target_size,
				program_id: &ID,
			}
			.invoke::<Journal>()?;
		}

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for WriteAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = WriteIx::try_from_bytes(data)?;
		let index = usize::from(args.index);
		let authority_key = *self.authority.address();

		self.authority.assert_signer()?;
		validate_journal(*self.journal, &authority_key)?;

		let (mut entries, len, revision) =
			self.journal
				.with_compact_account::<Journal, _>(&ID, |journal| {
					let current = journal.entries();
					if index >= current.len() {
						return Err(CompactAccountError::IndexOutOfBounds.into());
					}
					let mut entries = [PodU64::ZERO; MAX_ENTRIES];
					entries[..current.len()].copy_from_slice(current);
					Ok((entries, current.len(), journal.revision.get()))
				})?;
		entries[index].set(args.value.get());

		let encoded_size = {
			let mut data = self.journal.try_borrow_mut()?;
			let mut journal = Journal::try_from_bytes_mut(&mut data)?;
			journal.revision.set(next_revision(revision)?);
			journal
				.set_entries(&entries[..len])
				.map_err(|_| ProgramError::InvalidAccountData)?;
			journal
				.commit()
				.map_err(|_| ProgramError::InvalidAccountData)?
		};
		if encoded_size != Journal::projected_bytes(len, len)? {
			return Err(ProgramError::InvalidAccountData);
		}

		Ok(())
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
		let instruction: CompactInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			CompactInstruction::Initialize => {
				InitializeAccounts::try_from((program_id, accounts))?.process(data)
			}
			CompactInstruction::Resize => {
				ResizeAccounts::try_from((program_id, accounts))?.process(data)
			}
			CompactInstruction::Write => {
				WriteAccounts::try_from((program_id, accounts))?.process(data)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn size_formula_covers_empty_partial_and_full_accounts() {
		assert_eq!(Journal::HEADER_SIZE, 48);
		assert_eq!(Journal::MIN_SIZE, Journal::HEADER_SIZE);
		assert_eq!(Journal::MAX_SIZE, 120);
		assert_eq!(Journal::TAIL_ALIGNMENT, 1);
		assert_eq!(Journal::ENTRIES_CAPACITY, 8);
		assert_eq!(Journal::MARKERS_CAPACITY, 8);
		assert_eq!(Journal::projected_bytes(0, 0), Ok(Journal::MIN_SIZE));
		assert_eq!(
			Journal::projected_bytes(3, 3),
			Ok(Journal::HEADER_SIZE + 27)
		);
		assert_eq!(
			Journal::projected_bytes(MAX_ENTRIES, Journal::MARKERS_CAPACITY),
			Ok(Journal::MAX_SIZE)
		);
	}

	#[test]
	fn size_formula_rejects_each_count_past_its_capacity() {
		assert_eq!(
			Journal::projected_bytes(MAX_ENTRIES + 1, 0),
			Err(ProgramError::InvalidAccountData)
		);
		assert_eq!(
			Journal::projected_bytes(0, Journal::MARKERS_CAPACITY + 1),
			Err(ProgramError::InvalidAccountData)
		);
	}

	#[test]
	fn generated_size_validation_rejects_every_invalid_shape() {
		assert!(Journal::validate_size(Journal::HEADER_SIZE).is_ok());
		assert!(Journal::validate_size(Journal::MAX_SIZE).is_ok());
		for invalid in [Journal::HEADER_SIZE - 1, Journal::MAX_SIZE + 1] {
			assert_eq!(
				Journal::validate_size(invalid),
				Err(ProgramError::InvalidAccountData)
			);
		}
	}

	#[test]
	fn compact_codec_roundtrips_header_and_active_entries() {
		let mut data = [0u8; Journal::HEADER_SIZE + 27];
		let entries = initialized_entries(3);
		let markers = initialized_markers(3);
		let encoded_size = {
			let mut journal = Journal::initialize(&mut data)
				.unwrap_or_else(|error| panic!("initialize journal: {error:?}"));
			journal.bump = 4;
			journal.authority = Address::new_from_array([7; 32]);
			journal.revision.set(2);
			journal
				.set_entries(&entries[..3])
				.unwrap_or_else(|error| panic!("set entries: {error:?}"));
			journal
				.set_markers(&markers[..3])
				.unwrap_or_else(|error| panic!("set markers: {error:?}"));
			assert_eq!(journal.encoded_size(), Journal::MIN_SIZE);
			assert_eq!(
				journal.projected_size(),
				Journal::projected_bytes(3, 3)
					.unwrap_or_else(|error| panic!("project size: {error:?}"))
			);
			journal
				.commit()
				.unwrap_or_else(|error| panic!("commit journal: {error:?}"))
		};

		assert_eq!(encoded_size, data.len());
		let journal = Journal::try_from_bytes(&data)
			.unwrap_or_else(|error| panic!("decode journal: {error:?}"));
		assert_eq!(journal.bump, 4);
		assert_eq!(journal.authority, Address::new_from_array([7; 32]));
		assert_eq!(journal.revision.get(), 2);
		assert_eq!(journal.entries(), &entries[..3]);
		assert_eq!(journal.markers(), &markers[..3]);
		assert_eq!(journal.encoded_size(), encoded_size);
	}

	#[test]
	fn compact_codec_handles_empty_and_full_tails() {
		for count in [0, MAX_ENTRIES] {
			let mut data = [0u8; Journal::MAX_SIZE];
			let size = Journal::projected_bytes(count, count)
				.unwrap_or_else(|error| panic!("size: {error:?}"));
			let entries = initialized_entries(count);
			let markers = initialized_markers(count);
			let encoded = {
				let mut journal = Journal::initialize(&mut data[..size])
					.unwrap_or_else(|error| panic!("initialize: {error:?}"));
				journal
					.set_entries(&entries[..count])
					.unwrap_or_else(|error| panic!("set entries: {error:?}"));
				journal
					.set_markers(&markers[..count])
					.unwrap_or_else(|error| panic!("set markers: {error:?}"));
				journal
					.commit()
					.unwrap_or_else(|error| panic!("commit: {error:?}"))
			};
			assert_eq!(encoded, size);
			assert_eq!(
				Journal::try_from_bytes(&data[..size])
					.unwrap_or_else(|error| panic!("decode: {error:?}"))
					.entries(),
				&entries[..count]
			);
			assert_eq!(
				Journal::try_from_bytes(&data[..size])
					.unwrap_or_else(|error| panic!("decode: {error:?}"))
					.markers(),
				&markers[..count]
			);
		}
	}

	#[test]
	fn compact_codec_rejects_corrupt_discriminators_prefixes_and_capacity() {
		let mut data = [0u8; Journal::HEADER_SIZE];
		Journal::initialize(&mut data)
			.unwrap_or_else(|error| panic!("initialize journal: {error:?}"));
		data[0] = 99;
		assert!(Journal::try_from_bytes(&data).is_err());

		data[0] = CompactAccountType::Journal as u8;
		data[38..40].copy_from_slice(&1u16.to_le_bytes());
		assert!(Journal::try_from_bytes(&data).is_err());

		let mut full_data = [0u8; Journal::MAX_SIZE];
		let mut journal = Journal::initialize(&mut full_data)
			.unwrap_or_else(|error| panic!("initialize full journal: {error:?}"));
		let too_many = [PodU64::ZERO; MAX_ENTRIES + 1];
		assert!(journal.set_entries(&too_many).is_err());
	}

	#[test]
	fn instruction_codecs_roundtrip_all_size_arguments() {
		let mut initialize_bytes = [0u8; InitializeIx::SIZE];
		let initialize = InitializeIx::initialize(&mut initialize_bytes)
			.unwrap_or_else(|error| panic!("initialize ix: {error:?}"));
		initialize.bump = 9;
		initialize.entry_count = 3;
		let decoded = InitializeIx::try_from_bytes(&initialize_bytes)
			.unwrap_or_else(|error| panic!("decode initialize ix: {error:?}"));
		assert_eq!((decoded.bump, decoded.entry_count), (9, 3));

		let mut resize_bytes = [0u8; ResizeIx::SIZE];
		ResizeIx::initialize(&mut resize_bytes)
			.unwrap_or_else(|error| panic!("resize ix: {error:?}"))
			.entry_count = MAX_ENTRIES as u8;
		assert_eq!(
			ResizeIx::try_from_bytes(&resize_bytes)
				.unwrap_or_else(|error| panic!("decode resize ix: {error:?}"))
				.entry_count,
			MAX_ENTRIES as u8
		);
	}

	#[test]
	fn write_instruction_roundtrips_native_u64() {
		let mut bytes = [0u8; WriteIx::SIZE];
		let write =
			WriteIx::initialize(&mut bytes).unwrap_or_else(|error| panic!("write ix: {error:?}"));
		write.index = 2;
		write.value.set(55);
		let decoded = WriteIx::try_from_bytes(&bytes)
			.unwrap_or_else(|error| panic!("decode write ix: {error:?}"));
		assert_eq!(decoded.index, 2);
		assert_eq!(decoded.value.get(), 55);
	}

	#[test]
	fn revisions_increment_and_detect_overflow() {
		assert_eq!(next_revision(0), Ok(1));
		assert_eq!(
			next_revision(u32::MAX),
			Err(ProgramError::ArithmeticOverflow)
		);
	}

	#[test]
	fn initialized_entries_fill_only_the_active_prefix() {
		let entries = initialized_entries(3);
		assert_eq!(entries[0].get(), 0);
		assert_eq!(entries[1].get(), 1);
		assert_eq!(entries[2].get(), 2);
		assert!(entries[3..].iter().all(|entry| entry.get() == 0));
	}

	#[test]
	fn initialized_markers_fill_only_the_active_prefix() {
		let markers = initialized_markers(3);
		assert_eq!(&markers[..3], &[0, 1, 2]);
		assert!(markers[3..].iter().all(|marker| *marker == 0));
	}

	#[test]
	fn journal_pdas_are_authority_bound() {
		let first: Address = [1; 32].into();
		let second: Address = [2; 32].into();
		assert_ne!(
			Journal::find_pda(&first, &ID).0,
			Journal::find_pda(&second, &ID).0
		);
	}
}
