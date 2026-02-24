#![cfg_attr(target_arch = "bpf", no_std)]

use pinocchio::AccountView;
use pinocchio::Address;
use pinocchio::ProgramResult;
use pinocchio::no_allocator;
use pinocchio::nostd_panic_handler;
use pinocchio::program_entrypoint;
use solana_program_log::log;

nostd_panic_handler!();
no_allocator!();
program_entrypoint!(process_instruction);

fn process_instruction(
	_program_id: &Address,
	_accounts: &[AccountView],
	_instruction_data: &[u8],
) -> ProgramResult {
	log("Hello, World!");
	Ok(())
}

#[cfg(test)]
mod tests {
	use mollusk_svm::Mollusk;
	use mollusk_svm::result::Check;
	use solana_instruction::Instruction;

	#[test]
	#[ignore = "requires `cargo +nightly-2025-10-15 build-bpf` artifact"]
	pub fn hello_world() {
		let program_id = [2u8; 32].into();
		let program_path = format!(
			"{}/../../target/bpfel-unknown-none/release/libpinocchio_bpf_starter",
			env!("CARGO_MANIFEST_DIR")
		);
		let mollusk = Mollusk::new(&program_id, &program_path);
		mollusk.process_and_validate_instruction(
			&Instruction {
				program_id,
				accounts: vec![],
				data: vec![],
			},
			&[],
			&[Check::success()],
		);
	}
}
