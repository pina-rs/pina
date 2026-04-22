use client::generated::accounts::REGISTRY_CONFIG_DISCRIMINATOR;
use client::generated::accounts::ROLE_ENTRY_DISCRIMINATOR;
use client::generated::instructions::AddRole;
use client::generated::instructions::AddRoleInstructionData;
use client::generated::instructions::DeactivateRole;
use client::generated::instructions::DeactivateRoleInstructionData;
use client::generated::instructions::Initialize;
use client::generated::instructions::InitializeInstructionData;
use client::generated::instructions::RotateAdmin;
use client::generated::instructions::RotateAdminInstructionData;
use client::generated::instructions::UpdateRole;
use client::generated::instructions::UpdateRoleInstructionData;
use client::generated::instructions::{self};
use pina_pod_primitives::PodU64;
use role_registry_program_client as client;
use solana_instruction::AccountMeta;
use solana_pubkey::Pubkey;
use solana_pubkey::pubkey;

#[test]
fn role_registry_program_client_has_expected_contract_shape() {
	let program_id = client::generated::programs::ROLE_REGISTRY_PROGRAM_ID;
	assert_eq!(
		program_id,
		pubkey!("3B7roNNQLnW43Par9AfTuVzEqZx7yPtXRA9K3Ev7RHyX"),
	);

	assert_eq!(instructions::INITIALIZE_DISCRIMINATOR, 0u8);
	assert_eq!(instructions::ADD_ROLE_DISCRIMINATOR, 1u8);
	assert_eq!(instructions::UPDATE_ROLE_DISCRIMINATOR, 2u8);
	assert_eq!(instructions::DEACTIVATE_ROLE_DISCRIMINATOR, 3u8);
	assert_eq!(instructions::ROTATE_ADMIN_DISCRIMINATOR, 4u8);
	assert_eq!(REGISTRY_CONFIG_DISCRIMINATOR, 1u8);
	assert_eq!(ROLE_ENTRY_DISCRIMINATOR, 2u8);

	let admin = Pubkey::new_unique();
	let grantee = Pubkey::new_unique();
	let new_admin = Pubkey::new_unique();
	let registry_config = Pubkey::new_unique();
	let role_entry = Pubkey::new_unique();

	let initialize = Initialize::new(admin, registry_config);
	let init_payload = InitializeInstructionData::new(7);
	let init_ix = initialize.instruction(init_payload);
	assert_eq!(init_ix.program_id, program_id);
	assert_eq!(init_ix.accounts.len(), 3);
	assert_eq!(init_ix.accounts[0], AccountMeta::new(admin, true),);
	assert_eq!(
		init_ix.accounts[1],
		AccountMeta::new(registry_config, false)
	);
	assert_eq!(
		init_ix.accounts[2],
		AccountMeta::new_readonly(pubkey!("11111111111111111111111111111111"), false,),
	);
	assert_eq!(init_ix.data, bytemuck::bytes_of(&init_payload).to_vec());

	let add_role = AddRole::new(admin, grantee, registry_config, role_entry);
	let add_payload =
		AddRoleInstructionData::new(PodU64::from_primitive(11), PodU64::from_primitive(42), 3);
	let add_ix = add_role.instruction(add_payload);
	assert_eq!(add_ix.accounts.len(), 5);
	assert_eq!(add_ix.accounts[0], AccountMeta::new(admin, true));
	assert_eq!(
		add_ix.accounts[1],
		AccountMeta::new_readonly(grantee, false)
	);
	assert_eq!(add_ix.accounts[2], AccountMeta::new(registry_config, false));
	assert_eq!(add_ix.accounts[3], AccountMeta::new(role_entry, false));
	assert_eq!(
		add_ix.accounts[4],
		AccountMeta::new_readonly(pubkey!("11111111111111111111111111111111"), false,)
	);
	assert_eq!(add_ix.data, bytemuck::bytes_of(&add_payload).to_vec());

	let update = UpdateRole::new(admin, registry_config, role_entry);
	let update_payload = UpdateRoleInstructionData::new(PodU64::from_primitive(99));
	let update_ix = update.instruction(update_payload);
	assert_eq!(update_ix.accounts.len(), 3);
	assert_eq!(
		update_ix.accounts[0],
		AccountMeta::new_readonly(admin, true)
	);
	assert_eq!(
		update_ix.accounts[1],
		AccountMeta::new_readonly(registry_config, false)
	);
	assert_eq!(update_ix.accounts[2], AccountMeta::new(role_entry, false));
	assert_eq!(update_ix.data, bytemuck::bytes_of(&update_payload).to_vec());

	let deactivate = DeactivateRole::new(admin, registry_config, role_entry);
	let deactivate_payload = DeactivateRoleInstructionData::new();
	let deactivate_ix = deactivate.instruction(deactivate_payload);
	assert_eq!(deactivate_ix.accounts.len(), 3);
	assert_eq!(
		deactivate_ix.accounts[1],
		AccountMeta::new_readonly(registry_config, false)
	);
	assert_eq!(
		deactivate_ix.data,
		bytemuck::bytes_of(&deactivate_payload).to_vec()
	);

	let rotate = RotateAdmin::new(admin, new_admin, registry_config);
	let rotate_payload = RotateAdminInstructionData::new();
	let rotate_ix = rotate.instruction(rotate_payload);
	assert_eq!(rotate_ix.accounts.len(), 3);
	assert_eq!(
		rotate_ix.accounts[0],
		AccountMeta::new_readonly(admin, true)
	);
	assert_eq!(
		rotate_ix.accounts[1],
		AccountMeta::new_readonly(new_admin, false)
	);
	assert_eq!(
		rotate_ix.accounts[2],
		AccountMeta::new(registry_config, false)
	);
	assert_eq!(rotate_ix.data, bytemuck::bytes_of(&rotate_payload).to_vec());
}
