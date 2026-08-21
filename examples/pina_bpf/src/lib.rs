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
const PROP_AMM_PROGRAM_ID: Address = address!("55555555555555555555555555555555555555555555");

/// Seed namespace for the PDA used to authorize the generated CPI regression.
pub const CPI_AUTHORITY_SEED_PREFIX: &[u8] = b"cpi-authority";

/// Seed namespace for the account-creation regression.
pub const STATE_SEED_PREFIX: &[u8] = b"state";

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
pub struct State {
	pub bump: u8,
}

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

#[allow(clippy::trivially_copy_pass_by_ref)]
#[cfg(feature = "cpi-runtime-tests")]
mod prop_amm_cpi {
	use super::*;

	pub struct PropAmmProgram;

	impl CpiProgramId for PropAmmProgram {
		const ID: Address = PROP_AMM_PROGRAM_ID;
	}

	pub type ProgramAccount<'a> = Program<'a, PropAmmProgram>;

	#[derive(Clone, Copy)]
	pub struct RotateAuthorityAccounts<'a> {
		oracle: CpiHandle<'a>,
		authority: CpiHandle<'a>,
	}

	impl<'a> RotateAuthorityAccounts<'a> {
		pub fn new(
			oracle: &'a AccountView,
			authority: &'a AccountView,
		) -> Result<Self, ProgramError> {
			Ok(Self {
				oracle: CpiHandle::writable(oracle)?,
				authority: CpiHandle::readonly_signer(authority),
			})
		}
	}

	impl<'a> ToCpiAccounts<'a, 2> for RotateAuthorityAccounts<'a> {
		fn to_cpi_handles(&self) -> [CpiHandle<'a>; 2] {
			[self.oracle, self.authority]
		}
	}

	pub struct RotateAuthority<'a> {
		accounts: RotateAuthorityAccounts<'a>,
		new_authority: Address,
	}

	impl<'a> RotateAuthority<'a> {
		pub const fn new(accounts: RotateAuthorityAccounts<'a>, new_authority: Address) -> Self {
			Self {
				accounts,
				new_authority,
			}
		}

		pub fn invoke(&self, program: &ProgramAccount<'_>) -> ProgramResult {
			self.invoke_signed(program, &[])
		}

		pub fn invoke_signed(
			&self,
			program: &ProgramAccount<'_>,
			signers: &[Signer<'_, '_>],
		) -> ProgramResult {
			let mut data = [0u8; 1 + ADDRESS_BYTES];
			data[0] = 2;
			data[1..].copy_from_slice(self.new_authority.as_ref());
			let context = CpiContext::new(*program, self.accounts);

			context.invoke(&data, signers)
		}
	}
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
			let program = prop_amm_cpi::ProgramAccount::new(self.prop_amm_program)?;
			let accounts = prop_amm_cpi::RotateAuthorityAccounts::new(self.oracle, self.authority)?;

			prop_amm_cpi::RotateAuthority::new(accounts, args.new_authority).invoke(&program)
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
			let bump = [args.bump];
			let signer = PdaSigner::from_slices([CPI_AUTHORITY_SEED_PREFIX, bump.as_slice()]);
			let signers = [signer.as_signer()];

			self.authority
				.assert_seeds_with_bump(&[CPI_AUTHORITY_SEED_PREFIX, bump.as_slice()], &ID)?;
			let program = prop_amm_cpi::ProgramAccount::new(self.prop_amm_program)?;
			let accounts = prop_amm_cpi::RotateAuthorityAccounts::new(self.oracle, self.authority)?;

			prop_amm_cpi::RotateAuthority::new(accounts, args.new_authority)
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
			create_program_account_with_bump::<State>(
				self.state,
				self.payer,
				&ID,
				&[STATE_SEED_PREFIX],
				args.bump,
			)?;

			let mut state = self.state.as_account_mut::<State>(&ID)?;
			state.bump = args.bump;

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
				ForwardRotateAccounts::try_from(accounts)?.process(instruction_data)
			}
			PinaBpfInstruction::ForwardRotateWithPda => {
				ForwardRotateWithPdaAccounts::try_from(accounts)?.process(instruction_data)
			}
			PinaBpfInstruction::CreatePda => {
				CreatePdaAccounts::try_from(accounts)?.process(instruction_data)
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
