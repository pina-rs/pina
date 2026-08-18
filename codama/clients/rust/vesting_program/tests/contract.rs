use client::generated::accounts::VESTING_STATE_DISCRIMINATOR;
use client::generated::instructions::Cancel;
use client::generated::instructions::CancelInstructionData;
use client::generated::instructions::Claim;
use client::generated::instructions::ClaimInstructionData;
use client::generated::instructions::Initialize;
use client::generated::instructions::InitializeInstructionData;
use client::generated::instructions::{self};
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
	let associated_token_program = pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

	let initialize = Initialize::new(
		admin,
		beneficiary,
		mint,
		vesting_state,
		vault,
		token_program,
	);
	let init_payload = InitializeInstructionData::new(|data| {
		data.total_amount.set(1_000);
		data.start_ts.set(200);
		data.cliff_ts.set(300);
		data.end_ts.set(400);
		data.bump = 9;
	})
	.unwrap();
	let init_ix = initialize.instruction(init_payload);
	assert_eq!(init_ix.program_id, program_id);
	assert_eq!(init_ix.accounts.len(), 8);
	assert_eq!(init_ix.accounts[0], AccountMeta::new(admin, true));
	assert_eq!(init_ix.accounts[3], AccountMeta::new(vesting_state, false));
	assert_eq!(
		init_ix.accounts[5],
		AccountMeta::new_readonly(associated_token_program, false)
	);
	let mut expected_init = vec![0];
	expected_init.extend_from_slice(&1_000u64.to_le_bytes());
	expected_init.extend_from_slice(&200u64.to_le_bytes());
	expected_init.extend_from_slice(&300u64.to_le_bytes());
	expected_init.extend_from_slice(&400u64.to_le_bytes());
	expected_init.push(9);
	assert_eq!(init_ix.data, expected_init);

	let mut state_bytes = vec![0u8; client::generated::accounts::VestingState::LEN];
	{
		let state = client::generated::accounts::VestingState::initialize(&mut state_bytes)
			.expect("vesting state storage should initialize");
		state.admin = admin;
		state.beneficiary = beneficiary;
		state.mint = mint;
		state.total_amount.set(1_000);
		state.start_ts.set(2_000);
		state.cliff_ts.set(2_100);
		state.end_ts.set(3_000);
		state.cancelled.set(false);
		state.bump = 9;
	}
	let parsed_state = client::generated::accounts::VestingState::from_bytes(&state_bytes)
		.expect("vesting state storage should validate");
	assert_eq!(parsed_state.discriminator, VESTING_STATE_DISCRIMINATOR);
	assert_eq!(parsed_state.admin, admin);
	assert_eq!(parsed_state.total_amount.get(), 1_000);

	let claim = Claim::new(
		beneficiary,
		mint,
		vesting_state,
		Pubkey::new_unique(),
		vault,
		token_program,
	);
	let claim_payload = ClaimInstructionData::new(|data| data.amount.set(10)).unwrap();
	let claim_ix = claim.instruction(claim_payload);
	assert_eq!(claim_ix.accounts.len(), 8);
	assert_eq!(claim_ix.accounts[0], AccountMeta::new(beneficiary, true));
	assert_eq!(
		claim_ix.accounts[5],
		AccountMeta::new_readonly(associated_token_program, false)
	);
	let mut expected_claim = vec![1];
	expected_claim.extend_from_slice(&10u64.to_le_bytes());
	assert_eq!(claim_ix.data, expected_claim);

	let cancel = Cancel::new(admin, mint, vesting_state, vault, token_program);
	let cancel_payload = CancelInstructionData::new(|_| {}).unwrap();
	let cancel_ix = cancel.instruction(cancel_payload);
	assert_eq!(cancel_ix.accounts.len(), 5);
	assert_eq!(cancel_ix.data, vec![2]);
}
