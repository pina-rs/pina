//! The runtime half of [`crate::nostd_entrypoint!`],
//! [`crate::nostd_entrypoint_alloc!`], and [`crate::dispatch_entrypoint!`].

use core::mem::MaybeUninit;

use crate::AccountView;
use crate::Address;
use crate::PinaProgramError;
use crate::ProgramError;
use crate::ProgramResult;
use crate::pinocchio::account::MAX_PERMITTED_DATA_INCREASE;
use crate::pinocchio::account::RuntimeAccount;
use crate::pinocchio::entrypoint::NON_DUP_MARKER;

/// Bytes a serialized account record reserves past its data: the record
/// header, then the region the account may grow into during the instruction.
const STATIC_ACCOUNT_DATA: usize = size_of::<RuntimeAccount>() + MAX_PERMITTED_DATA_INCREASE;

/// Alignment the loader pads every serialized account record to.
const RECORD_ALIGNMENT: usize = 8;

/// The loader's input buffer, before any of its accounts is parsed.
///
/// [`crate::dispatch_entrypoint!`]'s entrypoint creates the only one, and the
/// generated router consumes it to parse the routed instruction's accounts, so
/// generated code never handles the raw pointer. Parsing takes the input by
/// value because the walk reads each record's current data length, which a
/// handler can change by resizing the account: the input is only walked
/// before any handler runs.
#[doc(hidden)]
pub struct EntrypointInput {
	input: *mut u8,
}

impl EntrypointInput {
	/// Parses one route's accounts with the walk `MODE` selects:
	/// [`ROUTE_EXACT`], [`ROUTE_BOUNDED`], or [`ROUTE_UNBOUNDED`].
	///
	/// `dispatch_entrypoint!` picks `MODE` and `SLOTS` per route at compile
	/// time. Selecting the walk here, on a const generic, leaves each generated
	/// route with a single call to its accounts struct's parser, which LLVM
	/// inlines; branching in the generated route kept three.
	///
	/// `SHARED` runs the record walk through one out-of-line copy instead of
	/// inlining it; see [`walk`].
	///
	/// # Errors
	///
	/// Returns the chosen walk's errors.
	#[inline(always)]
	#[allow(unsafe_code)]
	pub fn parse_route<const SLOTS: usize, const MODE: u8, const SHARED: bool>(
		self,
		slots: &mut [MaybeUninit<AccountView>; SLOTS],
	) -> Result<&mut [AccountView], ProgramError> {
		// SAFETY: `self` holds the loader's input and has not been walked.
		unsafe {
			match MODE {
				ROUTE_EXACT => {
					parse_exact_accounts::<SLOTS, SHARED>(self.input, slots)
						.map(<[AccountView; SLOTS]>::as_mut_slice)
				}
				ROUTE_BOUNDED => parse_bounded_accounts::<SLOTS, SHARED>(self.input, slots),
				_ => parse_leading_accounts::<SLOTS, SHARED>(self.input, slots),
			}
		}
	}

	/// Parses the first `MAX_ACCOUNTS` accounts and ignores any past them, as
	/// `nostd_entrypoint!` does. The reserved `Migrate` route uses this, with
	/// the same `SHARED` choice as [`Self::parse_route`].
	///
	/// # Errors
	///
	/// Returns `InvalidAccountData` for a duplicate marker that names a later
	/// slot.
	#[inline(always)]
	#[allow(unsafe_code)]
	pub fn parse_leading<const MAX_ACCOUNTS: usize, const SHARED: bool>(
		self,
		slots: &mut [MaybeUninit<AccountView>; MAX_ACCOUNTS],
	) -> Result<&mut [AccountView], ProgramError> {
		// SAFETY: `self` holds the loader's input and has not been walked.
		unsafe { parse_leading_accounts::<MAX_ACCOUNTS, SHARED>(self.input, slots) }
	}
}

/// The dispatch-first entrypoint: reads the instruction data from the pointer
/// the loader passes in `r2`, then lets `dispatch` parse only the accounts the
/// routed instruction reads.
///
/// Since SIMD-0321 the loader passes a pointer to the instruction data as the
/// entrypoint's second argument. The data's length is the `u64` in the eight
/// bytes before it, and the program ID follows the data.
///
/// # Safety
///
/// `input` and `instruction_data` must be the two arguments the SVM loader
/// passes to the `entrypoint` symbol on a runtime with SIMD-0321 active, valid
/// for the whole instruction.
#[doc(hidden)]
#[inline(always)]
#[allow(unsafe_code)]
#[expect(
	clippy::cast_ptr_alignment,
	reason = "the loader 8-byte aligns its input region, including every u64 read here"
)]
pub unsafe fn __dispatch_entrypoint(
	input: *mut u8,
	instruction_data: *const u8,
	dispatch: fn(EntrypointInput, &Address, &[u8]) -> ProgramResult,
) -> u64 {
	// SAFETY: the loader writes the data length as an aligned `u64` directly
	// before the data, the data follows, and the 32-byte program ID follows the
	// data, all inside the input region that outlives the instruction.
	let (program_id, data) = unsafe {
		let len = *instruction_data.sub(size_of::<u64>()).cast::<u64>() as usize;
		let data = core::slice::from_raw_parts(instruction_data, len);
		let program_id = &*instruction_data.add(len).cast::<Address>();

		(program_id, data)
	};

	match dispatch(EntrypointInput { input }, program_id, data) {
		Ok(()) => pinocchio::SUCCESS,
		Err(error) => error.into(),
	}
}

