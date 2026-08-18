//! End-to-end tests for the role_registry_program.
//!
//! These tests exercise the full instruction flow through `mollusk-svm`:
//! Initialize, AddRole, UpdateRole, DeactivateRole, and RotateAdmin. Because
//! the program only CPIs to the system program (no token CPIs), the entire
//! lifecycle — including account creation — can be tested end-to-end.
//!
//! ## Prerequisites
//!
//! The role_registry_program must be compiled to an SBF binary before running
//! these tests:
//!
//! ```sh
//! cargo build --release --target bpfel-unknown-none -p role_registry_program \
//!     -Z build-std -F bpf-entrypoint
//! ```
//!
//! Then set `SBF_OUT_DIR` to the directory containing the `.so` file, or place
//! it in `tests/fixtures/`.
//!
//! ## Running
//!
//! ```sh
//! SBF_OUT_DIR=target/bpfel-unknown-none/release \
//!     cargo test -p role_registry_program --test e2e -- --nocapture
//! ```

use std::mem::size_of;

use mollusk_svm::Mollusk;
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::result::Check;
use pina::PodBool;
use pina::PodU64;
use pina::ProgramError;
use pina::bytemuck;
use role_registry_program::AddRoleInstruction;
use role_registry_program::DeactivateRoleInstruction;
use role_registry_program::ID;
use role_registry_program::InitializeInstruction;
use role_registry_program::RegistryConfig;
use role_registry_program::RegistryError;
use role_registry_program::RoleEntry;
use role_registry_program::RotateAdminInstruction;
use role_registry_program::UpdateRoleInstruction;
use solana_account::Account;
use solana_instruction::AccountMeta;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn program_id() -> Pubkey {
	let id = ID;
	let bytes: &[u8] = id.as_ref();
	let array: [u8; 32] = bytes
		.try_into()
		.unwrap_or_else(|_| panic!("address must be 32 bytes"));
	Pubkey::new_from_array(array)
}

/// Try to create a mollusk instance for the role_registry_program.
///
/// Returns `None` if the SBF binary cannot be found (e.g. the program has not
/// been compiled yet). This allows tests to be skipped gracefully without
/// triggering a panic-abort from the `no_std` panic handler.
fn try_create_mollusk() -> Option<Mollusk> {
	let so_name = "role_registry_program.so";
	let search_dirs: Vec<std::path::PathBuf> = [
		std::env::var("SBF_OUT_DIR").ok(),
		std::env::var("BPF_OUT_DIR").ok(),
		Some("tests/fixtures".to_owned()),
	]
	.into_iter()
	.flatten()
	.map(std::path::PathBuf::from)
	.collect();

	let found = search_dirs.iter().any(|dir| dir.join(so_name).is_file());
	if !found {
		return None;
	}

	Some(Mollusk::new(&program_id(), "role_registry_program"))
}

/// Derive the registry config PDA for a given admin pubkey.
fn derive_registry_pda(admin: &Pubkey) -> (Pubkey, u8) {
	Pubkey::find_program_address(&[b"registry", admin.as_ref()], &program_id())
}

/// Derive the role entry PDA for a given registry address and role_id.
///
/// The seed uses the LE bytes of `role_id`, matching `&args.role_id.0` in the
/// program (which stores `PodU64` in little-endian order).
fn derive_role_entry_pda(registry: &Pubkey, role_id: u64) -> (Pubkey, u8) {
	let role_id_bytes = role_id.to_le_bytes();
	Pubkey::find_program_address(
		&[b"role-entry", registry.as_ref(), &role_id_bytes],
		&program_id(),
	)
}

fn pubkey_to_address(pk: &Pubkey) -> pina::Address {
	let bytes: [u8; 32] = pk.to_bytes();
	bytes.into()
}

// ---------------------------------------------------------------------------
// Instruction data builders
// ---------------------------------------------------------------------------

fn initialize_ix_data(bump: u8) -> Vec<u8> {
	let ix = InitializeInstruction::builder().bump(bump).build();
	ix.to_bytes().to_vec()
}

fn add_role_ix_data(role_id: u64, permissions: u64, bump: u8) -> Vec<u8> {
	let ix = AddRoleInstruction::builder()
		.role_id(PodU64::from_primitive(role_id))
		.permissions(PodU64::from_primitive(permissions))
		.bump(bump)
		.build();
	ix.to_bytes().to_vec()
}

fn update_role_ix_data(permissions: u64) -> Vec<u8> {
	let ix = UpdateRoleInstruction::builder()
		.permissions(PodU64::from_primitive(permissions))
		.build();
	ix.to_bytes().to_vec()
}

