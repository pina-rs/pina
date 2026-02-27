#![allow(clippy::inline_always)]
#![no_std]

#[cfg(test)]
extern crate std;

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("2nYtoevJCC8AFjdsfmkf8y1jN2nN9k4jVtD7G3f5n1Qe");

#[discriminator]
pub enum PinaBpfInstruction {
	Hello = 0,
}

#[instruction(discriminator = PinaBpfInstruction, variant = Hello)]
pub struct HelloInstruction {}

#[cfg_attr(not(any(test, feature = "bpf-entrypoint")), allow(dead_code))]
#[inline(always)]
fn process_hello(data: &[u8]) -> ProgramResult {
	let _ = HelloInstruction::try_from_bytes(data)?;
	log!("Hello, World!");
	Ok(())
}

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use pina::*;

	use super::*;

	nostd_entrypoint!(process_instruction);

	#[inline(always)]
	pub fn process_instruction(
		program_id: &Address,
		_accounts: &[AccountView],
		instruction_data: &[u8],
	) -> ProgramResult {
		let instruction: PinaBpfInstruction = parse_instruction(program_id, &ID, instruction_data)?;
		match instruction {
			PinaBpfInstruction::Hello => process_hello(instruction_data),
		}
	}
}

#[cfg(test)]
mod tests {
	use std::format;
	use std::fs;
	use std::path::Path;
	use std::string::String;

	use super::*;

	fn bpf_binary_path() -> String {
		format!(
			"{}/../../target/bpfel-unknown-none/release/libpina_bpf.so",
			env!("CARGO_MANIFEST_DIR")
		)
	}

	#[test]
	fn parse_instruction_accepts_matching_program_id() {
		let data = [PinaBpfInstruction::Hello as u8];
		let instruction = parse_instruction::<PinaBpfInstruction>(&ID, &ID, &data);
		assert!(matches!(instruction, Ok(PinaBpfInstruction::Hello)));
	}

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let wrong_program_id: Address = [7u8; 32].into();
		let data = [PinaBpfInstruction::Hello as u8];
		let result = parse_instruction::<PinaBpfInstruction>(&wrong_program_id, &ID, &data);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}

	#[test]
	fn process_hello_accepts_instruction_data() {
		let data = [PinaBpfInstruction::Hello as u8];
		assert!(process_hello(&data).is_ok());
	}

	#[test]
	fn parse_instruction_rejects_unknown_discriminator() {
		let data = [u8::MAX];
		let result = parse_instruction::<PinaBpfInstruction>(&ID, &ID, &data);
		assert!(matches!(result, Err(ProgramError::InvalidInstructionData)));
	}

	#[test]
	fn process_hello_rejects_empty_instruction_data() {
		let result = process_hello(&[]);
		assert!(matches!(result, Err(ProgramError::InvalidInstructionData)));
	}

	#[test]
	#[ignore = "requires `cargo +nightly build-bpf` artifact"]
	fn bpf_build_produces_artifact() {
		let artifact = bpf_binary_path();
		assert!(
			Path::new(&artifact).is_file(),
			"missing BPF artifact at {artifact}; run `cargo +nightly build-bpf`"
		);
	}

	#[test]
	#[ignore = "requires `cargo +nightly build-bpf` artifact"]
	fn bpf_build_artifact_is_elf() {
		let artifact = bpf_binary_path();
		let bytes = fs::read(&artifact)
			.unwrap_or_else(|error| panic!("failed to read BPF artifact at {artifact}: {error}"));
		assert!(
			bytes.starts_with(b"\x7fELF"),
			"artifact at {artifact} is not an ELF binary"
		);
	}
}