/// [`EntrypointInput::parse_route`] mode for a route whose struct reads exactly
/// as many accounts as its array holds.
#[doc(hidden)]
pub const ROUTE_EXACT: u8 = 0;
/// [`EntrypointInput::parse_route`] mode for a route whose struct reads at most
/// as many accounts as its array holds.
#[doc(hidden)]
pub const ROUTE_BOUNDED: u8 = 1;
/// [`EntrypointInput::parse_route`] mode for a route whose parser has no
/// account limit: the route reads the first accounts that fit its array and
/// ignores the rest.
#[doc(hidden)]
pub const ROUTE_UNBOUNDED: u8 = 2;

/// Walks exactly `N` of the loader's serialized accounts into `slots` and
/// returns them as a fixed-length array.
///
/// `dispatch_entrypoint!` calls this for a route whose accounts struct reads
/// exactly `N` accounts (its `ACCOUNT_MINIMUM` equals its `ACCOUNT_LIMIT`).
/// The fixed length lets the struct's own length checks fold away. An input
/// with any other count fails here, before any of the struct's per-account
/// checks, so the count error wins over a field the struct's parser would have
/// rejected first.
///
/// # Errors
///
/// Returns `NotEnoughAccountKeys` when the input holds fewer than `N`
/// accounts, `TooManyAccountKeys` when it holds more, and `InvalidAccountData`
/// for a duplicate marker that names a later slot.
///
/// # Safety
///
/// `input` must point to the program input buffer the SVM loader passes to the
/// `entrypoint` symbol, valid for the whole instruction, and no handler may have
/// resized an account in it.
#[inline(always)]
#[allow(unsafe_code)]
#[expect(
	clippy::cast_ptr_alignment,
	reason = "the loader 8-byte aligns its input region, including every u64 read here"
)]
unsafe fn parse_exact_accounts<const N: usize, const SHARED: bool>(
	input: *mut u8,
	slots: &mut [MaybeUninit<AccountView>; N],
) -> Result<&mut [AccountView; N], ProgramError> {
	// SAFETY: the input begins with the account count as an aligned `u64`.
	let count = unsafe { *input.cast::<u64>() } as usize;
	if count < N {
		return Err(ProgramError::NotEnoughAccountKeys);
	}
	if count > N {
		return Err(PinaProgramError::TooManyAccountKeys.into());
	}

	// SAFETY: the input holds `N` records, and `slots` has `N` slots.
	unsafe { walk::<SHARED>(input, slots.as_mut_ptr().cast::<AccountView>(), N)? };

	// SAFETY: the walk initialized all `N` slots.
	Ok(unsafe { &mut *slots.as_mut_ptr().cast::<[AccountView; N]>() })
}

/// Walks the loader's serialized accounts into `slots` when the input holds at
/// most `N`, and returns them.
///
/// `dispatch_entrypoint!` calls this for a route whose accounts struct reads at
/// most `N` accounts but accepts fewer, because it ends with optional fields.
/// An instruction with more than `N` fails here with `TooManyAccountKeys`, as
/// the struct's own parser would, and the parser still reports any required
/// account that is missing.
///
/// # Errors
///
/// Returns `TooManyAccountKeys` when the input holds more than `N` accounts,
/// and `InvalidAccountData` for a duplicate marker that names a later slot.
///
/// # Safety
///
/// `input` must point to the program input buffer the SVM loader passes to the
/// `entrypoint` symbol, valid for the whole instruction, and no handler may have
/// resized an account in it.
#[inline(always)]
#[allow(unsafe_code)]
#[expect(
	clippy::cast_ptr_alignment,
	reason = "the loader 8-byte aligns its input region, including every u64 read here"
)]
unsafe fn parse_bounded_accounts<const N: usize, const SHARED: bool>(
	input: *mut u8,
	slots: &mut [MaybeUninit<AccountView>; N],
) -> Result<&mut [AccountView], ProgramError> {
	// SAFETY: the input begins with the account count as an aligned `u64`.
	let count = unsafe { *input.cast::<u64>() } as usize;
	if count > N {
		return Err(PinaProgramError::TooManyAccountKeys.into());
	}

	let views = slots.as_mut_ptr().cast::<AccountView>();
	// SAFETY: the input holds `count` records, and `count <= N` slots exist.
	unsafe { walk::<SHARED>(input, views, count)? };

	// SAFETY: the walk initialized the first `count` slots.
	Ok(unsafe { core::slice::from_raw_parts_mut(views, count) })
}

