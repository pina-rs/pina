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
		accounts: &[AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: RegistryInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			RegistryInstruction::Initialize => {
				InitializeAccounts::try_from(accounts)?.process(data)
			}

			RegistryInstruction::AddRole => AddRoleAccounts::try_from(accounts)?.process(data),

			RegistryInstruction::UpdateRole => {
				UpdateRoleAccounts::try_from(accounts)?.process(data)
			}

			RegistryInstruction::DeactivateRole => {
				DeactivateRoleAccounts::try_from(accounts)?.process(data)
			}

			RegistryInstruction::RotateAdmin => {
				RotateAdminAccounts::try_from(accounts)?.process(data)
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
pub struct RegistryConfig {
	pub admin: Address,
	pub role_count: PodU64,
	pub bump: u8,
}

#[account(discriminator = RegistryAccountType)]
pub struct RoleEntry {
	pub registry: Address,
	pub role_id: PodU64,
	pub grantee: Address,
	pub permissions: PodU64,
	pub active: PodBool,
	pub bump: u8,
}

#[instruction(discriminator = RegistryInstruction, variant = Initialize)]
pub struct InitializeInstruction {
	pub bump: u8,
}

#[instruction(discriminator = RegistryInstruction, variant = AddRole)]
pub struct AddRoleInstruction {
	pub role_id: PodU64,
	pub permissions: PodU64,
	pub bump: u8,
}

#[instruction(discriminator = RegistryInstruction, variant = UpdateRole)]
pub struct UpdateRoleInstruction {
	pub permissions: PodU64,
}

#[instruction(discriminator = RegistryInstruction, variant = DeactivateRole)]
pub struct DeactivateRoleInstruction {}

#[instruction(discriminator = RegistryInstruction, variant = RotateAdmin)]
pub struct RotateAdminInstruction {}

#[derive(Accounts, Debug)]
pub struct InitializeAccounts<'a> {
	pub admin: &'a AccountView,
	pub registry_config: &'a AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct AddRoleAccounts<'a> {
	pub admin: &'a AccountView,
	pub grantee: &'a AccountView,
	pub registry_config: &'a AccountView,
	pub role_entry: &'a AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct UpdateRoleAccounts<'a> {
	pub admin: &'a AccountView,
	pub registry_config: &'a AccountView,
	pub role_entry: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct DeactivateRoleAccounts<'a> {
	pub admin: &'a AccountView,
	pub registry_config: &'a AccountView,
	pub role_entry: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct RotateAdminAccounts<'a> {
	pub admin: &'a AccountView,
	pub new_admin: &'a AccountView,
	pub registry_config: &'a AccountView,
}

const REGISTRY_SEED_PREFIX: &[u8] = b"registry";
const ROLE_ENTRY_SEED_PREFIX: &[u8] = b"role-entry";

#[macro_export]
macro_rules! registry_config_seeds {
	($admin:expr) => {
		&[REGISTRY_SEED_PREFIX, $admin]
	};
	($admin:expr, $bump:expr) => {
		&[REGISTRY_SEED_PREFIX, $admin, &[$bump]]
	};
}

#[macro_export]
macro_rules! role_entry_seeds {
	($registry:expr, $role_id:expr) => {
		&[ROLE_ENTRY_SEED_PREFIX, $registry, $role_id]
	};
	($registry:expr, $role_id:expr, $bump:expr) => {
		&[ROLE_ENTRY_SEED_PREFIX, $registry, $role_id, &[$bump]]
	};
}

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = InitializeInstruction::try_from_bytes(data)?;
		let registry_seeds = registry_config_seeds!(self.admin.address().as_ref());
		let registry_seeds_with_bump =
			registry_config_seeds!(self.admin.address().as_ref(), args.bump);

		self.admin.assert_signer()?;
		self.system_program.assert_address(&system::ID)?;
		self.registry_config
			.assert_empty()?
			.assert_writable()?
			.assert_seeds_with_bump(registry_seeds_with_bump, &ID)?;

		create_program_account_with_bump::<RegistryConfig>(
			self.registry_config,
			self.admin,
			&ID,
			registry_seeds,
			args.bump,
		)?;

		let registry_config = self.registry_config.as_account_mut::<RegistryConfig>(&ID)?;
		*registry_config = RegistryConfig::builder()
			.admin(*self.admin.address())
			.role_count(PodU64::from_primitive(0))
			.bump(args.bump)
			.build();

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for AddRoleAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = AddRoleInstruction::try_from_bytes(data)?;
		let role_entry_seeds =
			role_entry_seeds!(self.registry_config.address().as_ref(), &args.role_id.0);
		let role_entry_seeds_with_bump = role_entry_seeds!(
			self.registry_config.address().as_ref(),
			&args.role_id.0,
			args.bump
		);

		self.admin.assert_signer()?;
		self.system_program.assert_address(&system::ID)?;
		self.registry_config
			.assert_not_empty()?
			.assert_writable()?
			.assert_type::<RegistryConfig>(&ID)?;
		self.role_entry
			.assert_empty()?
			.assert_writable()?
			.assert_seeds_with_bump(role_entry_seeds_with_bump, &ID)?;

		let registry_config = self.registry_config.as_account::<RegistryConfig>(&ID)?;
		self.admin.assert_address(&registry_config.admin)?;

		create_program_account_with_bump::<RoleEntry>(
			self.role_entry,
			self.admin,
			&ID,
			role_entry_seeds,
			args.bump,
		)?;

		let role_count = u64::from(registry_config.role_count)
			.checked_add(1)
			.ok_or(ProgramError::ArithmeticOverflow)?;

		let role_entry = self.role_entry.as_account_mut::<RoleEntry>(&ID)?;
		*role_entry = RoleEntry::builder()
			.registry(*self.registry_config.address())
			.role_id(args.role_id)
			.grantee(*self.grantee.address())
			.permissions(args.permissions)
			.active(PodBool::from_bool(true))
			.bump(args.bump)
			.build();

		let registry_config = self.registry_config.as_account_mut::<RegistryConfig>(&ID)?;
		registry_config.role_count = PodU64::from_primitive(role_count);

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for UpdateRoleAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = UpdateRoleInstruction::try_from_bytes(data)?;

		self.admin.assert_signer()?;
		self.registry_config
			.assert_not_empty()?
			.assert_writable()?
			.assert_type::<RegistryConfig>(&ID)?;
		self.role_entry
			.assert_not_empty()?
			.assert_writable()?
			.assert_type::<RoleEntry>(&ID)?;

		let registry_config = self.registry_config.as_account::<RegistryConfig>(&ID)?;
		let role_entry = self.role_entry.as_account::<RoleEntry>(&ID)?;

		self.admin.assert_address(&registry_config.admin)?;

		if !bool::from(role_entry.active) {
			return Err(RegistryError::RoleInactive.into());
		}

		if role_entry.registry != *self.registry_config.address() {
			return Err(RegistryError::InvalidPermissions.into());
		}

		let role_entry = self.role_entry.as_account_mut::<RoleEntry>(&ID)?;
		role_entry.permissions = args.permissions;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for DeactivateRoleAccounts<'a> {
	fn process(&self, _data: &[u8]) -> ProgramResult {
		self.admin.assert_signer()?;
		self.registry_config
			.assert_not_empty()?
			.assert_writable()?
			.assert_type::<RegistryConfig>(&ID)?;
		self.role_entry
			.assert_not_empty()?
			.assert_writable()?
			.assert_type::<RoleEntry>(&ID)?;

		let registry_config = self.registry_config.as_account::<RegistryConfig>(&ID)?;
		let role_entry = self.role_entry.as_account::<RoleEntry>(&ID)?;

		self.admin.assert_address(&registry_config.admin)?;

		if role_entry.registry != *self.registry_config.address() {
			return Err(RegistryError::InvalidPermissions.into());
		}

		if !bool::from(role_entry.active) {
			return Err(RegistryError::RoleInactive.into());
		}

		let role_entry = self.role_entry.as_account_mut::<RoleEntry>(&ID)?;
		role_entry.active = PodBool::from_bool(false);

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for RotateAdminAccounts<'a> {
	fn process(&self, _data: &[u8]) -> ProgramResult {
		self.admin.assert_signer()?;
		self.registry_config
			.assert_not_empty()?
			.assert_writable()?
			.assert_type::<RegistryConfig>(&ID)?;

		let registry_config = self.registry_config.as_account::<RegistryConfig>(&ID)?;

		self.admin.assert_address(&registry_config.admin)?;

		let registry_config = self.registry_config.as_account_mut::<RegistryConfig>(&ID)?;
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
		let ix = AddRoleInstruction::builder()
			.role_id(PodU64::from_primitive(7))
			.permissions(PodU64::from_primitive(3))
			.bump(2)
			.build();
		let bytes = ix.to_bytes();
		let parsed = AddRoleInstruction::try_from_bytes(bytes)
			.unwrap_or_else(|e| panic!("decode failed: {e:?}"));
		assert_eq!(u64::from(parsed.role_id), 7);
		assert_eq!(u64::from(parsed.permissions), 3);
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
