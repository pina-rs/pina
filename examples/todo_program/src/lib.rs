//! Todo program example built with pina.
//!
//! This example demonstrates a compact PDA-backed account with three
//! instruction paths:
//! - `Initialize`: creates a todo account for an authority.
//! - `ToggleCompleted`: flips the completion flag.
//! - `UpdateDigest`: updates a fixed-size digest payload.

#![allow(clippy::inline_always)]
#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("Fc5A5xvNQ6w7kn2P7FpC18JNpDutLCRa14Q6gttxyPjd");

#[discriminator]
pub enum TodoInstruction {
	Initialize = 0,
	ToggleCompleted = 1,
	UpdateDigest = 2,
}

#[discriminator]
pub enum TodoAccount {
	TodoState = 1,
}

#[account(discriminator = TodoAccount)]
#[pda(seeds = [TODO_SEED, owner: Address], bump = bump)]
pub struct TodoState {
	pub owner: Address,
	pub bump: u8,
	pub completed: bool,
	pub digest: [u8; 32],
}

#[instruction(discriminator = TodoInstruction::Initialize)]
pub struct InitializeInstruction {
	pub bump: u8,
	pub digest: [u8; 32],
}

#[instruction(discriminator = TodoInstruction::ToggleCompleted)]
pub struct ToggleCompletedInstruction {}

#[instruction(discriminator = TodoInstruction::UpdateDigest)]
pub struct UpdateDigestInstruction {
	pub digest: [u8; 32],
}

/// Seed prefix for todo PDAs.
const TODO_SEED: &[u8] = b"todo";

#[derive(Accounts, Debug)]
pub struct InitializeAccounts<'a> {
	pub owner: &'a AccountView,
	pub todo: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct UpdateAccounts<'a> {
	pub owner: &'a AccountView,
	pub todo: &'a mut AccountView,
}

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		// Parse instruction and prepare PDA seeds
		let args = InitializeInstruction::try_from_bytes(data)?;
		let owner = self.owner.address();
		let seeds = TodoState::seeds(owner);
		let seeds_with_bump = seeds.with_bump(args.bump);

		// Validate accounts
		self.owner.assert_signer()?;
		let canonical_bump = self.todo.assert_canonical_bump(&seeds.as_slices(), &ID)?;
		if canonical_bump != args.bump {
			return Err(ProgramError::InvalidSeeds);
		}
		self.todo
			.assert_empty()?
			.assert_seeds_with_bump(&seeds_with_bump.as_slices(), &ID)?;
		self.system_program.assert_address(&system::ID)?;

		// Create the PDA account
		CreateProgramAccountWithBump {
			account: self.todo,
			payer: self.owner,
			owner: &ID,
			seeds: &seeds.as_slices(),
			bump: args.bump,
		}
		.invoke::<TodoState>()?;

		// Initialize account data
		let mut todo = self.todo.as_account_mut::<TodoState>(&ID)?;
		todo.owner = *owner;
		todo.bump = args.bump;
		todo.completed.set(false);
		todo.digest = args.digest;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for UpdateAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		// Parse instruction data
		let owner = self.owner.address();
		let instruction = parse_instruction::<TodoInstruction>(&ID, &ID, data)?;

		// Reject invalid instruction for this context
		if instruction == TodoInstruction::Initialize {
			return Err(ProgramError::InvalidInstructionData);
		}

		// Validate accounts
		self.owner.assert_signer()?;
		self.todo
			.assert_not_empty()?
			.assert_type::<TodoState>(&ID)?;

		let stored_owner = {
			let todo = self.todo.as_account::<TodoState>(&ID)?;
			todo.owner
		};
		self.owner.assert_address(&stored_owner)?;

		// Verify the todo is the PDA for the owner, using the stored bump
		// field (avoids re-deriving the canonical bump on-chain).
		TodoState::assert_seeds(self.todo, owner, &ID)?;

		// Execute instruction
		match instruction {
			TodoInstruction::ToggleCompleted => {
				let _ = ToggleCompletedInstruction::try_from_bytes(data)?;
				let mut todo = self.todo.as_account_mut::<TodoState>(&ID)?;
				let completed = todo.completed.get();

				todo.completed.set(!completed);
			}
			TodoInstruction::UpdateDigest => {
				let args = UpdateDigestInstruction::try_from_bytes(data)?;
				let mut todo = self.todo.as_account_mut::<TodoState>(&ID)?;

				todo.digest = args.digest;
			}
			TodoInstruction::Initialize => {
				// Already handled above; unreachable
				return Err(ProgramError::InvalidInstructionData);
			}
		}

		Ok(())
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
		data: &[u8],
	) -> ProgramResult {
		let instruction: TodoInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			TodoInstruction::Initialize => {
				InitializeAccounts::try_from((program_id, accounts))?.process(data)
			}
			TodoInstruction::ToggleCompleted | TodoInstruction::UpdateDigest => {
				UpdateAccounts::try_from((program_id, accounts))?.process(data)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn discriminator_values() {
		assert_eq!(TodoInstruction::Initialize as u8, 0);
		assert_eq!(TodoInstruction::ToggleCompleted as u8, 1);
		assert_eq!(TodoInstruction::UpdateDigest as u8, 2);
	}

	#[test]
	fn instruction_roundtrip() {
		assert!(TodoInstruction::try_from(0u8).is_ok());
		assert!(TodoInstruction::try_from(1u8).is_ok());
		assert!(TodoInstruction::try_from(2u8).is_ok());
		assert!(TodoInstruction::try_from(99u8).is_err());
	}

	#[test]
	fn todo_state_layout() {
		assert_eq!(TodoState::SIZE, 67);
	}

	#[test]
	fn initialize_instruction_layout() {
		assert_eq!(InitializeInstruction::SIZE, 34);
		assert!(InitializeInstruction::matches_discriminator(&[
			TodoInstruction::Initialize as u8
		]));
	}

	#[test]
	fn update_digest_layout() {
		assert_eq!(UpdateDigestInstruction::SIZE, 33);
		assert!(UpdateDigestInstruction::matches_discriminator(&[
			TodoInstruction::UpdateDigest as u8
		]));
	}

	#[test]
	fn pod_bool_conversion() {
		let value = PodBool::from(true);
		assert!(bool::from(value));
	}

	#[test]
	fn program_id_is_valid() {
		assert_ne!(ID, Address::default());
	}
}
