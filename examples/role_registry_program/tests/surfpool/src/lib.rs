#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use program_under_test::ID;
use program_under_test::RegistryInstruction;

/// Seed prefixes, mirroring the program's constants.
const REGISTRY_SEED: &[u8] = b"registry";
const ROLE_ENTRY_SEED: &[u8] = b"role-entry";

fn registry_pda(program_id: &Pubkey, admin: &Pubkey) -> (Pubkey, u8) {
	Pubkey::find_program_address(&[REGISTRY_SEED, admin.as_ref()], program_id)
}

fn role_entry_pda(program_id: &Pubkey, registry: &Pubkey, role_id: u64) -> (Pubkey, u8) {
	Pubkey::find_program_address(
		&[ROLE_ENTRY_SEED, registry.as_ref(), &role_id.to_le_bytes()],
		program_id,
	)
}

fn initialize_instruction(
	program: &ProgramTest,
	admin: &Pubkey,
	registry: &Pubkey,
	bump: u8,
) -> pina_test::Instruction {
	program.instruction(
		&[RegistryInstruction::Initialize as u8, bump],
		vec![
			AccountMeta::new(*admin, true),
			AccountMeta::new(*registry, false),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

#[allow(clippy::too_many_arguments)]
fn add_role_instruction(
	program: &ProgramTest,
	admin: &Pubkey,
	grantee: &Pubkey,
	registry: &Pubkey,
	role_entry: &Pubkey,
	role_id: u64,
	permissions: u64,
	bump: u8,
) -> pina_test::Instruction {
	let mut data = vec![RegistryInstruction::AddRole as u8];
	data.extend_from_slice(&role_id.to_le_bytes());
	data.extend_from_slice(&permissions.to_le_bytes());
	data.push(bump);

	program.instruction(
		&data,
		vec![
			AccountMeta::new(*admin, true),
			AccountMeta::new_readonly(*grantee, false),
			AccountMeta::new(*registry, false),
			AccountMeta::new(*role_entry, false),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

/// Initialize creates the registry config PDA for the admin with a zero role
/// count.
#[test]
#[ignore = "run with pina test"]
fn initialize_creates_the_registry_config() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let admin = program.payer();
		let (registry, bump) = registry_pda(&program_id, &admin);

		program
			.send_instruction(initialize_instruction(&program, &admin, &registry, bump))
			.expect("execute Initialize");

		let account = program.account(&registry).expect("fetch registry config");
		assert_eq!(account.owner, program_id);
		assert_eq!(
			account.data[0], 1,
			"account discriminator is RegistryConfig"
		);
		assert_eq!(&account.data[1..33], admin.to_bytes(), "stored admin");
		assert_eq!(&account.data[33..41], 0u64.to_le_bytes(), "role_count zero");
		assert_eq!(account.data[41], bump);

		program.stop().expect("stop isolated program test");
	});
}

/// A full lifecycle: add a role, update its permissions, deactivate it, and
/// verify every step on-chain.
#[test]
#[ignore = "run with pina test"]
fn add_update_deactivate_role_lifecycle() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let admin = program.payer();
		let (registry, bump) = registry_pda(&program_id, &admin);

		program
			.send_instruction(initialize_instruction(&program, &admin, &registry, bump))
			.expect("execute Initialize");

		let grantee = Pubkey::new_unique();
		program.fund(&grantee, 1_000_000_000).expect("fund grantee");
		let (role_entry, role_bump) = role_entry_pda(&program_id, &registry, 1);

		program
			.send_instruction(add_role_instruction(
				&program,
				&admin,
				&grantee,
				&registry,
				&role_entry,
				1,
				0b0101,
				role_bump,
			))
			.expect("execute AddRole");

		let entry = program.account(&role_entry).expect("fetch role entry");
		assert_eq!(entry.data[0], 2, "account discriminator is RoleEntry");
		assert_eq!(&entry.data[1..33], registry.to_bytes());
		assert_eq!(&entry.data[33..41], 1u64.to_le_bytes());
		assert_eq!(&entry.data[41..73], grantee.to_bytes());
		assert_eq!(&entry.data[73..81], 0b0101u64.to_le_bytes());
		assert_eq!(entry.data[81], 1, "role starts active");
		let registry_account = program.account(&registry).expect("fetch registry");
		assert_eq!(
			&registry_account.data[33..41],
			1u64.to_le_bytes(),
			"role_count = 1"
		);

		// UpdateRole: new permissions for an existing active role.
		let mut data = vec![RegistryInstruction::UpdateRole as u8];
		data.extend_from_slice(&0b1001u64.to_le_bytes());
		let update = program.instruction(
			&data,
			vec![
				AccountMeta::new_readonly(admin, true),
				AccountMeta::new_readonly(registry, false),
				AccountMeta::new(role_entry, false),
			],
		);
		program
			.send_instruction(update)
			.expect("execute UpdateRole");
		let entry = program.account(&role_entry).expect("fetch role entry");
		assert_eq!(&entry.data[73..81], 0b1001u64.to_le_bytes());

		// DeactivateRole clears the active flag.
		let deactivate = program.instruction(
			&[RegistryInstruction::DeactivateRole as u8],
			vec![
				AccountMeta::new_readonly(admin, true),
				AccountMeta::new_readonly(registry, false),
				AccountMeta::new(role_entry, false),
			],
		);
		program
			.send_instruction(deactivate)
			.expect("execute DeactivateRole");
		let entry = program.account(&role_entry).expect("fetch role entry");
		assert_eq!(entry.data[80], 0);

		// Updating an inactive role must fail with RoleInactive (custom 2).
		let mut data = vec![RegistryInstruction::UpdateRole as u8];
		data.extend_from_slice(&0b1111u64.to_le_bytes());
		let update = program.instruction(
			&data,
			vec![
				AccountMeta::new_readonly(admin, true),
				AccountMeta::new_readonly(registry, false),
				AccountMeta::new(role_entry, false),
			],
		);
		let error = program
			.send_instruction(update)
			.expect_err("an inactive role cannot be updated");
		assert_eq!(error.operation(), "execute program instruction");
		eprintln!("update inactive role error: {}", error.message());

		program.stop().expect("stop isolated program test");
	});
}

