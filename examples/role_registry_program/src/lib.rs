//! Role-based registry and configuration scaffold built with pina.
//!
//! This example shows a practical administrative flow:
//! - initialize a registry config PDA
//! - register per-role PDA entries
//! - update role permissions
//! - deactivate or rotate administrative control

#![allow(clippy::inline_always)]
#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("3B7roNNQLnW43Par9AfTuVzEqZx7yPtXRA9K3Ev7RHyX");

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
		let instruction: RegistryInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			RegistryInstruction::Initialize => {
				InitializeAccounts::try_from((program_id, accounts))?.process(data)
			}

			RegistryInstruction::AddRole => {
				AddRoleAccounts::try_from((program_id, accounts))?.process(data)
			}

			RegistryInstruction::UpdateRole => {
				UpdateRoleAccounts::try_from((program_id, accounts))?.process(data)
			}

			RegistryInstruction::DeactivateRole => {
				DeactivateRoleAccounts::try_from((program_id, accounts))?.process(data)
			}

			RegistryInstruction::RotateAdmin => {
				RotateAdminAccounts::try_from((program_id, accounts))?.process(data)
			}
		}
	}
}

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryError {
	InvalidPermissions = 0,
	RoleAlreadyExists = 1,
	RoleInactive = 2,
}

#[discriminator]
pub enum RegistryInstruction {
	Initialize = 0,
	AddRole = 1,
	UpdateRole = 2,
	DeactivateRole = 3,
	RotateAdmin = 4,
}

#[discriminator]
pub enum RegistryAccountType {
	RegistryConfig = 1,
	RoleEntry = 2,
}

#[account(discriminator = RegistryAccountType)]
#[pda(seeds = [SEED_REGISTRY_PREFIX, admin: Address], bump = bump)]
pub struct RegistryConfig {
	pub admin: Address,
	pub role_count: u64,
	pub bump: u8,
}

#[account(discriminator = RegistryAccountType)]
#[pda(seeds = [SEED_ROLE_ENTRY_PREFIX, registry: Address, role_id: u64], bump = bump)]
pub struct RoleEntry {
	pub registry: Address,
	pub role_id: u64,
	pub grantee: Address,
	pub permissions: u64,
	pub active: bool,
	pub bump: u8,
}

#[instruction(discriminator = RegistryInstruction::Initialize)]
pub struct InitializeInstruction {
	pub bump: u8,
}

#[instruction(discriminator = RegistryInstruction::AddRole)]
pub struct AddRoleInstruction {
	pub role_id: u64,
	pub permissions: u64,
	pub bump: u8,
}

#[instruction(discriminator = RegistryInstruction::UpdateRole)]
pub struct UpdateRoleInstruction {
	pub permissions: u64,
}

#[instruction(discriminator = RegistryInstruction::DeactivateRole)]
pub struct DeactivateRoleInstruction {}

#[instruction(discriminator = RegistryInstruction::RotateAdmin)]
pub struct RotateAdminInstruction {}

#[derive(Accounts, Debug)]
pub struct InitializeAccounts<'a> {
	pub admin: &'a mut AccountView,
	pub registry_config: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct AddRoleAccounts<'a> {
	pub admin: &'a mut AccountView,
	pub grantee: &'a AccountView,
	pub registry_config: &'a mut AccountView,
	pub role_entry: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct UpdateRoleAccounts<'a> {
	pub admin: &'a AccountView,
	pub registry_config: &'a AccountView,
	pub role_entry: &'a mut AccountView,
}

#[derive(Accounts, Debug)]
pub struct DeactivateRoleAccounts<'a> {
	pub admin: &'a AccountView,
	pub registry_config: &'a AccountView,
	pub role_entry: &'a mut AccountView,
}

#[derive(Accounts, Debug)]
pub struct RotateAdminAccounts<'a> {
	pub admin: &'a AccountView,
	pub new_admin: &'a AccountView,
	pub registry_config: &'a mut AccountView,
}

/// Seed prefix for registry config PDAs.
const SEED_REGISTRY_PREFIX: &[u8] = b"registry";