fn deactivate_role_ix_data() -> Vec<u8> {
	let ix = DeactivateRoleInstruction::builder().build();
	ix.to_bytes().to_vec()
}

fn rotate_admin_ix_data() -> Vec<u8> {
	let ix = RotateAdminInstruction::builder().build();
	ix.to_bytes().to_vec()
}

// ---------------------------------------------------------------------------
// Account state builders
// ---------------------------------------------------------------------------

/// Build a pre-populated `RegistryConfig` account for testing instructions
/// that don't need to run Initialize first.
fn registry_config_account(admin: &Pubkey, role_count: u64, bump: u8, lamports: u64) -> Account {
	let state = RegistryConfig::builder()
		.admin(pubkey_to_address(admin))
		.role_count(PodU64::from_primitive(role_count))
		.bump(bump)
		.build();
	let data = bytemuck::bytes_of(&state).to_vec();
	Account {
		lamports,
		data,
		owner: program_id(),
		executable: false,
		rent_epoch: 0,
	}
}

/// Build a pre-populated `RoleEntry` account for testing instructions
/// that don't need to run AddRole first.
fn role_entry_account(
	registry: &Pubkey,
	role_id: u64,
	grantee: &Pubkey,
	permissions: u64,
	active: bool,
	bump: u8,
	lamports: u64,
) -> Account {
	let state = RoleEntry::builder()
		.registry(pubkey_to_address(registry))
		.role_id(PodU64::from_primitive(role_id))
		.grantee(pubkey_to_address(grantee))
		.permissions(PodU64::from_primitive(permissions))
		.active(PodBool::from_bool(active))
		.bump(bump)
		.build();
	let data = bytemuck::bytes_of(&state).to_vec();
	Account {
		lamports,
		data,
		owner: program_id(),
		executable: false,
		rent_epoch: 0,
	}
}

const SKIP_MSG: &str = "[SKIP] role_registry_program SBF binary not found. Build it first with \
                        `cargo build --release --target bpfel-unknown-none -p \
                        role_registry_program -Z build-std -F bpf-entrypoint`.";

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Initialize a fresh registry and verify the resulting `RegistryConfig`
/// state: admin address stored, role_count = 0, bump correct.
#[test]
fn initialize_creates_registry_config() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let admin = Pubkey::new_unique();
	let (registry_pda, bump) = derive_registry_pda(&admin);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&initialize_ix_data(bump),
		vec![
			AccountMeta::new(admin, true),
			AccountMeta::new(registry_pda, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let accounts = vec![
		(
			admin,
			Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
		),
		(registry_pda, Account::default()),
		keyed_account_for_system_program(),
	];

	let result =
		mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);

	let registry_account = result
		.get_account(&registry_pda)
		.expect("registry_config PDA should exist after Initialize");

	let registry_config: &RegistryConfig = bytemuck::from_bytes(&registry_account.data);
	assert_eq!(
		registry_config.admin,
		pubkey_to_address(&admin),
		"admin should be stored in RegistryConfig"
	);
	assert_eq!(
		u64::from(registry_config.role_count),
		0,
		"role_count should start at 0"
	);
	assert_eq!(
		registry_config.bump, bump,
		"bump should be stored correctly"
	);

	eprintln!(
		"[CU] Initialize registry: {} compute units consumed",
		result.compute_units_consumed
	);
}

