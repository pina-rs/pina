//! The runtime half of [`crate::nostd_entrypoint!`] and
//! [`crate::nostd_entrypoint_alloc!`].

use core::mem::MaybeUninit;

use crate::AccountView;
use crate::Address;
use crate::ProgramResult;

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
	use crate::ProgramError;
	use crate::pinocchio::account::MAX_PERMITTED_DATA_INCREASE;
	use crate::pinocchio::account::RuntimeAccount;
	use crate::pinocchio::entrypoint::NON_DUP_MARKER;

	const PROGRAM_ID: Address = Address::new_from_array([7; 32]);

	/// One account slot in a serialized loader input.
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
}
