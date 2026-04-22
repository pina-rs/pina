use client::generated::accounts::POOL_STATE_DISCRIMINATOR;
use client::generated::accounts::POSITION_STATE_DISCRIMINATOR;
use client::generated::instructions::Claim;
use client::generated::instructions::ClaimInstructionData;
use client::generated::instructions::Deposit;
use client::generated::instructions::DepositInstructionData;
use client::generated::instructions::InitializePool;
use client::generated::instructions::InitializePoolInstructionData;
use client::generated::instructions::OpenPosition;
use client::generated::instructions::OpenPositionInstructionData;
use client::generated::instructions::Withdraw;
use client::generated::instructions::WithdrawInstructionData;
use client::generated::instructions::{self};
use pina_pod_primitives::PodU64;
use solana_instruction::AccountMeta;
use solana_pubkey::Pubkey;
use solana_pubkey::pubkey;
use staking_rewards_program_client as client;

#[test]
fn staking_rewards_program_client_has_expected_contract_shape() {
	let program_id = client::generated::programs::STAKING_REWARDS_PROGRAM_ID;
	assert_eq!(
		program_id,
		pubkey!("9MBwKBjzTLtLe8PkHVhi5CfGxKo8gCYbMEg5NMt1tcvr"),
	);

	assert_eq!(instructions::INITIALIZE_POOL_DISCRIMINATOR, 0u8);
	assert_eq!(instructions::OPEN_POSITION_DISCRIMINATOR, 1u8);
	assert_eq!(instructions::DEPOSIT_DISCRIMINATOR, 2u8);
	assert_eq!(instructions::WITHDRAW_DISCRIMINATOR, 3u8);
	assert_eq!(instructions::CLAIM_DISCRIMINATOR, 4u8);
	assert_eq!(POOL_STATE_DISCRIMINATOR, 1u8);
	assert_eq!(POSITION_STATE_DISCRIMINATOR, 2u8);

	let admin = Pubkey::new_unique();
	let stake_mint = Pubkey::new_unique();
	let reward_mint = Pubkey::new_unique();
	let pool_state = Pubkey::new_unique();
	let stake_vault = Pubkey::new_unique();
	let reward_vault = Pubkey::new_unique();
	let token_program = Pubkey::new_unique();

	let init_pool = InitializePool::new(
		admin,
		stake_mint,
		reward_mint,
		pool_state,
		stake_vault,
		reward_vault,
		token_program,
	);
	let init_payload = InitializePoolInstructionData::new(8);
	let init_ix = init_pool.instruction(init_payload);
	assert_eq!(init_ix.program_id, program_id);
	assert_eq!(init_ix.accounts.len(), 8);
	assert_eq!(init_ix.accounts[0], AccountMeta::new_readonly(admin, true));
	assert_eq!(
		init_ix.accounts[1],
		AccountMeta::new_readonly(stake_mint, false)
	);
	assert_eq!(
		init_ix.accounts[2],
		AccountMeta::new_readonly(reward_mint, false)
	);
	assert_eq!(init_ix.accounts[3], AccountMeta::new(pool_state, false));
	assert_eq!(
		init_ix.accounts[7],
		AccountMeta::new_readonly(token_program, false)
	);
	assert_eq!(init_ix.data, bytemuck::bytes_of(&init_payload).to_vec());

	let position_state = Pubkey::new_unique();
	let open_position = OpenPosition::new(admin, pool_state, position_state);
	let open_payload = OpenPositionInstructionData::new(5);
	let open_ix = open_position.instruction(open_payload);
	assert_eq!(open_ix.accounts.len(), 4);
	assert_eq!(open_ix.accounts[0], AccountMeta::new_readonly(admin, true));
	assert_eq!(
		open_ix.accounts[1],
		AccountMeta::new_readonly(pool_state, false)
	);
	assert_eq!(open_ix.accounts[2], AccountMeta::new(position_state, false));
	assert_eq!(open_ix.data, bytemuck::bytes_of(&open_payload).to_vec());

	let user_stake_ata = Pubkey::new_unique();
	let deposit = Deposit::new(
		admin,
		stake_mint,
		pool_state,
		position_state,
		user_stake_ata,
		token_program,
	);
	let deposit_payload = DepositInstructionData::new(PodU64::from_primitive(250));
	let deposit_ix = deposit.instruction(deposit_payload);
	assert_eq!(deposit_ix.accounts.len(), 7);
	assert_eq!(
		deposit_ix.accounts[0],
		AccountMeta::new_readonly(admin, true)
	);
	assert_eq!(
		deposit_ix.accounts[1],
		AccountMeta::new_readonly(stake_mint, false)
	);
	assert_eq!(deposit_ix.accounts[2], AccountMeta::new(pool_state, false));
	assert_eq!(
		deposit_ix.accounts[4],
		AccountMeta::new(user_stake_ata, false)
	);
	assert_eq!(
		deposit_ix.accounts[5],
		AccountMeta::new_readonly(token_program, false)
	);
	assert_eq!(
		deposit_ix.data,
		bytemuck::bytes_of(&deposit_payload).to_vec()
	);

	let withdraw = Withdraw::new(
		admin,
		stake_mint,
		pool_state,
		position_state,
		user_stake_ata,
		token_program,
	);
	let withdraw_payload = WithdrawInstructionData::new(PodU64::from_primitive(125));
	let withdraw_ix = withdraw.instruction(withdraw_payload);
	assert_eq!(withdraw_ix.accounts.len(), 7);
	assert_eq!(
		withdraw_ix.accounts[1],
		AccountMeta::new_readonly(stake_mint, false)
	);
	assert_eq!(withdraw_ix.accounts[2], AccountMeta::new(pool_state, false));
	assert_eq!(
		withdraw_ix.accounts[3],
		AccountMeta::new(position_state, false)
	);
	assert_eq!(
		withdraw_ix.accounts[6],
		AccountMeta::new_readonly(pubkey!("11111111111111111111111111111111"), false,)
	);
	assert_eq!(
		withdraw_ix.data,
		bytemuck::bytes_of(&withdraw_payload).to_vec()
	);

	let user_reward_ata = Pubkey::new_unique();
	let claim = Claim::new(
		admin,
		reward_mint,
		pool_state,
		position_state,
		user_reward_ata,
		token_program,
	);
	let claim_payload = ClaimInstructionData::new();
	let claim_ix = claim.instruction(claim_payload);
	assert_eq!(claim_ix.accounts.len(), 7);
	assert_eq!(claim_ix.accounts[0], AccountMeta::new_readonly(admin, true));
	assert_eq!(
		claim_ix.accounts[1],
		AccountMeta::new_readonly(reward_mint, false)
	);
	assert_eq!(
		claim_ix.accounts[2],
		AccountMeta::new_readonly(pool_state, false)
	);
	assert_eq!(
		claim_ix.accounts[4],
		AccountMeta::new(user_reward_ata, false)
	);
	assert_eq!(
		claim_ix.accounts[6],
		AccountMeta::new_readonly(pubkey!("11111111111111111111111111111111"), false,)
	);
	assert_eq!(claim_ix.data, bytemuck::bytes_of(&claim_payload).to_vec());
}