/// Runs [`walk_accounts`] inline, or through [`walk_accounts_shared`].
///
/// An inlined walk is the faster one: a route that reads a fixed number of
/// accounts unrolls it and folds the checks its positions make impossible. It
/// is also the smaller one while a program has few routes, but every route
/// carries its own copy, so `dispatch_entrypoint!` routes a program with more
/// walks through one shared copy instead.
///
/// # Safety
///
/// The contract of [`walk_accounts`].
#[inline(always)]
#[allow(unsafe_code)]
unsafe fn walk<const SHARED: bool>(
	input: *mut u8,
	views: *mut AccountView,
	count: usize,
) -> Result<(), ProgramError> {
	if !SHARED {
		// SAFETY: the caller upholds `walk_accounts`'s contract.
		return unsafe { walk_accounts(input, views, count) };
	}

	// SAFETY: the caller upholds `walk_accounts`'s contract.
	if unsafe { walk_accounts_shared(input, views, count) } {
		Ok(())
	} else {
		Err(ProgramError::InvalidAccountData)
	}
}

/// [`walk_accounts`] as the one out-of-line copy every route of a program
/// calls. It returns whether the walk succeeded, so the result fits a register;
/// the only failure is `InvalidAccountData`.
///
/// # Safety
///
/// The contract of [`walk_accounts`].
#[inline(never)]
#[allow(unsafe_code)]
unsafe fn walk_accounts_shared(input: *mut u8, views: *mut AccountView, count: usize) -> bool {
	// SAFETY: the caller upholds `walk_accounts`'s contract.
	unsafe { walk_accounts(input, views, count) }.is_ok()
}

/// Writes a view of each of the first `count` serialized accounts to `views`.
///
/// A duplicate marker copies the earlier view, as
/// `pinocchio::entrypoint::deserialize` does. A marker that does not name an
/// earlier slot fails with `InvalidAccountData`: the runtime never writes one,
/// and the check costs nothing on the non-duplicate path.
///
/// # Safety
///
/// `input` must point to the loader's input buffer, which holds at least
/// `count` account records, and `views` must be valid for `count` writes.
#[inline(always)]
#[allow(unsafe_code)]
unsafe fn walk_accounts(
	input: *mut u8,
	views: *mut AccountView,
	count: usize,
) -> Result<(), ProgramError> {
	// SAFETY: the first record follows the eight-byte account count.
	let mut cursor = unsafe { input.add(size_of::<u64>()) };
	let mut index = 0;

	while index < count {
		let account = cursor.cast::<RuntimeAccount>();
		// SAFETY: `cursor` is at the start of record `index`, which begins with
		// its one-byte duplicate marker. A duplicate record is eight bytes and a
		// non-duplicate one is a `RuntimeAccount` header followed by its data,
		// the growth region, and padding to the record alignment.
		let marker = unsafe { (*account).borrow_state };

		if marker == NON_DUP_MARKER {
			// SAFETY: `account` is a complete record header and `index < count`,
			// so the view slot exists; the next record starts after the header,
			// the data, the growth region, and the eight-byte rent epoch, aligned.
			unsafe {
				#[cfg(feature = "account-resize")]
				{
					(*account).padding = u32::to_le_bytes((*account).data_len as u32);
				}
				views.add(index).write(AccountView::new_unchecked(account));
				cursor = cursor
					.add(size_of::<u64>() + STATIC_ACCOUNT_DATA + (*account).data_len as usize)
					.map_addr(|address| (address + RECORD_ALIGNMENT - 1) & !(RECORD_ALIGNMENT - 1));
			}
		} else {
			let original = usize::from(marker);
			if original >= index {
				return Err(ProgramError::InvalidAccountData);
			}
			// SAFETY: slot `original` is below `index`, so it was written above,
			// and a duplicate record is eight bytes long.
			unsafe {
				views.add(index).write(*views.add(original));
				cursor = cursor.add(size_of::<u64>());
			}
		}

		index += 1;
	}

	Ok(())
}

/// Walks the first `MAX_ACCOUNTS` of the loader's serialized accounts into
/// `slots`, ignoring any past them, and returns them.
///
/// `dispatch_entrypoint!` calls this for a route whose parser has no account
/// limit, such as one with a `#[pina(remaining)]` slice, and for the reserved
/// `Migrate` route. The route sees the same accounts `nostd_entrypoint!` hands
/// it with the same array, whose deserializer skips accounts past the array.
/// This walk stops there instead: the entrypoint already has the instruction
/// data from its own pointer, so nothing past the array needs reading.
///
/// # Errors
///
/// Returns `InvalidAccountData` for a duplicate marker that names a later
/// slot.
///
/// # Safety
///
/// `input` must point to the program input buffer the SVM loader passes to the
/// `entrypoint` symbol, valid for the whole instruction, and no handler may have
/// resized an account in it.
#[inline(always)]
#[allow(unsafe_code)]
#[expect(
	clippy::cast_ptr_alignment,
	reason = "the loader 8-byte aligns its input region, including every u64 read here"
)]
unsafe fn parse_leading_accounts<const MAX_ACCOUNTS: usize, const SHARED: bool>(
	input: *mut u8,
	slots: &mut [MaybeUninit<AccountView>; MAX_ACCOUNTS],
) -> Result<&mut [AccountView], ProgramError> {
	// SAFETY: the input begins with the account count as an aligned `u64`.
	let count = (unsafe { *input.cast::<u64>() } as usize).min(MAX_ACCOUNTS);

	let views = slots.as_mut_ptr().cast::<AccountView>();
	// SAFETY: the input holds at least `count` records, and `count <=
	// MAX_ACCOUNTS` slots exist.
	unsafe { walk::<SHARED>(input, views, count)? };

	// SAFETY: the walk initialized the first `count` slots.
	Ok(unsafe { core::slice::from_raw_parts_mut(views, count) })
}

