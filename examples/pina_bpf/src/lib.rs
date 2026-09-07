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

#[cfg(feature = "cpi-runtime-tests")]
use prop_amm_program_cpi as prop_amm_cpi;

/// Seed namespace for the PDA used to authorize the generated CPI regression.
pub const SEED_CPI_AUTHORITY_PREFIX: &[u8] = b"cpi-authority";

/// Seed namespace for the account-creation regression.
pub const SEED_STATE_PREFIX: &[u8] = b"state";

#[discriminator]
pub enum PinaBpfInstruction {
	Hello = 0,
	ForwardRotateWithSigner = 1,
	ForwardRotateWithPda = 2,
	CreatePda = 3,
}

#[discriminator]
pub enum PinaBpfAccountType {
	State = 1,
}

#[account(discriminator = PinaBpfAccountType)]
#[pda(seeds = [SEED_STATE_PREFIX], bump = bump)]
pub struct State {
	pub bump: u8,
}

/// Typed schema for the data-free PDA that signs the CPI regression.
#[pda(seeds = [SEED_CPI_AUTHORITY_PREFIX])]
pub struct AuthorityState {}

#[instruction(discriminator = PinaBpfInstruction::Hello)]
pub struct HelloInstruction {}

#[instruction(discriminator = PinaBpfInstruction::ForwardRotateWithSigner)]
pub struct ForwardRotateWithSignerInstruction {
	pub new_authority: Address,
}

#[instruction(discriminator = PinaBpfInstruction::ForwardRotateWithPda)]
pub struct ForwardRotateWithPdaInstruction {
	pub bump: u8,
	pub new_authority: Address,
}

#[instruction(discriminator = PinaBpfInstruction::CreatePda)]
pub struct CreatePdaInstruction {
	pub bump: u8,
}

#[derive(Accounts, Debug)]
pub struct ForwardRotateAccounts<'a> {
	pub oracle: &'a mut AccountView,
	pub authority: &'a AccountView,
	pub prop_amm_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct ForwardRotateWithPdaAccounts<'a> {
	pub oracle: &'a mut AccountView,
	pub authority: &'a AccountView,
	pub prop_amm_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct CreatePdaAccounts<'a> {
	pub payer: &'a AccountView,
	pub state: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

#[cfg_attr(not(any(test, feature = "bpf-entrypoint")), allow(dead_code))]
#[inline(always)]
fn process_hello(data: &[u8]) -> ProgramResult {
	let _ = HelloInstruction::try_from_bytes(data)?;
	log!("Hello, World!");
	Ok(())
}

impl<'a> ProcessAccountInfos<'a> for ForwardRotateAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		#[cfg(feature = "cpi-runtime-tests")]
		{
			let args = ForwardRotateWithSignerInstruction::try_from_bytes(data)?;

			self.authority.assert_signer()?;
			let program = prop_amm_cpi::ProgramAccount::try_new(self.prop_amm_program)?;

			prop_amm_cpi::RotateAuthority {
				oracle: self.oracle,
				authority: self.authority,
				ix: prop_amm_cpi::RotateAuthorityIx {
					new_authority: &args.new_authority,
				},
			}
			.invoke(&program)
		}

		#[cfg(not(feature = "cpi-runtime-tests"))]
		{
			let _ = (self, data);
			Err(ProgramError::InvalidInstructionData)
		}
	}
}

impl<'a> ProcessAccountInfos<'a> for ForwardRotateWithPdaAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		#[cfg(feature = "cpi-runtime-tests")]
		{
			let args = ForwardRotateWithPdaInstruction::try_from_bytes(data)?;
			let seeds = AuthorityState::seeds();
			let canonical_bump = self
				.authority
				.assert_canonical_bump(&seeds.as_slices(), &ID)?;
			if canonical_bump != args.bump {
				return Err(ProgramError::InvalidSeeds);
			}
			let seeds_with_bump = seeds.with_bump(args.bump);
			let signer = seeds_with_bump.to_signer();
			let signers = [signer.as_signer()];

			self.authority
				.assert_seeds_with_bump(&seeds_with_bump.as_slices(), &ID)?;
			let program = prop_amm_cpi::ProgramAccount::try_new(self.prop_amm_program)?;

			prop_amm_cpi::RotateAuthority {
				oracle: self.oracle,
				authority: self.authority,
				ix: prop_amm_cpi::RotateAuthorityIx {
					new_authority: &args.new_authority,
				},
			}
			.invoke_signed(&program, &signers)
		}

		#[cfg(not(feature = "cpi-runtime-tests"))]
		{
			let _ = (self, data);
			Err(ProgramError::InvalidInstructionData)
		}
	}
}

impl<'a> ProcessAccountInfos<'a> for CreatePdaAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		#[cfg(feature = "cpi-runtime-tests")]
		{
			let args = CreatePdaInstruction::try_from_bytes(data)?;

			self.payer.assert_signer()?.assert_writable()?;
			self.state.assert_empty()?;
			self.system_program.assert_address(&system::ID)?;
			CreateProgramAccountWithBump {
				account: self.state,
				payer: self.payer,
				owner: &ID,
				seeds: &[SEED_STATE_PREFIX],
				bump: args.bump,
			}
			.invoke_with::<State>(|state| {
				state.bump = args.bump;

				Ok(())
			})?;

			Ok(())
		}

		#[cfg(not(feature = "cpi-runtime-tests"))]
		{
			let _ = (self, data);
			Err(ProgramError::InvalidInstructionData)
		}
	}
}

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use pina::*;

	use super::*;

	nostd_entrypoint!(process_instruction);

	#[inline(always)]
	pub fn process_instruction(
		program_id: &Address,
		accounts: &mut [AccountView],
		instruction_data: &[u8],
	) -> ProgramResult {
		let instruction: PinaBpfInstruction = parse_instruction(program_id, &ID, instruction_data)?;

		match instruction {
			PinaBpfInstruction::Hello => process_hello(instruction_data),
			PinaBpfInstruction::ForwardRotateWithSigner => {
				ForwardRotateAccounts::try_from((program_id, accounts))?.process(instruction_data)
			}
			PinaBpfInstruction::ForwardRotateWithPda => {
				ForwardRotateWithPdaAccounts::try_from((program_id, accounts))?
					.process(instruction_data)
			}
			PinaBpfInstruction::CreatePda => {
				CreatePdaAccounts::try_from((program_id, accounts))?.process(instruction_data)
			}
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
