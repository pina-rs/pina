#![allow(unsafe_code)]

//! The explicit program ID opt-out preserves instruction and account checks
//! through both generated routers. A separate binary keeps the entrypoint
//! discriminator unique within its crate.

use core::cell::Cell;

use pina::*;
use pinocchio::account::MAX_PERMITTED_DATA_INCREASE;
use pinocchio::account::RuntimeAccount;
use pinocchio::entrypoint::NON_DUP_MARKER;

declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

/// Instruction router with the explicit deployment identity opt-out.
#[discriminator(entrypoint, skip_program_id_check)]
pub enum Instruction {
	/// Record successful routing.
	Run = 0,
}

/// A mutable account whose signer constraint remains enforced when enabled.
#[derive(Accounts)]
pub struct RunAccounts<'a> {
	/// Authority supplied to the handler.
	#[cfg_attr(feature = "validation", pina(validate(signer)))]
	pub authority: &'a mut AccountView,
}

std::thread_local! {
	static HANDLED: Cell<bool> = const { Cell::new(false) };
}

impl<'a> ProcessAccountInfos<'a> for RunAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		HANDLED.set(true);
		Ok(())
	}
}

/// Build aligned loader input containing one empty account and a mismatched
/// runtime program ID. The account privileges vary independently of that ID.
fn run(dispatch: bool, signer: bool, writable: bool, data: &[u8]) -> (u64, bool) {
	let mut bytes = 1_u64.to_le_bytes().to_vec();
	let mut header = [0; size_of::<RuntimeAccount>()];
	header[0] = NON_DUP_MARKER;
	header[1] = u8::from(signer);
	header[2] = u8::from(writable);
	header[8] = 1;
	header[40..72].copy_from_slice(ID.as_ref());
	bytes.extend(header);
	bytes.resize(
		bytes.len() + MAX_PERMITTED_DATA_INCREASE + size_of::<u64>(),
		0,
	);
	bytes.extend((data.len() as u64).to_le_bytes());
	let offset = bytes.len();
	bytes.extend(data);
	bytes.extend(Address::new_from_array([9; 32]).as_ref());
	let mut input = bytes
		.chunks(size_of::<u64>())
		.map(|chunk| {
			let mut word = [0; 8];
			word[..chunk.len()].copy_from_slice(chunk);
			u64::from_ne_bytes(word)
		})
		.collect::<std::vec::Vec<_>>();
	let input = input.as_mut_ptr().cast::<u8>();
	HANDLED.set(false);
	// SAFETY: the aligned buffer contains a complete loader input, with its
	// instruction data starting at `offset`, and remains alive for the call.
	let status = unsafe {
		if dispatch {
			__dispatch_entrypoint(input, input.add(offset), Instruction::__dispatch)
		} else {
			__process_entrypoint::<2>(input, Instruction::process_instruction)
		}
	};

	(status, HANDLED.get())
}

#[test]
fn both_routers_execute_under_a_different_program_id() {
	for dispatch in [false, true] {
		assert_eq!(run(dispatch, true, true, &[0]), (pinocchio::SUCCESS, true));
	}
}

#[test]
fn both_routers_still_reject_invalid_instructions() {
	for dispatch in [false, true] {
		for data in [&[][..], &[9][..]] {
			assert_eq!(
				run(dispatch, true, true, data),
				(u64::from(ProgramError::InvalidInstructionData), false)
			);
		}
	}
}

#[test]
fn both_routers_still_reject_read_only_mutable_accounts() {
	for dispatch in [false, true] {
		assert_eq!(
			run(dispatch, true, false, &[0]),
			(u64::from(ProgramError::InvalidAccountData), false)
		);
	}
}

#[cfg(feature = "validation")]
#[test]
fn both_routers_still_require_declared_signers() {
	for dispatch in [false, true] {
		assert_eq!(
			run(dispatch, false, true, &[0]),
			(u64::from(ProgramError::MissingRequiredSignature), false)
		);
	}
}