/// Deserializes the loader's input, runs `process_instruction`, and returns
/// its result as the entrypoint's `u64` status.
///
/// This is `pinocchio::entrypoint::process_entrypoint` with one difference:
/// the `ProgramError` is converted to its status code inline. Pinocchio keeps
/// that conversion in an `#[inline(never)]` function, an 864-byte comparison
/// tree that every program carries even when each error it returns is a
/// constant. Inlined, an error returned by value folds to its code where it is
/// returned, and the tree is only emitted for errors the compiler cannot see
/// through, such as those from an outlined helper or a CPI.
///
/// # Safety
///
/// `input` must point to the program input buffer the SVM loader passes to
/// the `entrypoint` symbol, valid for the whole instruction. This is the
/// contract of `pinocchio::entrypoint::deserialize`.
#[doc(hidden)]
#[inline(always)]
#[allow(unsafe_code)]
pub unsafe fn __process_entrypoint<const MAX_ACCOUNTS: usize>(
	input: *mut u8,
	process_instruction: fn(&Address, &mut [AccountView], &[u8]) -> ProgramResult,
) -> u64 {
	let mut accounts = [const { MaybeUninit::<AccountView>::uninit() }; MAX_ACCOUNTS];
	// SAFETY: the caller upholds `deserialize`'s contract for `input`.
	let (program_id, count, instruction_data) =
		unsafe { pinocchio::entrypoint::deserialize::<MAX_ACCOUNTS>(input, &mut accounts) };
	// SAFETY: `deserialize` initialized the first `count` slots, and `count`
	// never exceeds `MAX_ACCOUNTS`.
	let accounts = unsafe {
		core::slice::from_raw_parts_mut(accounts.as_mut_ptr().cast::<AccountView>(), count)
	};

	match process_instruction(program_id, accounts, instruction_data) {
		Ok(()) => pinocchio::SUCCESS,
		Err(error) => error.into(),
	}
}

#[cfg(test)]
#[allow(unsafe_code)]
mod tests {
	extern crate std;

	use std::cell::Cell;
	use std::cell::RefCell;
	use std::vec::Vec;

	use super::*;

	const PROGRAM_ID: Address = Address::new_from_array([7; 32]);

	/// One account slot in a serialized loader input.
	#[derive(Clone, Copy, Debug)]
	enum Slot {
		/// A distinct account with this address and data length.
		Account { address: u8, data_len: u8 },
		/// A duplicate of the account at this index.
		Duplicate(u8),
	}

	/// Serializes the aligned input the SVM loader passes to `entrypoint`.
	///
	/// The buffer is `u64`-backed so every field the deserializer reads is
	/// eight-byte aligned, as it is in the loader's input region.
	fn serialize(slots: &[Slot], instruction_data: &[u8]) -> Vec<u64> {
		let mut bytes = Vec::new();
		bytes.extend_from_slice(&(slots.len() as u64).to_le_bytes());

		for slot in slots {
			match slot {
				Slot::Account { address, data_len } => {
					let mut header = [0_u8; size_of::<RuntimeAccount>()];
					header[0] = NON_DUP_MARKER;
					header[8..40].fill(*address);
					header[80..88].copy_from_slice(&u64::from(*data_len).to_le_bytes());
					bytes.extend_from_slice(&header);
					// Data, the growth region, and the trailing rent epoch,
					// padded so the next slot starts eight-byte aligned.
					let tail =
						(usize::from(*data_len) + MAX_PERMITTED_DATA_INCREASE + size_of::<u64>())
							.next_multiple_of(size_of::<u64>());
					bytes.resize(bytes.len() + tail, 0);
				}
				Slot::Duplicate(index) => {
					bytes.extend_from_slice(&[*index, 0, 0, 0, 0, 0, 0, 0]);
				}
			}
		}

		bytes.extend_from_slice(&(instruction_data.len() as u64).to_le_bytes());
		bytes.extend_from_slice(instruction_data);
		bytes.extend_from_slice(PROGRAM_ID.as_ref());
		bytes.resize(bytes.len().next_multiple_of(size_of::<u64>()), 0);

		bytes
			.as_chunks::<{ size_of::<u64>() }>()
			.0
			.iter()
			.map(|word| u64::from_ne_bytes(*word))
			.collect()
	}

	fn run<const MAX_ACCOUNTS: usize>(
		input: &mut [u64],
		process_instruction: fn(&Address, &mut [AccountView], &[u8]) -> ProgramResult,
	) -> u64 {
		// SAFETY: `input` is a complete, aligned loader input built by
		// `serialize` that outlives the call.
		unsafe {
			__process_entrypoint::<MAX_ACCOUNTS>(input.as_mut_ptr().cast(), process_instruction)
		}
	}

