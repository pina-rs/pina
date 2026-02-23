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
pub struct TodoState {
	pub owner: Address,
	pub bump: u8,
	pub completed: PodBool,
	pub digest: [u8; 32],
}

#[instruction(discriminator = TodoInstruction, variant = Initialize)]
pub struct InitializeInstruction {
	pub bump: u8,
	pub digest: [u8; 32],
}

#[instruction(discriminator = TodoInstruction, variant = ToggleCompleted)]
pub struct ToggleCompletedInstruction {}

#[instruction(discriminator = TodoInstruction, variant = UpdateDigest)]
pub struct UpdateDigestInstruction {
	pub digest: [u8; 32],
}

const TODO_SEED: &[u8] = b"todo";

#[macro_export]
macro_rules! todo_seeds {
	($owner:expr) => {
		&[TODO_SEED, $owner]
	};
	($owner:expr, $bump:expr) => {
		&[TODO_SEED, $owner, &[$bump]]
	};
}

#[derive(Accounts, Debug)]
pub struct InitializeAccounts<'a> {
	pub owner: &'a AccountView,
	pub todo: &'a AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct UpdateAccounts<'a> {
	pub owner: &'a AccountView,
	pub todo: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = InitializeInstruction::try_from_bytes(data)?;
		let owner = self.owner.address();
		let seeds = todo_seeds!(owner.as_ref());
		let seeds_with_bump = todo_seeds!(owner.as_ref(), args.bump);

		self.owner.assert_signer()?;
		self.todo
			.assert_empty()?
			.assert_writable()?
			.assert_seeds_with_bump(seeds_with_bump, &ID)?;
		self.system_program.assert_address(&system::ID)?;

		create_program_account_with_bump::<TodoState>(
			self.todo, self.owner, &ID, seeds, args.bump,
		)?;

		let todo = self.todo.as_account_mut::<TodoState>(&ID)?;
		*todo = TodoState::builder()
			.owner(*owner)
			.bump(args.bump)
			.completed(PodBool::from_bool(false))
			.digest(args.digest)
			.build();

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for UpdateAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let owner = self.owner.address();

		self.owner.assert_signer()?;
		self.todo
			.assert_not_empty()?
			.assert_writable()?
			.assert_type::<TodoState>(&ID)?;

		let todo = self.todo.as_account::<TodoState>(&ID)?;
		self.owner.assert_address(&todo.owner)?;
		let seeds_with_bump = todo_seeds!(owner.as_ref(), todo.bump);
		self.todo.assert_seeds_with_bump(seeds_with_bump, &ID)?;

		match parse_instruction::<TodoInstruction>(&ID, &ID, data)? {
			TodoInstruction::ToggleCompleted => {
				let _ = ToggleCompletedInstruction::try_from_bytes(data)?;
				let todo = self.todo.as_account_mut::<TodoState>(&ID)?;
				let completed = bool::from(todo.completed);
				todo.completed = PodBool::from_bool(!completed);
			}
			TodoInstruction::UpdateDigest => {
				let args = UpdateDigestInstruction::try_from_bytes(data)?;
				let todo = self.todo.as_account_mut::<TodoState>(&ID)?;
				todo.digest = args.digest;
			}
			TodoInstruction::Initialize => return Err(ProgramError::InvalidInstructionData),
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
		accounts: &[AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: TodoInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			TodoInstruction::Initialize => InitializeAccounts::try_from(accounts)?.process(data),
			TodoInstruction::ToggleCompleted | TodoInstruction::UpdateDigest => {
				UpdateAccounts::try_from(accounts)?.process(data)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use core::mem::size_of;

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
		assert_eq!(size_of::<TodoState>(), 67);
	}

	#[test]
	fn initialize_instruction_layout() {
		assert_eq!(size_of::<InitializeInstruction>(), 34);
		assert!(InitializeInstruction::matches_discriminator(&[
			TodoInstruction::Initialize as u8
		]));
	}

	#[test]
	fn update_digest_layout() {
		assert_eq!(size_of::<UpdateDigestInstruction>(), 33);
		assert!(UpdateDigestInstruction::matches_discriminator(&[
			TodoInstruction::UpdateDigest as u8
		]));
	}

	#[test]
	fn pod_bool_conversion() {
		let value = PodBool::from_bool(true);
		assert!(bool::from(value));
	}

	#[test]
	fn program_id_is_valid() {
		assert_ne!(ID, Address::default());
	}
}
