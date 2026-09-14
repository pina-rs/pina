//! Hand-written pinocchio half of the hello comparison: the floor a framework
//! is measured against.
#![no_std]

use pinocchio::account::AccountView;
use pinocchio::address::Address;
use pinocchio::error::ProgramResult;

pinocchio::program_entrypoint!(process_instruction);
pinocchio::no_allocator!();
pinocchio::nostd_panic_handler!();

fn process_instruction(
	_program_id: &Address,
	_accounts: &mut [AccountView],
	_data: &[u8],
) -> ProgramResult {
	solana_program_log::log("Hello, Solana!");
	Ok(())
}