	std::thread_local! {
		static RESULT: RefCell<ProgramResult> = const { RefCell::new(Ok(())) };
		static SEEN_ACCOUNTS: Cell<usize> = const { Cell::new(0) };
	}

	/// Records what the program saw and returns the result the test chose.
	fn record(program_id: &Address, accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
		assert_eq!(program_id, &PROGRAM_ID);
		assert_eq!(data, [1, 2, 3]);
		SEEN_ACCOUNTS.set(accounts.len());

		RESULT.with_borrow(Clone::clone)
	}

	fn run_with_result(result: ProgramResult) -> u64 {
		RESULT.set(result);
		let mut input = serialize(
			&[Slot::Account {
				address: 1,
				data_len: 3,
			}],
			&[1, 2, 3],
		);

		run::<{ pinocchio::MAX_TX_ACCOUNTS }>(&mut input, record)
	}

	#[test]
	fn success_returns_the_success_status() {
		assert_eq!(run_with_result(Ok(())), pinocchio::SUCCESS);
		assert_eq!(SEEN_ACCOUNTS.get(), 1);
	}

	#[test]
	fn errors_return_the_same_status_as_the_program_error_conversion() {
		let errors = [
			ProgramError::Custom(0),
			ProgramError::Custom(1),
			ProgramError::Custom(u32::MAX),
			ProgramError::InvalidArgument,
			ProgramError::InvalidInstructionData,
			ProgramError::InvalidAccountData,
			ProgramError::AccountDataTooSmall,
			ProgramError::InsufficientFunds,
			ProgramError::IncorrectProgramId,
			ProgramError::MissingRequiredSignature,
			ProgramError::AccountAlreadyInitialized,
			ProgramError::UninitializedAccount,
			ProgramError::NotEnoughAccountKeys,
			ProgramError::AccountBorrowFailed,
			ProgramError::MaxSeedLengthExceeded,
			ProgramError::InvalidSeeds,
			ProgramError::BorshIoError,
			ProgramError::AccountNotRentExempt,
			ProgramError::UnsupportedSysvar,
			ProgramError::IllegalOwner,
			ProgramError::MaxAccountsDataAllocationsExceeded,
			ProgramError::InvalidRealloc,
			ProgramError::MaxInstructionTraceLengthExceeded,
			ProgramError::BuiltinProgramsMustConsumeComputeUnits,
			ProgramError::InvalidAccountOwner,
			ProgramError::ArithmeticOverflow,
			ProgramError::Immutable,
			ProgramError::IncorrectAuthority,
		];

		for error in errors {
			assert_eq!(
				run_with_result(Err(error.clone())),
				u64::from(error.clone()),
				"{error:?}"
			);
		}
	}

	/// With an account array smaller than the transaction, the program sees
	/// only the array's accounts, and the instruction data and program ID are
	/// still found past the skipped ones — including a skipped duplicate.
	#[test]
	fn accounts_past_the_array_are_skipped_before_the_instruction_data() {
		RESULT.set(Ok(()));
		let mut input = serialize(
			&[
				Slot::Account {
					address: 1,
					data_len: 0,
				},
				Slot::Duplicate(0),
				Slot::Account {
					address: 2,
					data_len: 5,
				},
				Slot::Duplicate(2),
			],
			&[1, 2, 3],
		);

		assert_eq!(run::<2>(&mut input, record), pinocchio::SUCCESS);
		assert_eq!(SEEN_ACCOUNTS.get(), 2);
		assert_eq!(run::<4>(&mut input, record), pinocchio::SUCCESS);
		assert_eq!(SEEN_ACCOUNTS.get(), 4);
	}

