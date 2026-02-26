//! Instruction introspection helpers for reading the Instructions sysvar.
//!
//! These utilities enable on-chain programs to inspect the transaction-level
//! instruction list. Common use cases include:
//!
//! - **Flash loan guards** — verify the current instruction is not being invoked
//!   via CPI so that atomic flash-loan exploits are prevented.
//! - **CPI depth checks** — ensure instructions are top-level calls.
//! - **Sandwich detection** — check whether a specific program appears before or
//!   after the current instruction in the transaction.
//!
//! All functions accept a reference to the Instructions sysvar account
//! (`&AccountView`) and validate its address before reading data.

use pinocchio::AccountView;
use pinocchio::Address;
use pinocchio::error::ProgramError;
use pinocchio::sysvars::instructions::Instructions;

use crate::ProgramResult;

/// Verifies the current instruction is not being invoked via CPI.
///
/// This is useful for flash loan guards: a program can ensure it is the
/// top-level caller by checking that the instruction at the current index
/// in the sysvar has a matching `program_id`.
///
/// # Arguments
///
/// * `instructions_account` - The Instructions sysvar account.
/// * `program_id` - The expected program ID of the current instruction (i.e.
///   the program performing the check).
///
/// # Errors
///
/// Returns `ProgramError::UnsupportedSysvar` if the account address does not
/// match the Instructions sysvar ID.
///
/// Returns `ProgramError::InvalidInstructionData` if the current instruction
/// index is out of bounds.
///
/// Returns `ProgramError::InvalidAccountData` if the program ID at the
/// current instruction index does not match the provided `program_id`,
/// indicating the instruction is being invoked via CPI.
///
/// # Example
///
/// ```ignore
/// use pina::introspection::assert_no_cpi;
///
/// fn process(accounts: &[AccountView], program_id: &Address) -> ProgramResult {
///     let instructions_account = &accounts[0];
///     // Ensure we are not being called via CPI
///     assert_no_cpi(instructions_account, program_id)?;
///     // ... rest of the instruction logic
///     Ok(())
/// }
/// ```
pub fn assert_no_cpi(instructions_account: &AccountView, program_id: &Address) -> ProgramResult {
	let instructions = Instructions::try_from(instructions_account)?;
	let current_index = instructions.load_current_index();
	let current_ix = instructions.load_instruction_at(current_index as usize)?;

	if current_ix.get_program_id() != program_id {
		return Err(ProgramError::InvalidAccountData);
	}

	Ok(())
}

/// Returns the total number of instructions in the transaction.
///
/// # Arguments
///
/// * `instructions_account` - The Instructions sysvar account.
///
/// # Errors
///
/// Returns `ProgramError::UnsupportedSysvar` if the account address does not
/// match the Instructions sysvar ID.
///
/// # Example
///
/// ```ignore
/// use pina::introspection::get_instruction_count;
///
/// fn process(accounts: &[AccountView]) -> ProgramResult {
///     let instructions_account = &accounts[0];
///     let count = get_instruction_count(instructions_account)?;
///     // e.g. reject transactions with too many instructions
///     if count > 5 {
///         return Err(ProgramError::InvalidInstructionData);
///     }
///     Ok(())
/// }
/// ```
pub fn get_instruction_count(instructions_account: &AccountView) -> Result<u16, ProgramError> {
	let instructions = Instructions::try_from(instructions_account)?;
	Ok(instructions.num_instructions() as u16)
}

/// Returns the index of the currently executing instruction.
///
/// # Arguments
///
/// * `instructions_account` - The Instructions sysvar account.
///
/// # Errors
///
/// Returns `ProgramError::UnsupportedSysvar` if the account address does not
/// match the Instructions sysvar ID.
///
/// # Example
///
/// ```ignore
/// use pina::introspection::get_current_instruction_index;
///
/// fn process(accounts: &[AccountView]) -> ProgramResult {
///     let instructions_account = &accounts[0];
///     let index = get_current_instruction_index(instructions_account)?;
///     // e.g. ensure this is the first instruction in the transaction
///     if index != 0 {
///         return Err(ProgramError::InvalidInstructionData);
///     }
///     Ok(())
/// }
/// ```
pub fn get_current_instruction_index(
	instructions_account: &AccountView,
) -> Result<u16, ProgramError> {
	let instructions = Instructions::try_from(instructions_account)?;
	Ok(instructions.load_current_index())
}

/// Checks if any instruction before the current one targets the given program.
///
/// This is useful for detecting whether a specific program (e.g. a DEX or
/// lending protocol) has already executed an instruction earlier in the
/// transaction.
///
/// # Arguments
///
/// * `instructions_account` - The Instructions sysvar account.
/// * `program_id` - The program ID to search for.
///
/// # Errors
///
/// Returns `ProgramError::UnsupportedSysvar` if the account address does not
/// match the Instructions sysvar ID.
///
/// Returns `ProgramError::InvalidInstructionData` if any instruction index
/// is out of bounds (should not happen for well-formed sysvar data).
///
/// # Example
///
/// ```ignore
/// use pina::introspection::has_instruction_before;
///
/// fn process(
///     accounts: &[AccountView],
///     suspect_program: &Address,
/// ) -> ProgramResult {
///     let instructions_account = &accounts[0];
///     if has_instruction_before(instructions_account, suspect_program)? {
///         // Another instruction targeting `suspect_program` was already
///         // executed in this transaction before us.
///         return Err(ProgramError::InvalidInstructionData);
///     }
///     Ok(())
/// }
/// ```
pub fn has_instruction_before(
	instructions_account: &AccountView,
	program_id: &Address,
) -> Result<bool, ProgramError> {
	let instructions = Instructions::try_from(instructions_account)?;
	let current_index = instructions.load_current_index() as usize;

	for i in 0..current_index {
		let ix = instructions.load_instruction_at(i)?;
		if ix.get_program_id() == program_id {
			return Ok(true);
		}
	}

	Ok(false)
}

/// Checks if any instruction after the current one targets the given program.
///
/// This is useful for detecting whether a specific program is scheduled to
/// execute later in the same transaction (e.g. to detect sandwich attacks or
/// ensure a repayment instruction follows a borrow).
///
/// # Arguments
///
/// * `instructions_account` - The Instructions sysvar account.
/// * `program_id` - The program ID to search for.
///
/// # Errors
///
/// Returns `ProgramError::UnsupportedSysvar` if the account address does not
/// match the Instructions sysvar ID.
///
/// Returns `ProgramError::InvalidInstructionData` if any instruction index
/// is out of bounds (should not happen for well-formed sysvar data).
///
/// # Example
///
/// ```ignore
/// use pina::introspection::has_instruction_after;
///
/// fn process(
///     accounts: &[AccountView],
///     repay_program: &Address,
/// ) -> ProgramResult {
///     let instructions_account = &accounts[0];
///     if !has_instruction_after(instructions_account, repay_program)? {
///         // No repayment instruction follows — reject the borrow.
///         return Err(ProgramError::InvalidInstructionData);
///     }
///     Ok(())
/// }
/// ```
pub fn has_instruction_after(
	instructions_account: &AccountView,
	program_id: &Address,
) -> Result<bool, ProgramError> {
	let instructions = Instructions::try_from(instructions_account)?;
	let current_index = instructions.load_current_index() as usize;
	let total = instructions.num_instructions();

	for i in (current_index + 1)..total {
		let ix = instructions.load_instruction_at(i)?;
		if ix.get_program_id() == program_id {
			return Ok(true);
		}
	}

	Ok(false)
}