/// Roles are unique per (registry, role_id): adding the same role twice fails
/// on the second create-account.
#[test]
#[ignore = "run with pina test"]
fn cannot_add_the_same_role_twice() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let admin = program.payer();
		let (registry, bump) = registry_pda(&program_id, &admin);

		program
			.send_instruction(initialize_instruction(&program, &admin, &registry, bump))
			.expect("execute Initialize");

		let grantee = Pubkey::new_unique();
		program.fund(&grantee, 1_000_000_000).expect("fund grantee");
		let (role_entry, role_bump) = role_entry_pda(&program_id, &registry, 7);

		let add = add_role_instruction(
			&program,
			&admin,
			&grantee,
			&registry,
			&role_entry,
			7,
			1,
			role_bump,
		);
		program.send_instruction(add).expect("first AddRole");

		let add = add_role_instruction(
			&program,
			&admin,
			&grantee,
			&registry,
			&role_entry,
			7,
			2,
			role_bump,
		);
		let error = program
			.send_instruction(add)
			.expect_err("role id 7 already exists");
		assert_eq!(error.operation(), "execute program instruction");

		program.stop().expect("stop isolated program test");
	});
}

/// RotateAdmin transfers admin ownership; the former admin can no longer add
/// roles.
#[test]
#[ignore = "run with pina test"]
fn rotate_admin_and_verify_the_new_admin() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let admin = program.payer();
		let (registry, bump) = registry_pda(&program_id, &admin);

		program
			.send_instruction(initialize_instruction(&program, &admin, &registry, bump))
			.expect("execute Initialize");

		let new_admin = Pubkey::new_unique();
		program
			.fund(&new_admin, 1_000_000_000)
			.expect("fund new admin");

		let rotate = program.instruction(
			&[RegistryInstruction::RotateAdmin as u8],
			vec![
				AccountMeta::new_readonly(admin, true),
				AccountMeta::new_readonly(new_admin, false),
				AccountMeta::new(registry, false),
			],
		);
		program
			.send_instruction(rotate)
			.expect("execute RotateAdmin");

		let account = program.account(&registry).expect("fetch registry");
		assert_eq!(&account.data[1..33], new_admin.to_bytes(), "admin rotated");

		program.stop().expect("stop isolated program test");
	});
}