	/// A duplicate slot resolves to the same account view as the slot it names.
	#[test]
	fn duplicate_slots_alias_their_original_account() {
		#[allow(
			clippy::unnecessary_wraps,
			reason = "the entrypoint calls it through the `process_instruction` signature"
		)]
		fn check_aliases(
			_program_id: &Address,
			accounts: &mut [AccountView],
			_data: &[u8],
		) -> ProgramResult {
			assert_eq!(accounts.len(), 3);
			assert_eq!(accounts[0], accounts[2]);
			assert_ne!(accounts[0], accounts[1]);
			assert_eq!(accounts[1].address(), &Address::new_from_array([2; 32]));

			Ok(())
		}

		let mut input = serialize(
			&[
				Slot::Account {
					address: 1,
					data_len: 1,
				},
				Slot::Account {
					address: 2,
					data_len: 9,
				},
				Slot::Duplicate(0),
			],
			&[],
		);

		assert_eq!(run::<3>(&mut input, check_aliases), pinocchio::SUCCESS);
	}

	/// Byte offset of the instruction data in a `serialize`d input: past the
	/// account count, every record, and the data length.
	fn instruction_data_offset(slots: &[Slot]) -> usize {
		let records: usize = slots
			.iter()
			.map(|slot| {
				match slot {
					Slot::Account { data_len, .. } => {
						size_of::<RuntimeAccount>()
							+ (usize::from(*data_len)
								+ MAX_PERMITTED_DATA_INCREASE
								+ size_of::<u64>())
							.next_multiple_of(size_of::<u64>())
					}
					Slot::Duplicate(_) => size_of::<u64>(),
				}
			})
			.sum();

		size_of::<u64>() + records + size_of::<u64>()
	}

	fn slots<const N: usize>() -> [MaybeUninit<AccountView>; N] {
		[const { MaybeUninit::uninit() }; N]
	}

	fn addresses(accounts: &[AccountView]) -> Vec<u8> {
		accounts
			.iter()
			.map(|account| account.address().as_ref()[0])
			.collect()
	}

	/// Accounts of varied data lengths, so every record boundary is aligned
	/// differently.
	fn varied_accounts() -> [Slot; 3] {
		[
			Slot::Account {
				address: 1,
				data_len: 3,
			},
			Slot::Account {
				address: 2,
				data_len: 0,
			},
			Slot::Account {
				address: 3,
				data_len: 9,
			},
		]
	}

	#[test]
	fn exact_parse_walks_every_record_and_rejects_other_counts() {
		let mut input = serialize(&varied_accounts(), &[]);
		let input = input.as_mut_ptr().cast::<u8>();

		// SAFETY: `input` is a complete loader input built by `serialize`.
		unsafe {
			let mut exact = slots::<3>();
			let accounts = parse_exact_accounts::<3, false>(input, &mut exact);
			assert_eq!(
				accounts.map(|accounts| addresses(accounts)),
				Ok(std::vec![1, 2, 3])
			);

			let mut wider = slots::<4>();
			assert_eq!(
				parse_exact_accounts::<4, false>(input, &mut wider).map(|_| ()),
				Err(ProgramError::NotEnoughAccountKeys)
			);

			let mut narrower = slots::<2>();
			assert_eq!(
				parse_exact_accounts::<2, false>(input, &mut narrower).map(|_| ()),
				Err(PinaProgramError::TooManyAccountKeys.into())
			);
		}
	}

	#[test]
	fn bounded_parse_accepts_fewer_accounts_and_rejects_more() {
		let mut input = serialize(&varied_accounts(), &[]);
		let input = input.as_mut_ptr().cast::<u8>();

		// SAFETY: `input` is a complete loader input built by `serialize`.
		unsafe {
			let mut wider = slots::<5>();
			let accounts = parse_bounded_accounts::<5, false>(input, &mut wider);
			assert_eq!(
				accounts.map(|accounts| addresses(accounts)),
				Ok(std::vec![1, 2, 3])
			);

			let mut narrower = slots::<2>();
			assert_eq!(
				parse_bounded_accounts::<2, false>(input, &mut narrower).map(|_| ()),
				Err(PinaProgramError::TooManyAccountKeys.into())
			);
		}

		let mut empty = serialize(&[], &[]);
		let mut none = slots::<2>();
		// SAFETY: `empty` is a complete loader input built by `serialize`.
		let accounts =
			unsafe { parse_bounded_accounts::<2, false>(empty.as_mut_ptr().cast(), &mut none) };
		assert_eq!(accounts.map(|accounts| accounts.len()), Ok(0));
	}

	/// A duplicate marker resolves to the view of the slot it names, in both
	/// walks, and a marker naming the current or a later slot is rejected.
	#[test]
	fn walks_resolve_duplicates_and_reject_forward_markers() {
		let mut input = serialize(
			&[
				Slot::Account {
					address: 1,
					data_len: 2,
				},
				Slot::Duplicate(0),
				Slot::Account {
					address: 2,
					data_len: 0,
				},
			],
			&[],
		);
		let input = input.as_mut_ptr().cast::<u8>();

		// SAFETY: `input` is a complete loader input built by `serialize`.
		unsafe {
			let mut exact = slots::<3>();
			let aliases = parse_exact_accounts::<3, false>(input, &mut exact)
				.map(|accounts| (accounts[0] == accounts[1], accounts[0] == accounts[2]));
			assert_eq!(aliases, Ok((true, false)));

			let mut bounded = slots::<4>();
			let accounts = parse_bounded_accounts::<4, false>(input, &mut bounded)
				.map(|accounts| addresses(accounts));
			assert_eq!(accounts, Ok(std::vec![1, 1, 2]));
		}

		for forward in [1, 2] {
			let mut input = serialize(
				&[
					Slot::Account {
						address: 1,
						data_len: 0,
					},
					Slot::Duplicate(forward),
					Slot::Account {
						address: 2,
						data_len: 0,
					},
				],
				&[],
			);
			let input = input.as_mut_ptr().cast::<u8>();
			let mut exact = slots::<3>();
			// SAFETY: `input` is a complete loader input built by `serialize`.
			let rejected = unsafe { parse_exact_accounts::<3, false>(input, &mut exact) };
			assert_eq!(
				rejected.map(|_| ()),
				Err(ProgramError::InvalidAccountData),
				"marker {forward}"
			);

			let mut leading = slots::<3>();
			let rejected = EntrypointInput { input }.parse_leading::<3, false>(&mut leading);
			assert_eq!(
				rejected.map(|_| ()),
				Err(ProgramError::InvalidAccountData),
				"marker {forward}"
			);

			// The shared walk reports the same error through its flag.
			let mut shared = slots::<3>();
			let rejected =
				EntrypointInput { input }.parse_route::<3, ROUTE_EXACT, true>(&mut shared);
			assert_eq!(
				rejected.map(|_| ()),
				Err(ProgramError::InvalidAccountData),
				"marker {forward}"
			);
		}
	}

	/// The input token runs the walk each mode names.
	#[test]
	fn route_modes_select_their_walk() {
		let mut input = serialize(&varied_accounts(), &[]);
		let input = input.as_mut_ptr().cast::<u8>();
		// Each walk gets a fresh token over the same unresized input, as each
		// instruction's entrypoint does.
		let token = || EntrypointInput { input };

		let mut exact = slots::<3>();
		let accounts = token().parse_route::<3, ROUTE_EXACT, false>(&mut exact);
		assert_eq!(accounts.map(|accounts| accounts.len()), Ok(3));

		let mut exact_mismatch = slots::<4>();
		assert_eq!(
			token()
				.parse_route::<4, ROUTE_EXACT, false>(&mut exact_mismatch)
				.map(|_| ()),
			Err(ProgramError::NotEnoughAccountKeys)
		);

		let mut bounded = slots::<4>();
		let accounts = token().parse_route::<4, ROUTE_BOUNDED, false>(&mut bounded);
		assert_eq!(accounts.map(|accounts| accounts.len()), Ok(3));

		// The unbounded walks skip accounts past their array instead of failing.
		let mut unbounded = slots::<2>();
		let accounts = token().parse_route::<2, ROUTE_UNBOUNDED, false>(&mut unbounded);
		assert_eq!(
			accounts.map(|accounts| addresses(accounts)),
			Ok(std::vec![1, 2])
		);

		let mut leading = slots::<2>();
		let accounts = token().parse_leading::<2, false>(&mut leading);
		assert_eq!(
			accounts.map(|accounts| addresses(accounts)),
			Ok(std::vec![1, 2])
		);

		// Fewer accounts than the array are all read.
		let mut wider = slots::<5>();
		let accounts = token().parse_leading::<5, false>(&mut wider);
		assert_eq!(
			accounts.map(|accounts| addresses(accounts)),
			Ok(std::vec![1, 2, 3])
		);

		// The shared walk reads the same accounts in every mode.
		let mut exact = slots::<3>();
		let accounts = token().parse_route::<3, ROUTE_EXACT, true>(&mut exact);
		assert_eq!(
			accounts.map(|accounts| addresses(accounts)),
			Ok(std::vec![1, 2, 3])
		);
		let mut bounded = slots::<4>();
		let accounts = token().parse_route::<4, ROUTE_BOUNDED, true>(&mut bounded);
		assert_eq!(
			accounts.map(|accounts| addresses(accounts)),
			Ok(std::vec![1, 2, 3])
		);
		let mut leading = slots::<2>();
		let accounts = token().parse_leading::<2, true>(&mut leading);
		assert_eq!(
			accounts.map(|accounts| addresses(accounts)),
			Ok(std::vec![1, 2])
		);
	}

	std::thread_local! {
		static DISPATCHED: RefCell<Option<(Address, Vec<u8>, usize)>> = const { RefCell::new(None) };
	}

	/// Records what the dispatch-first entrypoint handed the router.
	fn record_dispatch(input: EntrypointInput, program_id: &Address, data: &[u8]) -> ProgramResult {
		let mut all = slots::<4>();
		let accounts = input.parse_leading::<4, false>(&mut all)?;
		DISPATCHED.set(Some((*program_id, data.to_vec(), accounts.len())));

		RESULT.with_borrow(Clone::clone)
	}

	/// The entrypoint reads the data and program ID through the instruction
	/// data pointer, and converts the router's result to its status.
	#[test]
	fn dispatch_entrypoint_reads_the_instruction_data_pointer() {
		let [first, second, third] = varied_accounts();
		let accounts = [first, second, third, Slot::Duplicate(0)];
		let data = [4, 5, 6, 7, 8];
		let mut input = serialize(&accounts, &data);
		let offset = instruction_data_offset(&accounts);
		let input = input.as_mut_ptr().cast::<u8>();

		for (result, status) in [
			(Ok(()), pinocchio::SUCCESS),
			(
				Err(ProgramError::InvalidAccountData),
				u64::from(ProgramError::InvalidAccountData),
			),
		] {
			RESULT.set(result);
			// SAFETY: `offset` is where `serialize` wrote the instruction data.
			let returned =
				unsafe { __dispatch_entrypoint(input, input.add(offset), record_dispatch) };
			assert_eq!(returned, status);
			assert_eq!(
				DISPATCHED.take(),
				Some((PROGRAM_ID, data.to_vec(), accounts.len()))
			);
		}
	}

	/// With `account-resize`, the walk records each account's original data
	/// length in its header padding, as pinocchio's deserializer does.
	#[cfg(feature = "account-resize")]
	#[test]
	fn walks_record_the_original_data_length() {
		let mut input = serialize(&varied_accounts(), &[]);
		let mut exact = slots::<3>();
		// SAFETY: `input` is a complete loader input built by `serialize`.
		let lengths =
			unsafe { parse_exact_accounts::<3, false>(input.as_mut_ptr().cast(), &mut exact) }.map(
				|accounts| {
					accounts
						.iter()
						// SAFETY: every view points at a record header inside `input`.
						.map(|account| {
							u32::from_le_bytes(unsafe { (*account.account_ptr()).padding })
						})
						.collect::<Vec<_>>()
				},
			);

		assert_eq!(lengths, Ok(std::vec![3, 0, 9]));
	}

	/// A layout of up to eight slots the runtime can write: each is a distinct
	/// account with a varied data length, or a duplicate of an earlier slot.
	fn layouts() -> impl proptest::strategy::Strategy<Value = Vec<Slot>> {
		use proptest::strategy::Strategy;

		proptest::collection::vec(
			(proptest::bool::ANY, 0_u8..24, proptest::num::u8::ANY),
			0..=8,
		)
		.prop_map(|layout| {
			(0_u8..)
				.zip(layout)
				.map(|(position, (duplicate, data_len, pick))| {
					if duplicate && position > 0 {
						Slot::Duplicate(pick % position)
					} else {
						Slot::Account {
							address: position,
							data_len,
						}
					}
				})
				.collect()
		})
	}

	/// Each view's byte offset into the input it was walked from.
	fn offsets(accounts: &[AccountView], input: *const u64) -> Vec<usize> {
		accounts
			.iter()
			.map(|account| account.account_ptr() as usize - input as usize)
			.collect()
	}

	/// Runs pinocchio's deserializer with `MAX_ACCOUNTS` slots over `layout`,
	/// and returns the view offsets and the input it leaves.
	fn deserialized<const MAX_ACCOUNTS: usize>(layout: &[Slot]) -> (Vec<usize>, Vec<u64>) {
		let mut input = serialize(layout, &[1, 2, 3]);
		let mut views = slots::<MAX_ACCOUNTS>();
		// SAFETY: `input` is a complete loader input built by `serialize`.
		let (_program_id, count, _data) = unsafe {
			crate::pinocchio::entrypoint::deserialize::<MAX_ACCOUNTS>(
				input.as_mut_ptr().cast(),
				&mut views,
			)
		};
		// SAFETY: `deserialize` initialized the first `count` slots.
		let accounts =
			unsafe { core::slice::from_raw_parts(views.as_ptr().cast::<AccountView>(), count) };

		(offsets(accounts, input.as_ptr()), input)
	}

	/// Runs the prefix walk with `MAX_ACCOUNTS` slots over `layout`, and
	/// returns the view offsets and the input it leaves.
	fn walked<const MAX_ACCOUNTS: usize>(
		layout: &[Slot],
		shared: bool,
	) -> (Result<Vec<usize>, ProgramError>, Vec<u64>) {
		let mut input = serialize(layout, &[1, 2, 3]);
		let base = input.as_mut_ptr();
		let token = EntrypointInput { input: base.cast() };
		let mut views = slots::<MAX_ACCOUNTS>();
		let accounts = if shared {
			token.parse_leading::<MAX_ACCOUNTS, true>(&mut views)
		} else {
			token.parse_leading::<MAX_ACCOUNTS, false>(&mut views)
		};
		let offsets = accounts.map(|accounts| offsets(accounts, base));

		(offsets, input)
	}

	proptest::proptest! {
		// No failure persistence: it reads the working directory, which Miri's
		// isolation forbids.
		#![proptest_config(proptest::prelude::ProptestConfig {
			cases: if cfg!(miri) { 4 } else { 256 },
			failure_persistence: None,
			..proptest::prelude::ProptestConfig::default()
		})]

		/// For any input the runtime can write, the prefix walk finds the views
		/// pinocchio's deserializer finds with the same array, and leaves the
		/// input byte for byte as it does, including the original data lengths
		/// `account-resize` records.
		#[test]
		fn prefix_walk_matches_pinocchio_deserialize(layout in layouts(), shared in proptest::bool::ANY) {
			let (expected, expected_input) = deserialized::<8>(&layout);
			let (actual, actual_input) = walked::<8>(&layout, shared);
			proptest::prop_assert_eq!(actual, Ok(expected));
			proptest::prop_assert_eq!(actual_input, expected_input);

			// A smaller array reads the leading accounts and ignores the rest.
			let (expected, expected_input) = deserialized::<3>(&layout);
			let (actual, actual_input) = walked::<3>(&layout, shared);
			proptest::prop_assert_eq!(actual, Ok(expected.clone()));
			proptest::prop_assert_eq!(actual_input, expected_input);

			// The bounded walk reads the same accounts when they fit its array,
			// and rejects the input when they do not.
			let mut input = serialize(&layout, &[1, 2, 3]);
			let base = input.as_mut_ptr();
			let mut views = slots::<3>();
			// SAFETY: `input` is a complete loader input built by `serialize`.
			let bounded = unsafe { parse_bounded_accounts::<3, false>(base.cast(), &mut views) }
				.map(|accounts| offsets(accounts, base));
			if layout.len() <= 3 {
				proptest::prop_assert_eq!(bounded, Ok(expected));
			} else {
				proptest::prop_assert_eq!(bounded, Err(PinaProgramError::TooManyAccountKeys.into()));
			}
		}
	}
}
