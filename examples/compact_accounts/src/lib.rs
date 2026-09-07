//! A focused compact-account lifecycle example.
//!
//! [`Journal`] stores a fixed header and only the active title, entry, and
//! marker bytes. [`ResizeAccounts`] uses [`ResizeCompactAccount`] to allocate
//! before growth, commit every tail, and refund rent after shrinkage.

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
pub const DEFAULT_TITLE: &str = "journal";

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
	Rename = 3,
}

#[discriminator]
pub enum CompactAccountType {
	Journal = 1,
}

/// A compact account with three independently encoded dynamic fields.
///
/// The fixed header includes a semantic `Option<u64>` encoded as
/// `PodOption<PodU64>`. The title uses `PodString`, while entries and markers
/// use vectors; only their active bytes are allocated.
#[account(discriminator = CompactAccountType, compact)]
#[pda(seeds = [SEED_JOURNAL, authority: Address], bump = bump)]
pub struct Journal {
	/// Canonical PDA bump.
	pub bump: u8,
	/// Signer permitted to mutate and fund this journal.
	pub authority: Address,
	/// Number of successful resize or write operations.
	pub revision: u32,
	/// Most recently written entry value, stored as `PodOption<PodU64>`.
	pub featured_entry: Option<u64>,
	/// Human-readable title stored as active UTF-8 bytes.
	pub title: PodString<24>,
	/// Active entries. Unused capacity consumes no account bytes.
	pub entries: Vec<u64, 8>,
	/// Independently sized markers stored as a second compact tail.
	pub markers: PodVec<u8, 8, 8>,
}

#[instruction(discriminator = CompactInstruction::Initialize)]
pub struct InitializeIx {
	pub bump: u8,
	/// Initial number of active entries. They are filled with `0..entry_count`.
	pub entry_count: u8,
	/// Initial number of active markers. They are filled with `0..marker_count`.
	pub marker_count: u8,
}

#[instruction(discriminator = CompactInstruction::Resize)]
pub struct ResizeIx {
	/// New logical length. Growth appends each new entry's index as its value.
	pub entry_count: u8,
	/// New marker length, independent of `entry_count`.
	pub marker_count: u8,
}

#[instruction(discriminator = CompactInstruction::Write)]
pub struct WriteIx {
	pub index: u8,
	pub value: u64,
}