/// Full lifecycle: Initialize → AddRole → UpdateRole → DeactivateRole.
///
/// Each step feeds its resulting accounts into the next instruction, verifying
/// the state is correct at every point.
#[test]
fn full_flow_initialize_add_update_deactivate() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let admin = Pubkey::new_unique();
	let grantee = Pubkey::new_unique();
	let (registry_pda, registry_bump) = derive_registry_pda(&admin);
	let role_id: u64 = 42;
	let (role_entry_pda, role_entry_bump) = derive_role_entry_pda(&registry_pda, role_id);

	// ----- Step 1: Initialize -----

	let init_ix = Instruction::new_with_bytes(
		program_id(),
		&initialize_ix_data(registry_bump),
		vec![
			AccountMeta::new(admin, true),
			AccountMeta::new(registry_pda, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let admin_account = Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id());

	let init_result = mollusk.process_and_validate_instruction(
		&init_ix,
		&[
			(admin, admin_account.clone()),
			(registry_pda, Account::default()),
			keyed_account_for_system_program(),
		],
		&[Check::success()],
	);

	eprintln!(
		"[CU] Full flow - Initialize: {} CU",
		init_result.compute_units_consumed
	);

	let admin_after_init = init_result
		.get_account(&admin)
		.cloned()
		.unwrap_or_else(|| panic!("admin not found after Initialize"));
	let registry_after_init = init_result
		.get_account(&registry_pda)
		.cloned()
		.unwrap_or_else(|| panic!("registry_pda not found after Initialize"));

	// Verify role_count == 0 after Initialize.
	let registry_config: &RegistryConfig = bytemuck::from_bytes(&registry_after_init.data);
	assert_eq!(u64::from(registry_config.role_count), 0);

	// ----- Step 2: AddRole -----

	let add_role_ix = Instruction::new_with_bytes(
		program_id(),
		&add_role_ix_data(role_id, 0b0000_0111, role_entry_bump),
		vec![
			AccountMeta::new(admin, true),
			AccountMeta::new_readonly(grantee, false),
			AccountMeta::new(registry_pda, false),
			AccountMeta::new(role_entry_pda, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let grantee_account = Account::new(0, 0, &solana_sdk_ids::system_program::id());

	let add_result = mollusk.process_and_validate_instruction(
		&add_role_ix,
		&[
			(admin, admin_after_init),
			(grantee, grantee_account),
			(registry_pda, registry_after_init),
			(role_entry_pda, Account::default()),
			keyed_account_for_system_program(),
		],
		&[Check::success()],
	);

	eprintln!(
		"[CU] Full flow - AddRole: {} CU",
		add_result.compute_units_consumed
	);

	let registry_after_add = add_result
		.get_account(&registry_pda)
		.cloned()
		.unwrap_or_else(|| panic!("registry_pda not found after AddRole"));
	let role_entry_after_add = add_result
		.get_account(&role_entry_pda)
		.cloned()
		.unwrap_or_else(|| panic!("role_entry_pda not found after AddRole"));
	let admin_after_add = add_result
		.get_account(&admin)
		.cloned()
		.unwrap_or_else(|| panic!("admin not found after AddRole"));

	// Verify role_count incremented to 1 and role entry is active.
	let registry_config: &RegistryConfig = bytemuck::from_bytes(&registry_after_add.data);
	assert_eq!(
		u64::from(registry_config.role_count),
		1,
		"role_count should be 1 after AddRole"
	);

	let role_entry: &RoleEntry = bytemuck::from_bytes(&role_entry_after_add.data);
	assert!(
		bool::from(role_entry.active),
		"role should be active after AddRole"
	);
	assert_eq!(u64::from(role_entry.permissions), 0b0000_0111);
	assert_eq!(u64::from(role_entry.role_id), role_id);

	// ----- Step 3: UpdateRole -----

	let update_role_ix = Instruction::new_with_bytes(
		program_id(),
		&update_role_ix_data(0b1111_1111),
		vec![
			AccountMeta::new_readonly(admin, true),
			AccountMeta::new(registry_pda, false),
			AccountMeta::new(role_entry_pda, false),
		],
	);

	let update_result = mollusk.process_and_validate_instruction(
		&update_role_ix,
		&[
			(admin, admin_after_add),
			(registry_pda, registry_after_add),
			(role_entry_pda, role_entry_after_add),
		],
		&[Check::success()],
	);

	eprintln!(
		"[CU] Full flow - UpdateRole: {} CU",
		update_result.compute_units_consumed
	);

	let registry_after_update = update_result
		.get_account(&registry_pda)
		.cloned()
		.unwrap_or_else(|| panic!("registry_pda not found after UpdateRole"));
	let role_entry_after_update = update_result
		.get_account(&role_entry_pda)
		.cloned()
		.unwrap_or_else(|| panic!("role_entry_pda not found after UpdateRole"));
	let admin_after_update = update_result
		.get_account(&admin)
		.cloned()
		.unwrap_or_else(|| panic!("admin not found after UpdateRole"));

	// Verify permissions were updated.
	let role_entry: &RoleEntry = bytemuck::from_bytes(&role_entry_after_update.data);
	assert_eq!(
		u64::from(role_entry.permissions),
		0b1111_1111,
		"permissions should be updated after UpdateRole"
	);
	assert!(
		bool::from(role_entry.active),
		"role should still be active after UpdateRole"
	);

	// ----- Step 4: DeactivateRole -----

	let deactivate_ix = Instruction::new_with_bytes(
		program_id(),
		&deactivate_role_ix_data(),
		vec![
			AccountMeta::new_readonly(admin, true),
			AccountMeta::new(registry_pda, false),
			AccountMeta::new(role_entry_pda, false),
		],
	);

	let deactivate_result = mollusk.process_and_validate_instruction(
		&deactivate_ix,
		&[
			(admin, admin_after_update),
			(registry_pda, registry_after_update),
			(role_entry_pda, role_entry_after_update),
		],
		&[Check::success()],
	);

	eprintln!(
		"[CU] Full flow - DeactivateRole: {} CU",
		deactivate_result.compute_units_consumed
	);

	let role_entry_after_deactivate = deactivate_result
		.get_account(&role_entry_pda)
		.expect("role_entry_pda should exist after DeactivateRole");

	let role_entry: &RoleEntry = bytemuck::from_bytes(&role_entry_after_deactivate.data);
	assert!(
		!bool::from(role_entry.active),
		"role should be inactive after DeactivateRole"
	);

	eprintln!(
		"[CU] Full flow - Total: {} CU",
		init_result.compute_units_consumed
			+ add_result.compute_units_consumed
			+ update_result.compute_units_consumed
			+ deactivate_result.compute_units_consumed
	);
}

/// Initialize a registry and then rotate the admin. Verify that the new
/// admin address is stored in the `RegistryConfig`.
#[test]
fn rotate_admin_changes_admin() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let admin = Pubkey::new_unique();
	let new_admin = Pubkey::new_unique();
	let (registry_pda, registry_bump) = derive_registry_pda(&admin);

	// Initialize.
	let init_ix = Instruction::new_with_bytes(
		program_id(),
		&initialize_ix_data(registry_bump),
		vec![
			AccountMeta::new(admin, true),
			AccountMeta::new(registry_pda, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	let init_result = mollusk.process_and_validate_instruction(
		&init_ix,
		&[
			(
				admin,
				Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
			),
			(registry_pda, Account::default()),
			keyed_account_for_system_program(),
		],
		&[Check::success()],
	);

	eprintln!(
		"[CU] RotateAdmin - Initialize: {} CU",
		init_result.compute_units_consumed
	);

	let admin_after_init = init_result
		.get_account(&admin)
		.cloned()
		.unwrap_or_else(|| panic!("admin not found after Initialize"));
	let registry_after_init = init_result
		.get_account(&registry_pda)
		.cloned()
		.unwrap_or_else(|| panic!("registry_pda not found after Initialize"));

	// RotateAdmin.
	let rotate_ix = Instruction::new_with_bytes(
		program_id(),
		&rotate_admin_ix_data(),
		vec![
			AccountMeta::new_readonly(admin, true),
			AccountMeta::new_readonly(new_admin, false),
			AccountMeta::new(registry_pda, false),
		],
	);

	let new_admin_account = Account::new(0, 0, &solana_sdk_ids::system_program::id());

	let rotate_result = mollusk.process_and_validate_instruction(
		&rotate_ix,
		&[
			(admin, admin_after_init),
			(new_admin, new_admin_account),
			(registry_pda, registry_after_init),
		],
		&[Check::success()],
	);

	eprintln!(
		"[CU] RotateAdmin - RotateAdmin: {} CU",
		rotate_result.compute_units_consumed
	);

	let registry_account = rotate_result
		.get_account(&registry_pda)
		.expect("registry_pda should exist after RotateAdmin");

	let registry_config: &RegistryConfig = bytemuck::from_bytes(&registry_account.data);
	assert_eq!(
		registry_config.admin,
		pubkey_to_address(&new_admin),
		"admin should be updated to new_admin after RotateAdmin"
	);
}

/// Pre-populate a `RoleEntry` with `active = false` and attempt to run
/// DeactivateRole. The program should reject it with `RoleInactive`.
#[test]
fn deactivate_inactive_role_fails() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let admin = Pubkey::new_unique();
	let grantee = Pubkey::new_unique();
	let (registry_pda, registry_bump) = derive_registry_pda(&admin);
	let role_id: u64 = 1;
	let (role_entry_pda, role_entry_bump) = derive_role_entry_pda(&registry_pda, role_id);

	let registry_lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<RegistryConfig>());
	let role_entry_lamports = mollusk.sysvars.rent.minimum_balance(size_of::<RoleEntry>());

	// Pre-built accounts: active=false on the RoleEntry.
	let registry_acct = registry_config_account(&admin, 1, registry_bump, registry_lamports);
	let role_entry_acct = role_entry_account(
		&registry_pda,
		role_id,
		&grantee,
		0b0000_0001,
		false, // already inactive
		role_entry_bump,
		role_entry_lamports,
	);

	let instruction = Instruction::new_with_bytes(
		program_id(),
		&deactivate_role_ix_data(),
		vec![
			AccountMeta::new_readonly(admin, true),
			AccountMeta::new(registry_pda, false),
			AccountMeta::new(role_entry_pda, false),
		],
	);

	mollusk.process_and_validate_instruction(
		&instruction,
		&[
			(
				admin,
				Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
			),
			(registry_pda, registry_acct),
			(role_entry_pda, role_entry_acct),
		],
		&[Check::err(RegistryError::RoleInactive.into())],
	);
}

/// Initialize a registry with admin A. Then try to AddRole using admin B as
/// the signer. The program checks that the signing account matches the stored
/// admin, so this should fail with `ProgramError::InvalidAccountData`.
#[test]
fn wrong_admin_cannot_add_role() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	let admin_a = Pubkey::new_unique(); // real admin stored in RegistryConfig
	let admin_b = Pubkey::new_unique(); // impostor signer
	let grantee = Pubkey::new_unique();
	let (registry_pda, registry_bump) = derive_registry_pda(&admin_a);
	let role_id: u64 = 5;
	let (role_entry_pda, role_entry_bump) = derive_role_entry_pda(&registry_pda, role_id);

	let registry_lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<RegistryConfig>());

	// Pre-built RegistryConfig with admin = admin_a.
	let registry_acct = registry_config_account(&admin_a, 0, registry_bump, registry_lamports);

	// admin_b signs the AddRole instruction, but RegistryConfig stores admin_a.
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&add_role_ix_data(role_id, 0b0011, role_entry_bump),
		vec![
			AccountMeta::new(admin_b, true), // wrong signer
			AccountMeta::new_readonly(grantee, false),
			AccountMeta::new(registry_pda, false),
			AccountMeta::new(role_entry_pda, false),
			AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
		],
	);

	mollusk.process_and_validate_instruction(
		&instruction,
		&[
			(
				admin_b,
				Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
			),
			(
				grantee,
				Account::new(0, 0, &solana_sdk_ids::system_program::id()),
			),
			(registry_pda, registry_acct),
			(role_entry_pda, Account::default()),
			keyed_account_for_system_program(),
		],
		&[Check::err(ProgramError::InvalidAccountData)],
	);
}

/// Create a `RoleEntry` whose `registry` field points to registry A, then run
/// UpdateRole with registry B as the `registry_config` account. The program
/// checks `role_entry.registry == registry_config.address()`, so this must
/// fail with `InvalidPermissions`.
#[test]
fn update_role_on_wrong_registry_fails() {
	let Some(mollusk) = try_create_mollusk() else {
		eprintln!("{SKIP_MSG}");
		return;
	};

	// Both registries share the same admin so that the admin address check
	// passes and we reach the registry mismatch check.
	let admin = Pubkey::new_unique();
	let grantee = Pubkey::new_unique();

	// Registry A — the one the RoleEntry actually belongs to.
	let (registry_a_pda, registry_a_bump) = derive_registry_pda(&admin);

	// Registry B — a second, distinct registry also owned by the same admin.
	// We use a different intermediate pubkey to produce a distinct PDA address.
	let admin_b_seed = Pubkey::new_unique();
	let (registry_b_pda, registry_b_bump) = derive_registry_pda(&admin_b_seed);

	let role_id: u64 = 99;
	// The role entry PDA is derived from registry A.
	let (role_entry_pda, role_entry_bump) = derive_role_entry_pda(&registry_a_pda, role_id);

	let registry_lamports = mollusk
		.sysvars
		.rent
		.minimum_balance(size_of::<RegistryConfig>());
	let role_entry_lamports = mollusk.sysvars.rent.minimum_balance(size_of::<RoleEntry>());

	// Registry A account (correct registry for the role entry).
	let _registry_a_acct = registry_config_account(&admin, 1, registry_a_bump, registry_lamports);

	// Registry B account (wrong registry, but same admin so admin check passes).
	let registry_b_acct = registry_config_account(&admin, 0, registry_b_bump, registry_lamports);

	// RoleEntry whose `registry` field points to registry A.
	let role_entry_acct = role_entry_account(
		&registry_a_pda, // registry field = registry A
		role_id,
		&grantee,
		0b0001,
		true,
		role_entry_bump,
		role_entry_lamports,
	);

	// UpdateRole passes registry B — mismatch with role_entry.registry (A).
	let instruction = Instruction::new_with_bytes(
		program_id(),
		&update_role_ix_data(0b1111),
		vec![
			AccountMeta::new_readonly(admin, true),
			AccountMeta::new(registry_b_pda, false), // wrong registry
			AccountMeta::new(role_entry_pda, false),
		],
	);

	mollusk.process_and_validate_instruction(
		&instruction,
		&[
			(
				admin,
				Account::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
			),
			(registry_b_pda, registry_b_acct),
			(role_entry_pda, role_entry_acct),
		],
		&[Check::err(RegistryError::InvalidPermissions.into())],
	);
}
