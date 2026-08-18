use client::generated::accounts::VESTING_STATE_DISCRIMINATOR;
use client::generated::instructions::Cancel;
use client::generated::instructions::CancelInstructionData;
use client::generated::instructions::Claim;
use client::generated::instructions::ClaimInstructionData;
use client::generated::instructions::Initialize;
use client::generated::instructions::InitializeInstructionData;
use client::generated::instructions::{self};
use pina_pod_primitives::PodBool;
use pina_pod_primitives::PodU64;
use solana_instruction::AccountMeta;
use solana_pubkey::Pubkey;
use solana_pubkey::pubkey;
use vesting_program_client as client;

#[test]
fn vesting_program_client_has_expected_contract_shape() {
	let program_id = client::generated::programs::VESTING_PROGRAM_ID;
	assert_eq!(
		program_id,
		pubkey!("FEa5fqN6NACrhWUZSBdGKybJKNxkdw8cdLvRvTARsFHh"),
	);

	assert_eq!(instructions::INITIALIZE_DISCRIMINATOR, 0u8);
	assert_eq!(instructions::CLAIM_DISCRIMINATOR, 1u8);
	assert_eq!(instructions::CANCEL_DISCRIMINATOR, 2u8);
	assert_eq!(VESTING_STATE_DISCRIMINATOR, 1u8);

	let admin = Pubkey::new_unique();
	let beneficiary = Pubkey::new_unique();
	let mint = Pubkey::new_unique();
	let vesting_state = Pubkey::new_unique();
	let vault = Pubkey::new_unique();
	let token_program = Pubkey::new_unique();

	let initialize = Initialize::new(
		admin,
		beneficiary,
		mint,
		vesting_state,
		vault,
		token_program,
	);
	let init_payload = InitializeInstructionData::new(
		PodU64::from_primitive(1_000),
		PodU64::from_primitive(200),
		PodU64::from_primitive(300),
		PodU64::from_primitive(400),
		9,
	);
	let init_ix = initialize.instruction(init_payload);
	assert_eq!(init_ix.program_id, program_id);
	assert_eq!(init_ix.accounts.len(), 7);
	assert_eq!(init_ix.accounts[0], AccountMeta::new_readonly(admin, true),);
	assert_eq!(
		init_ix.accounts[1],
		AccountMeta::new_readonly(beneficiary, false),
	);
	assert_eq!(init_ix.accounts[2], AccountMeta::new_readonly(mint, false));
	assert_eq!(init_ix.accounts[3], AccountMeta::new(vesting_state, false));
	assert_eq!(init_ix.accounts[4], AccountMeta::new(vault, false));
	assert_eq!(
		init_ix.accounts[5],
		AccountMeta::new_readonly(pubkey!("11111111111111111111111111111111"), false,),
	);
	assert_eq!(
		init_ix.accounts[6],
		AccountMeta::new_readonly(token_program, false)
	);
	assert_eq!(init_ix.data, bytemuck::bytes_of(&init_payload).to_vec());

	let state = client::generated::accounts::VestingState::new(
		admin,
		beneficiary,
		mint,
		PodU64::from_primitive(1_000),
		PodU64::from_primitive(0),
		PodU64::from_primitive(2_000),
		PodU64::from_primitive(2_100),
		PodU64::from_primitive(3_000),
		PodBool::from_bool(false),
		9,
	);
	let state_bytes = bytemuck::bytes_of(&state);
	let parsed_state = client::generated::accounts::VestingState::from_bytes(state_bytes)
		.unwrap_or_else(|_| panic!("vesting state decoder should decode self-serialized account"));
	assert_eq!(parsed_state.discriminator, VESTING_STATE_DISCRIMINATOR);
	assert_eq!(parsed_state.admin, admin);

	let claim = Claim::new(
		beneficiary,
		mint,
		vesting_state,
		Pubkey::new_unique(),
		vault,
		token_program,
	);
	let claim_payload = ClaimInstructionData::new(PodU64::from_primitive(10));
	let claim_ix = claim.instruction(claim_payload);
	assert_eq!(claim_ix.accounts.len(), 7);
	assert_eq!(
		claim_ix.accounts[0],
		AccountMeta::new_readonly(beneficiary, true),
	);
	assert_eq!(claim_ix.accounts[1], AccountMeta::new_readonly(mint, false));
	assert_eq!(claim_ix.accounts[2], AccountMeta::new(vesting_state, false));
	assert_eq!(
		claim_ix.accounts[6],
		AccountMeta::new_readonly(token_program, false)
	);
	assert_eq!(claim_ix.data, bytemuck::bytes_of(&claim_payload).to_vec());

	let cancel = Cancel::new(admin, mint, vesting_state, vault, token_program);
	let cancel_payload = CancelInstructionData::new();
	let cancel_ix = cancel.instruction(cancel_payload);
	assert_eq!(cancel_ix.accounts.len(), 5);
	assert_eq!(
		cancel_ix.accounts[0],
		AccountMeta::new_readonly(admin, true)
	);
	assert_eq!(
		cancel_ix.accounts[4],
		AccountMeta::new_readonly(token_program, false)
	);
	assert_eq!(cancel_ix.data, bytemuck::bytes_of(&cancel_payload).to_vec());
}