#[instruction(discriminator = CompactInstruction::Rename)]
pub struct RenameIx {
	/// Active byte length within `title`.
	pub title_len: u8,
	/// UTF-8 title bytes. Bytes after `title_len` are ignored.
	pub title: [u8; 24],
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

#[derive(Accounts, Debug)]
pub struct RenameAccounts<'a> {
	/// Funds title growth and receives rent refunded by title shrinkage.
	pub authority: &'a mut AccountView,
	pub journal: &'a mut AccountView,
	pub system_program: &'a AccountView,
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

fn initialized_entries(entry_count: usize) -> [PodU64; Journal::ENTRIES_CAPACITY] {
	let mut entries = [PodU64::ZERO; Journal::ENTRIES_CAPACITY];
	for (index, entry) in entries.iter_mut().take(entry_count).enumerate() {
		entry.set(index as u64);
	}
	entries
}

fn initialized_markers(marker_count: usize) -> [u8; Journal::MARKERS_CAPACITY] {
	let mut markers = [0; Journal::MARKERS_CAPACITY];
	for (index, marker) in markers.iter_mut().take(marker_count).enumerate() {
		*marker = index as u8;
	}
	markers
}

fn title_from_bytes(bytes: &[u8; 24], title_len: usize) -> Result<&str, ProgramError> {
	let title = bytes
		.get(..title_len)
		.ok_or(ProgramError::InvalidInstructionData)?;
	core::str::from_utf8(title).map_err(|_| ProgramError::InvalidInstructionData)
}

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = InitializeIx::try_from_bytes(data)?;
		let entry_count = usize::from(args.entry_count);
		let marker_count = usize::from(args.marker_count);
		let space = Journal::projected_bytes(DEFAULT_TITLE.len(), entry_count, marker_count)
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
		let markers = initialized_markers(marker_count);
		let encoded_size = {
			let mut data = self.journal.try_borrow_mut()?;
			let mut journal = Journal::try_from_bytes_mut(&mut data)?;
			journal.bump = args.bump;
			journal.authority = authority_key;
			journal.revision.set(0);
			journal.featured_entry.set(None);
			journal
				.set_title(DEFAULT_TITLE)
				.map_err(|_| ProgramError::InvalidAccountData)?;
			journal
				.set_entries(&entries[..entry_count])
				.map_err(|_| ProgramError::InvalidAccountData)?;
			journal
				.set_markers(&markers[..marker_count])
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
		let target_entry_count = usize::from(args.entry_count);
		let target_marker_count = usize::from(args.marker_count);
		let authority_key = *self.authority.address();

		self.authority.assert_signer()?.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;
		validate_journal(*self.journal, &authority_key)?;

		let (title, title_len, mut entries, current_count, revision) = self
			.journal
			.with_compact_account::<Journal, _>(&ID, |journal| {
				let current_title = journal.title().as_bytes();
				let mut title = [0; Journal::TITLE_CAPACITY];
				title[..current_title.len()].copy_from_slice(current_title);
				let current = journal.entries();
				let mut entries = [PodU64::ZERO; Journal::ENTRIES_CAPACITY];
				entries[..current.len()].copy_from_slice(current);
				Ok((
					title,
					current_title.len(),
					entries,
					current.len(),
					journal.revision.get(),
				))
			})?;
		let title = core::str::from_utf8(&title[..title_len])
			.map_err(|_| ProgramError::InvalidAccountData)?;
		let target_size =
			Journal::projected_bytes(title_len, target_entry_count, target_marker_count)
				.map_err(|_| ProgramError::from(CompactAccountError::CapacityExceeded))?;
		for (index, entry) in entries
			.iter_mut()
			.enumerate()
			.take(target_entry_count)
			.skip(current_count)
		{
			entry.set(index as u64);
		}
		let markers = initialized_markers(target_marker_count);

		ResizeCompactAccount {
			account: self.journal,
			rent_account: self.authority,
			target_size,
			program_id: &ID,
		}
		.invoke::<Journal, _>(|data| {
			let mut journal = Journal::try_from_bytes_mut(data)?;
			journal.revision.set(next_revision(revision)?);
			journal
				.set_title(title)
				.map_err(|_| ProgramError::InvalidAccountData)?;
			journal
				.set_entries(&entries[..target_entry_count])
				.map_err(|_| ProgramError::InvalidAccountData)?;
			journal
				.set_markers(&markers[..target_marker_count])
				.map_err(|_| ProgramError::InvalidAccountData)?;
			journal
				.commit()
				.map_err(|_| ProgramError::InvalidAccountData)?;

			Ok(())
		})?;

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

		let (mut entries, title_len, entry_count, marker_count, revision) = self
			.journal
			.with_compact_account::<Journal, _>(&ID, |journal| {
				let current = journal.entries();
				if index >= current.len() {
					return Err(CompactAccountError::IndexOutOfBounds.into());
				}
				let mut entries = [PodU64::ZERO; Journal::ENTRIES_CAPACITY];
				entries[..current.len()].copy_from_slice(current);
				Ok((
					entries,
					journal.title().len(),
					current.len(),
					journal.markers().len(),
					journal.revision.get(),
				))
			})?;
		entries[index].set(args.value.get());

		let encoded_size = {
			let mut data = self.journal.try_borrow_mut()?;
			let mut journal = Journal::try_from_bytes_mut(&mut data)?;
			journal.revision.set(next_revision(revision)?);
			journal.featured_entry.set(Some(args.value));
			journal
				.set_entries(&entries[..entry_count])
				.map_err(|_| ProgramError::InvalidAccountData)?;
			journal
				.commit()
				.map_err(|_| ProgramError::InvalidAccountData)?
		};
		if encoded_size != Journal::projected_bytes(title_len, entry_count, marker_count)? {
			return Err(ProgramError::InvalidAccountData);
		}

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for RenameAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = RenameIx::try_from_bytes(data)?;
		let title_len = usize::from(args.title_len);
		let title = title_from_bytes(&args.title, title_len)?;
		let authority_key = *self.authority.address();

		self.authority.assert_signer()?.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;
		validate_journal(*self.journal, &authority_key)?;
		let (entry_count, marker_count, revision) = self
			.journal
			.with_compact_account::<Journal, _>(&ID, |journal| {
				Ok((
					journal.entries().len(),
					journal.markers().len(),
					journal.revision.get(),
				))
			})?;
		let target_size = Journal::projected_bytes(title_len, entry_count, marker_count)
			.map_err(|_| ProgramError::from(CompactAccountError::CapacityExceeded))?;

		ResizeCompactAccount {
			account: self.journal,
			rent_account: self.authority,
			target_size,
			program_id: &ID,
		}
		.invoke::<Journal, _>(|data| {
			let mut journal = Journal::try_from_bytes_mut(data)?;
			journal.revision.set(next_revision(revision)?);
			journal
				.set_title(title)
				.map_err(|_| ProgramError::InvalidAccountData)?;
			journal
				.commit()
				.map_err(|_| ProgramError::InvalidAccountData)?;

			Ok(())
		})
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
			CompactInstruction::Rename => {
				RenameAccounts::try_from((program_id, accounts))?.process(data)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn size_formula_covers_empty_partial_and_full_accounts() {
		assert_eq!(Journal::HEADER_SIZE, 58);
		assert_eq!(Journal::MIN_SIZE, Journal::HEADER_SIZE);
		assert_eq!(Journal::MAX_SIZE, 154);
		assert_eq!(Journal::TAIL_ALIGNMENT, 1);
		assert_eq!(Journal::TITLE_CAPACITY, 24);
		assert_eq!(Journal::ENTRIES_CAPACITY, 8);
		assert_eq!(Journal::MARKERS_CAPACITY, 8);
		assert_eq!(Journal::projected_bytes(0, 0, 0), Ok(Journal::MIN_SIZE));
		assert_eq!(
			Journal::projected_bytes(5, 3, 5),
			Ok(Journal::HEADER_SIZE + 34)
		);
		assert_eq!(
			Journal::projected_bytes(
				Journal::TITLE_CAPACITY,
				Journal::ENTRIES_CAPACITY,
				Journal::MARKERS_CAPACITY,
			),
			Ok(Journal::MAX_SIZE)
		);
	}

	#[test]
	fn size_formula_rejects_each_count_past_its_capacity() {
		assert_eq!(
			Journal::projected_bytes(Journal::TITLE_CAPACITY + 1, 0, 0),
			Err(ProgramError::InvalidAccountData)
		);
		assert_eq!(
			Journal::projected_bytes(0, Journal::ENTRIES_CAPACITY + 1, 0),
			Err(ProgramError::InvalidAccountData)
		);
		assert_eq!(
			Journal::projected_bytes(0, 0, Journal::MARKERS_CAPACITY + 1),
			Err(ProgramError::InvalidAccountData)
		);
	}

	#[test]
	fn generated_size_validation_rejects_every_invalid_shape() {
		assert!(Journal::validate_size(Journal::MIN_SIZE).is_ok());
		assert!(Journal::validate_size(Journal::MAX_SIZE).is_ok());
		for invalid in [Journal::MIN_SIZE - 1, Journal::MAX_SIZE + 1] {
			assert_eq!(
				Journal::validate_size(invalid),
				Err(ProgramError::InvalidAccountData)
			);
		}
	}

	#[test]
	fn compact_codec_roundtrips_header_and_independent_tails() {
		let target_size = Journal::projected_bytes(5, 3, 5)
			.unwrap_or_else(|error| panic!("project size: {error:?}"));
		let mut backing = [0u8; Journal::MAX_SIZE];
		let data = &mut backing[..target_size];
		let entries = initialized_entries(3);
		let markers = initialized_markers(5);
		let encoded_size = {
			let mut journal = Journal::initialize(&mut *data)
				.unwrap_or_else(|error| panic!("initialize journal: {error:?}"));
			journal.bump = 4;
			journal.authority = Address::new_from_array([7; 32]);
			journal.revision.set(2);
			journal.featured_entry.set(Some(PodU64::from(13)));
			journal
				.set_title("piña")
				.unwrap_or_else(|error| panic!("set title: {error:?}"));
			journal
				.set_entries(&entries[..3])
				.unwrap_or_else(|error| panic!("set entries: {error:?}"));
			journal
				.set_markers(&markers[..5])
				.unwrap_or_else(|error| panic!("set markers: {error:?}"));
			assert_eq!(journal.encoded_size(), Journal::MIN_SIZE);
			assert_eq!(
				journal.projected_size(),
				Journal::projected_bytes(5, 3, 5)
					.unwrap_or_else(|error| panic!("project size: {error:?}"))
			);
			journal
				.commit()
				.unwrap_or_else(|error| panic!("commit journal: {error:?}"))
		};

		assert_eq!(encoded_size, target_size);
		let journal = Journal::try_from_bytes(&*data)
			.unwrap_or_else(|error| panic!("decode journal: {error:?}"));
		assert_eq!(journal.bump, 4);
		assert_eq!(journal.authority, Address::new_from_array([7; 32]));
		assert_eq!(journal.revision.get(), 2);
		assert_eq!(
			journal.featured_entry.get().map(|value| value.get()),
			Some(13)
		);
		assert_eq!(journal.title(), "piña");
		assert_eq!(journal.entries(), &entries[..3]);
		assert_eq!(journal.markers(), &markers[..5]);
		assert_eq!(journal.encoded_size(), encoded_size);
	}

	#[test]
	fn compact_codec_handles_independent_empty_and_full_tails() {
		let full_title = "abcdefghijklmnopqrstuvwx";
		for (title, entry_count, marker_count) in [
			("", 0, 0),
			("", 0, Journal::MARKERS_CAPACITY),
			(full_title, Journal::ENTRIES_CAPACITY, 0),
			(
				full_title,
				Journal::ENTRIES_CAPACITY,
				Journal::MARKERS_CAPACITY,
			),
		] {
			let mut data = [0u8; Journal::MAX_SIZE];
			let size = Journal::projected_bytes(title.len(), entry_count, marker_count)
				.unwrap_or_else(|error| panic!("size: {error:?}"));
			let entries = initialized_entries(entry_count);
			let markers = initialized_markers(marker_count);
			let encoded = {
				let mut journal = Journal::initialize(&mut data[..size])
					.unwrap_or_else(|error| panic!("initialize: {error:?}"));
				journal
					.set_title(title)
					.unwrap_or_else(|error| panic!("set title: {error:?}"));
				journal
					.set_entries(&entries[..entry_count])
					.unwrap_or_else(|error| panic!("set entries: {error:?}"));
				journal
					.set_markers(&markers[..marker_count])
					.unwrap_or_else(|error| panic!("set markers: {error:?}"));
				journal
					.commit()
					.unwrap_or_else(|error| panic!("commit: {error:?}"))
			};
			assert_eq!(encoded, size);
			assert_eq!(
				Journal::try_from_bytes(&data[..size])
					.unwrap_or_else(|error| panic!("decode: {error:?}"))
					.title(),
				title,
			);
			assert_eq!(
				Journal::try_from_bytes(&data[..size])
					.unwrap_or_else(|error| panic!("decode: {error:?}"))
					.entries(),
				&entries[..entry_count]
			);
			assert_eq!(
				Journal::try_from_bytes(&data[..size])
					.unwrap_or_else(|error| panic!("decode: {error:?}"))
					.markers(),
				&markers[..marker_count]
			);
		}
	}

	#[test]
	fn compact_codec_rejects_corrupt_discriminators_prefixes_and_capacity() {
		let mut data = [0u8; Journal::MIN_SIZE];
		Journal::initialize(&mut data)
			.unwrap_or_else(|error| panic!("initialize journal: {error:?}"));
		data[0] = 99;
		assert!(Journal::try_from_bytes(&data).is_err());

		data[0] = CompactAccountType::Journal as u8;
		data[48..50].copy_from_slice(&1u16.to_le_bytes());
		assert!(Journal::try_from_bytes(&data).is_err());

		let mut full_data = [0u8; Journal::MAX_SIZE];
		let mut journal = Journal::initialize(&mut full_data)
			.unwrap_or_else(|error| panic!("initialize full journal: {error:?}"));
		assert!(
			journal
				.set_title("this title exceeds twenty-four bytes")
				.is_err()
		);
		let too_many = [PodU64::ZERO; Journal::ENTRIES_CAPACITY + 1];
		assert!(journal.set_entries(&too_many).is_err());

		let mut invalid_utf8 = [0u8; Journal::MIN_SIZE + 1];
		let mut journal = Journal::initialize(&mut invalid_utf8)
			.unwrap_or_else(|error| panic!("initialize title: {error:?}"));
		journal
			.set_title("x")
			.unwrap_or_else(|error| panic!("set title: {error:?}"));
		journal
			.commit()
			.unwrap_or_else(|error| panic!("commit title: {error:?}"));
		invalid_utf8[Journal::HEADER_SIZE] = 0xff;
		assert!(Journal::try_from_bytes(&invalid_utf8).is_err());
	}

	#[test]
	fn instruction_codecs_roundtrip_all_size_arguments() {
		let mut initialize_bytes = [0u8; InitializeIx::SIZE];
		let initialize = InitializeIx::initialize(&mut initialize_bytes)
			.unwrap_or_else(|error| panic!("initialize ix: {error:?}"));
		initialize.bump = 9;
		initialize.entry_count = 3;
		initialize.marker_count = 5;
		let decoded = InitializeIx::try_from_bytes(&initialize_bytes)
			.unwrap_or_else(|error| panic!("decode initialize ix: {error:?}"));
		assert_eq!(
			(decoded.bump, decoded.entry_count, decoded.marker_count),
			(9, 3, 5)
		);

		let mut resize_bytes = [0u8; ResizeIx::SIZE];
		let resize = ResizeIx::initialize(&mut resize_bytes)
			.unwrap_or_else(|error| panic!("resize ix: {error:?}"));
		resize.entry_count = Journal::ENTRIES_CAPACITY as u8;
		resize.marker_count = 2;
		let decoded = ResizeIx::try_from_bytes(&resize_bytes)
			.unwrap_or_else(|error| panic!("decode resize ix: {error:?}"));
		assert_eq!(
			(decoded.entry_count, decoded.marker_count),
			(Journal::ENTRIES_CAPACITY as u8, 2)
		);

		let mut rename_bytes = [0u8; RenameIx::SIZE];
		let rename = RenameIx::initialize(&mut rename_bytes)
			.unwrap_or_else(|error| panic!("initialize rename ix: {error:?}"));
		rename.title_len = 5;
		rename.title[..5].copy_from_slice("piña".as_bytes());
		let decoded = RenameIx::try_from_bytes(&rename_bytes)
			.unwrap_or_else(|error| panic!("decode rename ix: {error:?}"));
		assert_eq!(title_from_bytes(&decoded.title, 5), Ok("piña"));
	}

	#[test]
	fn title_decoder_rejects_out_of_range_and_invalid_utf8() {
		let mut title = [0; 24];
		assert_eq!(
			title_from_bytes(&title, Journal::TITLE_CAPACITY + 1),
			Err(ProgramError::InvalidInstructionData)
		);
		title[0] = 0xff;
		assert_eq!(
			title_from_bytes(&title, 1),
			Err(ProgramError::InvalidInstructionData)
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
