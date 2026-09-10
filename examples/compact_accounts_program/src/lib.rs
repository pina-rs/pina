//! A focused compact-account lifecycle example.
//!
//! [`Journal`] stores a fixed header and only the active bytes of its title,
//! entries, markers, and optional note. Mutations use one atomic patch that
//! plans rent adjustment, reallocation, and validated encoding together.

#![allow(clippy::inline_always)]
#![allow(unused_qualifications)]
#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use core::mem::size_of;

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

/// A compact account with four independently encoded dynamic fields.
///
/// The fixed header includes a semantic `Option<u64>` encoded as
/// `PodOption<PodU64>`. The title and optional note use compact strings, while
/// entries and markers use vectors; only their active bytes are allocated.
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
	pub title: String<24>,
	/// Active entries. Unused capacity consumes no account bytes.
	pub entries: Vec<u64, 8>,
	/// Independently sized markers stored as a second compact tail.
	pub markers: PodVec<u8, 8, 8>,
	/// Optional human-readable status attached to the latest resize.
	#[allow(unused_qualifications)]
	pub note: Option<pina::String<64>>,
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
	/// Funds growth if a future write patch changes the encoded length.
	pub authority: &'a mut AccountView,
	pub journal: &'a mut AccountView,
}

#[derive(Accounts, Debug)]
pub struct RenameAccounts<'a> {
	/// Funds title growth and receives rent refunded by title shrinkage.
	pub authority: &'a mut AccountView,
	pub journal: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