/// Seed prefix for role entry PDAs.
const SEED_ROLE_ENTRY_PREFIX: &[u8] = b"role-entry";

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = InitializeInstruction::try_from_bytes(data)?;
		let admin_address = *self.admin.address();
		let registry_seeds = RegistryConfig::seeds(&admin_address);

		self.admin.assert_signer()?;
		self.system_program.assert_address(&system::ID)?;
		self.registry_config.assert_empty()?;

		CreateProgramAccountWithBump {
			account: self.registry_config,
			payer: self.admin,
			owner: &ID,
			seeds: &registry_seeds.as_slices(),
			bump: args.bump,
		}
		.invoke_with::<RegistryConfig>(|registry_config| {
			registry_config.admin = admin_address;
			registry_config.role_count.set(0);
			registry_config.bump = args.bump;

			Ok(())
		})?;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for AddRoleAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = AddRoleInstruction::try_from_bytes(data)?;
		let registry_address = *self.registry_config.address();
		let role_entry_seeds = RoleEntry::seeds(&registry_address, args.role_id.get());

		self.admin.assert_signer()?;
		self.system_program.assert_address(&system::ID)?;
		self.registry_config.assert_not_empty()?;
		self.role_entry.assert_empty()?;

		let role_count = {
			let registry_config = self.registry_config.as_account::<RegistryConfig>(&ID)?;
			self.admin.assert_address(&registry_config.admin)?;

			registry_config
				.role_count
				.get()
				.checked_add(1)
				.ok_or(ProgramError::ArithmeticOverflow)?
		};
		let registry_address = *self.registry_config.address();
		let grantee_address = *self.grantee.address();

		CreateProgramAccountWithBump {
			account: self.role_entry,
			payer: self.admin,
			owner: &ID,
			seeds: &role_entry_seeds.as_slices(),
			bump: args.bump,
		}
		.invoke_with::<RoleEntry>(|role_entry| {
			role_entry.registry = registry_address;
			role_entry.role_id = args.role_id;
			role_entry.grantee = grantee_address;
			role_entry.permissions = args.permissions;
			role_entry.active.set(true);
			role_entry.bump = args.bump;

			Ok(())
		})?;

		let mut registry_config = self.registry_config.as_account_mut::<RegistryConfig>(&ID)?;
		registry_config.role_count.set(role_count);

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for UpdateRoleAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = UpdateRoleInstruction::try_from_bytes(data)?;

		self.admin.assert_signer()?;
		self.registry_config.assert_not_empty()?;
		self.role_entry.assert_not_empty()?;

		{
			let registry_config = self.registry_config.as_account::<RegistryConfig>(&ID)?;
			let role_entry = self.role_entry.as_account::<RoleEntry>(&ID)?;

			self.admin.assert_address(&registry_config.admin)?;

			if !role_entry.active.get() {
				return Err(RegistryError::RoleInactive.into());
			}

			if role_entry.registry != *self.registry_config.address() {
				return Err(RegistryError::InvalidPermissions.into());
			}
		}

		let mut role_entry = self.role_entry.as_account_mut::<RoleEntry>(&ID)?;
		role_entry.permissions = args.permissions;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for DeactivateRoleAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		self.admin.assert_signer()?;
		self.registry_config.assert_not_empty()?;
		self.role_entry.assert_not_empty()?;

		{
			let registry_config = self.registry_config.as_account::<RegistryConfig>(&ID)?;
			let role_entry = self.role_entry.as_account::<RoleEntry>(&ID)?;

			self.admin.assert_address(&registry_config.admin)?;

			if role_entry.registry != *self.registry_config.address() {
				return Err(RegistryError::InvalidPermissions.into());
			}

			if !role_entry.active.get() {
				return Err(RegistryError::RoleInactive.into());
			}
		}

		let mut role_entry = self.role_entry.as_account_mut::<RoleEntry>(&ID)?;
		role_entry.active.set(false);

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for RotateAdminAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		self.admin.assert_signer()?;
		self.registry_config.assert_not_empty()?;

		{
			let registry_config = self.registry_config.as_account::<RegistryConfig>(&ID)?;
			self.admin.assert_address(&registry_config.admin)?;
		}

		let mut registry_config = self.registry_config.as_account_mut::<RegistryConfig>(&ID)?;
		registry_config.admin = *self.new_admin.address();

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn discriminator_values() {
		assert_eq!(RegistryInstruction::Initialize as u8, 0);
		assert_eq!(RegistryInstruction::AddRole as u8, 1);
		assert_eq!(RegistryInstruction::UpdateRole as u8, 2);
		assert_eq!(RegistryInstruction::DeactivateRole as u8, 3);
		assert_eq!(RegistryInstruction::RotateAdmin as u8, 4);
	}

	#[test]
	fn instruction_roundtrip() {
		let mut bytes = [0u8; AddRoleInstruction::SIZE];
		AddRoleInstruction::initialize(&mut bytes, |instruction| {
			instruction.role_id.set(7);
			instruction.permissions.set(3);
			instruction.bump = 2;
			Ok(())
		})
		.unwrap_or_else(|error| panic!("initialize failed: {error:?}"));
		let parsed = AddRoleInstruction::try_from_bytes(&bytes)
			.unwrap_or_else(|e| panic!("decode failed: {e:?}"));
		assert_eq!(parsed.role_id.get(), 7);
		assert_eq!(parsed.permissions.get(), 3);
		assert_eq!(parsed.bump, 2);
	}

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let wrong_program_id: Address = [7u8; 32].into();
		let data = [RegistryInstruction::Initialize as u8];
		let result = parse_instruction::<RegistryInstruction>(&wrong_program_id, &ID, &data);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}
}