fn assert_journal_authority(stored_authority: Address, authority: &Address) -> ProgramResult {
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

fn journal_size(
	title_len: usize,
	entry_count: usize,
	marker_count: usize,
	note_len: Option<usize>,
) -> Result<usize, ProgramError> {
	if title_len > Journal::TITLE_CAPACITY
		|| entry_count > Journal::ENTRIES_CAPACITY
		|| marker_count > Journal::MARKERS_CAPACITY
		|| note_len.is_some_and(|len| len > Journal::NOTE_CAPACITY)
	{
		return Err(CompactAccountError::CapacityExceeded.into());
	}

	let entry_bytes = entry_count
		.checked_mul(size_of::<PodU64>())
		.ok_or(CompactAccountError::CapacityExceeded)?;
	let note_bytes = match note_len {
		Some(len) => {
			len.checked_add(size_of::<u8>())
				.ok_or(CompactAccountError::CapacityExceeded)?
		}
		None => 0,
	};

	Journal::HEADER_SIZE
		.checked_add(title_len)
		.and_then(|size| size.checked_add(entry_bytes))
		.and_then(|size| size.checked_add(marker_count))
		.and_then(|size| size.checked_add(note_bytes))
		.ok_or_else(|| CompactAccountError::CapacityExceeded.into())
}

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = InitializeIx::try_from_bytes(data)?;
		let entry_count = usize::from(args.entry_count);
		let marker_count = usize::from(args.marker_count);
		let space = journal_size(DEFAULT_TITLE.len(), entry_count, marker_count, None)?;
		let authority_key = *self.authority.address();
		let entries = initialized_entries(entry_count);
		let markers = initialized_markers(marker_count);
		let seeds = Journal::seeds(&authority_key);

		self.authority.assert_signer()?.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;

		CreateCompactProgramAccountWithBump {
			account: self.journal,
			payer: self.authority,
			owner: &ID,
			seeds: &seeds.as_slices(),
			bump: args.bump,
			patch: JournalPatch::new()
				.bump(args.bump)
				.authority(authority_key)
				.revision(0)
				.title(DEFAULT_TITLE)
				.replace_entries(&entries[..entry_count])
				.replace_markers(&markers[..marker_count]),
			space,
		}
		.invoke::<Journal>()?;
		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for ResizeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = ResizeIx::try_from_bytes(data)?;
		let target_entry_count = usize::from(args.entry_count);
		let target_marker_count = usize::from(args.marker_count);
		let authority_key = *self.authority.address();
		journal_size(0, target_entry_count, target_marker_count, None)?;

		self.authority.assert_signer()?.assert_writable()?;
		self.system_program.assert_address(&system::ID)?;
		self.journal.assert_writable()?;

		let (mut entries, current_count, revision) =
			Journal::with_pda(self.journal, &authority_key, &ID, |journal| {
				assert_journal_authority(journal.authority, &authority_key)?;
				let current = journal.entries();
				let mut entries = [PodU64::ZERO; Journal::ENTRIES_CAPACITY];
				entries[..current.len()].copy_from_slice(current);
				Ok((entries, current.len(), journal.revision.get()))
			})?;

		for (index, entry) in entries
			.iter_mut()
			.enumerate()
			.take(target_entry_count)
			.skip(current_count)
		{
			entry.set(index as u64);
		}
		let markers = initialized_markers(target_marker_count);

		UpdateResizableAccount {
			account: self.journal,
			rent_account: self.authority,
			program_id: &ID,
			patch: JournalPatch::new()
				.revision(next_revision(revision)?)
				.replace_entries(&entries[..target_entry_count])
				.replace_markers(&markers[..target_marker_count])
				.note(Some("Updated")),
		}
		.invoke::<Journal>()?;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for WriteAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = WriteIx::try_from_bytes(data)?;
		let index = usize::from(args.index);
		let authority_key = *self.authority.address();

		self.authority.assert_signer()?.assert_writable()?;
		self.journal.assert_writable()?;

		let (mut entries, entry_count, revision) =
			Journal::with_pda(self.journal, &authority_key, &ID, |journal| {
				assert_journal_authority(journal.authority, &authority_key)?;
				let current = journal.entries();
				if index >= current.len() {
					return Err(CompactAccountError::IndexOutOfBounds.into());
				}
				let mut entries = [PodU64::ZERO; Journal::ENTRIES_CAPACITY];
				entries[..current.len()].copy_from_slice(current);
				Ok((entries, current.len(), journal.revision.get()))
			})?;
		entries[index].set(args.value.get());

		UpdateResizableAccount {
			account: self.journal,
			rent_account: self.authority,
			program_id: &ID,
			patch: JournalPatch::new()
				.revision(next_revision(revision)?)
				.featured_entry(Some(args.value.get()))
				.replace_entries(&entries[..entry_count]),
		}
		.invoke::<Journal>()?;

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
		self.journal.assert_writable()?;
		let revision = Journal::with_pda(self.journal, &authority_key, &ID, |journal| {
			assert_journal_authority(journal.authority, &authority_key)?;

			Ok(journal.revision.get())
		})?;

		UpdateResizableAccount {
			account: self.journal,
			rent_account: self.authority,
			program_id: &ID,
			patch: JournalPatch::new()
				.revision(next_revision(revision)?)
				.title(title),
		}
		.invoke::<Journal>()?;

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
		assert_eq!(Journal::HEADER_SIZE, 59);
		assert_eq!(Journal::MIN_SIZE, Journal::HEADER_SIZE);
		assert_eq!(Journal::MAX_SIZE, 220);
		assert_eq!(Journal::TAIL_ALIGNMENT, 1);
		assert_eq!(Journal::TITLE_CAPACITY, 24);
		assert_eq!(Journal::ENTRIES_CAPACITY, 8);
		assert_eq!(Journal::MARKERS_CAPACITY, 8);
		assert_eq!(Journal::NOTE_CAPACITY, 64);
		assert_eq!(journal_size(0, 0, 0, None), Ok(Journal::MIN_SIZE));
		assert_eq!(journal_size(5, 3, 5, None), Ok(Journal::HEADER_SIZE + 34));
		assert_eq!(
			journal_size(
				Journal::TITLE_CAPACITY,
				Journal::ENTRIES_CAPACITY,
				Journal::MARKERS_CAPACITY,
				Some(Journal::NOTE_CAPACITY),
			),
			Ok(Journal::MAX_SIZE)
		);
		assert_eq!(journal_size(0, 0, 0, Some(0)), Ok(Journal::MIN_SIZE + 1));
	}

	#[test]
	fn size_formula_rejects_each_count_past_its_capacity() {
		for result in [
			journal_size(Journal::TITLE_CAPACITY + 1, 0, 0, None),
			journal_size(0, Journal::ENTRIES_CAPACITY + 1, 0, None),
			journal_size(0, 0, Journal::MARKERS_CAPACITY + 1, None),
			journal_size(0, 0, 0, Some(Journal::NOTE_CAPACITY + 1)),
		] {
			assert!(matches!(
				result,
				Err(ProgramError::Custom(code))
					if code == CompactAccountError::CapacityExceeded as u32
			));
		}
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
		let mut data = [0u8; Journal::MAX_SIZE];
		let entries = initialized_entries(3);
		let markers = initialized_markers(5);
		let encoded_size = Journal::initialize(
			&mut data,
			&JournalPatch::new()
				.bump(4)
				.authority(Address::new_from_array([7; 32]))
				.revision(2)
				.featured_entry(Some(13_u64))
				.title("piña")
				.replace_entries(&entries[..3])
				.replace_markers(&markers[..5])
				.note(Some("Updated")),
		)
		.unwrap_or_else(|error| panic!("initialize journal: {error:?}"));

		assert_eq!(encoded_size, Journal::HEADER_SIZE + 42);
		let journal = Journal::try_from_bytes(&data[..encoded_size])
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
		assert_eq!(journal.note(), Some("Updated"));
		assert_eq!(journal.encoded_len(), encoded_size);
	}

	#[test]
	fn compact_patch_adds_an_optional_note_atomically() {
		let mut data = [0u8; Journal::MAX_SIZE];
		Journal::initialize(&mut data, &JournalPatch::new())
			.unwrap_or_else(|error| panic!("initialize journal: {error:?}"));

		let encoded_size = Journal::update(
			&mut data,
			&JournalPatch::new().revision(1).note(Some("Updated")),
		)
		.unwrap_or_else(|error| panic!("update journal: {error:?}"));
		let journal = Journal::try_from_bytes(&data[..encoded_size])
			.unwrap_or_else(|error| panic!("decode journal: {error:?}"));

		assert_eq!(journal.revision.get(), 1);
		assert_eq!(journal.note(), Some("Updated"));
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
			let size = journal_size(title.len(), entry_count, marker_count, None)
				.unwrap_or_else(|error| panic!("size: {error:?}"));
			let entries = initialized_entries(entry_count);
			let markers = initialized_markers(marker_count);
			let encoded = Journal::initialize(
				&mut data[..size],
				&JournalPatch::new()
					.title(title)
					.replace_entries(&entries[..entry_count])
					.replace_markers(&markers[..marker_count]),
			)
			.unwrap_or_else(|error| panic!("initialize: {error:?}"));
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
		Journal::initialize(&mut data, &JournalPatch::new())
			.unwrap_or_else(|error| panic!("initialize journal: {error:?}"));
		data[0] = 99;
		assert!(Journal::try_from_bytes(&data).is_err());

		data[0] = CompactAccountType::Journal as u8;
		data[48..50].copy_from_slice(&1u16.to_le_bytes());
		assert!(Journal::try_from_bytes(&data).is_err());

		let mut full_data = [0u8; Journal::MAX_SIZE];
		let too_many = [PodU64::ZERO; Journal::ENTRIES_CAPACITY + 1];
		assert!(
			Journal::initialize(
				&mut full_data,
				&JournalPatch::new().replace_entries(&too_many),
			)
			.is_err()
		);
		assert!(
			Journal::initialize(
				&mut full_data,
				&JournalPatch::new().title("this title exceeds twenty-four bytes"),
			)
			.is_err()
		);
		assert!(
			Journal::initialize(
				&mut full_data,
				&JournalPatch::new().note(Some(
					"this optional note contains more than sixty-four bytes of active UTF-8 data",
				)),
			)
			.is_err()
		);

		let mut invalid_utf8 = [0u8; Journal::MAX_SIZE];
		let encoded_size = Journal::initialize(&mut invalid_utf8, &JournalPatch::new().title("x"))
			.unwrap_or_else(|error| panic!("initialize title: {error:?}"));
		invalid_utf8[Journal::HEADER_SIZE] = 0xff;
		assert!(Journal::try_from_bytes(&invalid_utf8[..encoded_size]).is_err());
	}

	#[test]
	fn instruction_codecs_roundtrip_all_size_arguments() {
		let mut initialize_bytes = [0u8; InitializeIx::SIZE];
		InitializeIx::initialize(&mut initialize_bytes, |initialize| {
			initialize.bump = 9;
			initialize.entry_count = 3;
			initialize.marker_count = 5;
			Ok(())
		})
		.unwrap_or_else(|error| panic!("initialize ix: {error:?}"));
		let decoded = InitializeIx::try_from_bytes(&initialize_bytes)
			.unwrap_or_else(|error| panic!("decode initialize ix: {error:?}"));
		assert_eq!(
			(decoded.bump, decoded.entry_count, decoded.marker_count),
			(9, 3, 5)
		);

		let mut resize_bytes = [0u8; ResizeIx::SIZE];
		ResizeIx::initialize(&mut resize_bytes, |resize| {
			resize.entry_count = Journal::ENTRIES_CAPACITY as u8;
			resize.marker_count = 2;
			Ok(())
		})
		.unwrap_or_else(|error| panic!("resize ix: {error:?}"));
		let decoded = ResizeIx::try_from_bytes(&resize_bytes)
			.unwrap_or_else(|error| panic!("decode resize ix: {error:?}"));
		assert_eq!(
			(decoded.entry_count, decoded.marker_count),
			(Journal::ENTRIES_CAPACITY as u8, 2)
		);

		let mut rename_bytes = [0u8; RenameIx::SIZE];
		RenameIx::initialize(&mut rename_bytes, |rename| {
			rename.title_len = 5;
			rename.title[..5].copy_from_slice("piña".as_bytes());
			Ok(())
		})
		.unwrap_or_else(|error| panic!("initialize rename ix: {error:?}"));
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
		WriteIx::initialize(&mut bytes, |write| {
			write.index = 2;
			write.value.set(55);
			Ok(())
		})
		.unwrap_or_else(|error| panic!("write ix: {error:?}"));
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
