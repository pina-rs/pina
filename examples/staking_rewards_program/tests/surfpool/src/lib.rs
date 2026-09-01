#![cfg(test)]

//! Surfpool coverage for the staking rewards example: provision real SPL
//! mints, initialize a pool with vaults, then move the stake accounting
//! through open/deposit/withdraw on-chain.

use pina_test::Account;
use pina_test::AccountMeta;
use pina_test::Instruction;
use pina_test::Keypair;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Signer;
use pina_test::TestError;
use program_under_test::ID;
use program_under_test::StakingInstruction;

/// SPL Token (Tokenkeg…), one of the example's allowlisted programs.
const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ATA_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

const MINT_SPACE: u64 = 82;
const DECIMALS: u8 = 6;
const FUND: u64 = 1_000_000_000;
const POOL_SEED: &[u8] = b"pool";
const POSITION_SEED: &[u8] = b"position";
const DEPOSIT: u64 = 1_500;
const WITHDRAW: u64 = 500;

fn token_program_id() -> Pubkey {
	Pubkey::from_str_const(TOKEN_PROGRAM)
}

fn ata_program_id() -> Pubkey {
	Pubkey::from_str_const(ATA_PROGRAM)
}

fn ata_of(wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
	Pubkey::find_program_address(
		&[wallet.as_ref(), token_program_id().as_ref(), mint.as_ref()],
		&ata_program_id(),
	)
	.0
}

fn pool_pda(program_id: &Pubkey, stake_mint: &Pubkey, reward_mint: &Pubkey) -> (Pubkey, u8) {
	Pubkey::find_program_address(
		&[POOL_SEED, stake_mint.as_ref(), reward_mint.as_ref()],
		program_id,
	)
}

fn position_pda(program_id: &Pubkey, pool: &Pubkey, owner: &Pubkey) -> (Pubkey, u8) {
	Pubkey::find_program_address(&[POSITION_SEED, pool.as_ref(), owner.as_ref()], program_id)
}

fn rent_minimum(space: u64) -> u64 {
	pina_test::Rent::default().minimum_balance(usize::try_from(space).expect("space"))
}

fn create_account_instruction(
	_program: &ProgramTest,
	payer: &Pubkey,
	new_account: &Pubkey,
	lamports: u64,
	space: u64,
	owner: &Pubkey,
) -> pina_test::Instruction {
	let mut data = vec![0u8, 0, 0, 0];
	data.extend_from_slice(&lamports.to_le_bytes());
	data.extend_from_slice(&space.to_le_bytes());
	data.extend_from_slice(owner.as_ref());

	// Create-account targets the SYSTEM program; `owner` rides in the data.
	Instruction::new_with_bytes(
		Pubkey::default(),
		&data,
		vec![
			AccountMeta::new(*payer, true),
			AccountMeta::new(*new_account, true),
			AccountMeta::new_readonly(*owner, false),
		],
	)
}

fn provision_mint(
	program: &ProgramTest,
	payer: &Pubkey,
	authority: &Keypair,
) -> Result<Pubkey, TestError> {
	let mint = Keypair::new();
	let create = create_account_instruction(
		program,
		payer,
		&mint.pubkey(),
		rent_minimum(MINT_SPACE),
		MINT_SPACE,
		&token_program_id(),
	);
	program.send_with_signers(create, &[&mint])?;

	// InitializeMint2 = tag 20.
	let mut data = vec![20u8];
	data.push(DECIMALS);
	data.extend_from_slice(authority.pubkey().as_ref());
	data.extend_from_slice(&0u32.to_le_bytes());
	let initialize = Instruction::new_with_bytes(
		token_program_id(),
		&data,
		vec![AccountMeta::new(mint.pubkey(), false)],
	);
	program.send_instruction(initialize)?;

	Ok(mint.pubkey())
}

fn initialize_pool_instruction(
	program: &ProgramTest,
	admin: &Pubkey,
	stake_mint: &Pubkey,
	reward_mint: &Pubkey,
	pool: &Pubkey,
	stake_vault: &Pubkey,
	reward_vault: &Pubkey,
	bump: u8,
) -> pina_test::Instruction {
	program.instruction(
		&[StakingInstruction::InitializePool as u8, bump],
		vec![
			AccountMeta::new(*admin, true),
			AccountMeta::new_readonly(*stake_mint, false),
			AccountMeta::new_readonly(*reward_mint, false),
			AccountMeta::new(*pool, false),
			AccountMeta::new(*stake_vault, false),
			AccountMeta::new(*reward_vault, false),
			AccountMeta::new_readonly(ata_program_id(), false),
			AccountMeta::new_readonly(Pubkey::default(), false),
			AccountMeta::new_readonly(token_program_id(), false),
		],
	)
}

fn open_position_instruction(
	program: &ProgramTest,
	user: &Pubkey,
	pool: &Pubkey,
	position: &Pubkey,
	bump: u8,
) -> pina_test::Instruction {
	program.instruction(
		&[StakingInstruction::OpenPosition as u8, bump],
		vec![
			AccountMeta::new(*user, true),
			AccountMeta::new_readonly(*pool, false),
			AccountMeta::new(*position, false),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

fn deposit_instruction(
	program: &ProgramTest,
	user: &Pubkey,
	stake_mint: &Pubkey,
	pool: &Pubkey,
	position: &Pubkey,
	user_stake_ata: &Pubkey,
	amount: u64,
) -> pina_test::Instruction {
	let mut data = vec![StakingInstruction::Deposit as u8];
	data.extend_from_slice(&amount.to_le_bytes());

	program.instruction(
		&data,
		vec![
			AccountMeta::new(*user, true),
			AccountMeta::new_readonly(*stake_mint, false),
			AccountMeta::new(*pool, false),
			AccountMeta::new(*position, false),
			AccountMeta::new(*user_stake_ata, false),
			AccountMeta::new_readonly(ata_program_id(), false),
			AccountMeta::new_readonly(token_program_id(), false),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

fn withdraw_instruction(
	program: &ProgramTest,
	user: &Pubkey,
	stake_mint: &Pubkey,
	pool: &Pubkey,
	position: &Pubkey,
	user_stake_ata: &Pubkey,
	amount: u64,
) -> pina_test::Instruction {
	let mut data = vec![StakingInstruction::Withdraw as u8];
	data.extend_from_slice(&amount.to_le_bytes());

	program.instruction(
		&data,
		vec![
			AccountMeta::new_readonly(*user, true),
			AccountMeta::new_readonly(*stake_mint, false),
			AccountMeta::new(*pool, false),
			AccountMeta::new(*position, false),
			AccountMeta::new(*user_stake_ata, false),
			AccountMeta::new_readonly(token_program_id(), false),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

/// PoolState content: [disc][admin 32][stake_mint 32][reward_mint 32]
/// [total_staked 8][reward_index 8][paused][bump].
fn assert_pool(
	account: &Account,
	admin: &Pubkey,
	stake_mint: &Pubkey,
	reward_mint: &Pubkey,
	total_staked: u64,
	bump: u8,
) {
	assert_eq!(account.data[0], 1, "discriminator is PoolState");
	assert_eq!(&account.data[1..33], admin.to_bytes());
	assert_eq!(&account.data[33..65], stake_mint.to_bytes());
	assert_eq!(&account.data[65..97], reward_mint.to_bytes());
	assert_eq!(
		&account.data[97..105],
		total_staked.to_le_bytes(),
		"total_staked on-chain"
	);
	assert_eq!(
		&account.data[105..113],
		0u64.to_le_bytes(),
		"reward_index zero"
	);
	assert_eq!(account.data[113], 0, "pool is unpaused");
	assert_eq!(account.data[114], bump);
}

/// PositionState content: [disc][pool 32][owner 32][staked 8][reward_debt 8]
/// [pending 8][bump].
fn assert_position(account: &Account, pool: &Pubkey, owner: &Pubkey, staked: u64, bump: u8) {
	assert_eq!(account.data[0], 2, "discriminator is PositionState");
	assert_eq!(&account.data[1..33], pool.to_bytes());
	assert_eq!(&account.data[33..65], owner.to_bytes());
	assert_eq!(&account.data[65..73], staked.to_le_bytes());
	assert_eq!(account.data[89], bump);
}

fn vault_amount(account: &Account) -> u64 {
	u64::from_le_bytes(account.data[64..72].try_into().expect("token amount"))
}

#[test]
#[ignore = "run with pina test"]
fn pool_positions_and_stake_accounting() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new();
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let admin = program.payer();
		let stake_mint =
			provision_mint(&program, &admin, &mint_authority).expect("provision stake mint");
		let reward_mint =
			provision_mint(&program, &admin, &mint_authority).expect("provision reward mint");

		let (pool, pool_bump) = pool_pda(&program_id, &stake_mint, &reward_mint);
		let stake_vault = ata_of(&pool, &stake_mint);
		let reward_vault = ata_of(&pool, &reward_mint);
		let user_stake_ata = ata_of(&admin, &stake_mint);
		let user_reward_ata = ata_of(&admin, &reward_mint);

		// --- InitializePool ---
		program
			.send_instruction(initialize_pool_instruction(
				&program,
				&admin,
				&stake_mint,
				&reward_mint,
				&pool,
				&stake_vault,
				&reward_vault,
				pool_bump,
			))
			.expect("execute InitializePool");

		assert_pool(
			&program.account(&pool).expect("pool state exists"),
			&admin,
			&stake_mint,
			&reward_mint,
			0,
			pool_bump,
		);
		let vault_account = program.account(&stake_vault).expect("stake vault exists");
		assert_eq!(vault_account.owner, token_program_id());
		assert_eq!(vault_amount(&vault_account), 0);

		// --- OpenPosition ---
		let (position, position_bump) = position_pda(&program_id, &pool, &admin);
		program
			.send_instruction(open_position_instruction(
				&program,
				&admin,
				&pool,
				&position,
				position_bump,
			))
			.expect("execute OpenPosition");
		assert_position(
			&program.account(&position).expect("position exists"),
			&pool,
			&admin,
			0,
			position_bump,
		);

		// --- Deposit ---
		program
			.send_instruction(deposit_instruction(
				&program,
				&admin,
				&stake_mint,
				&pool,
				&position,
				&user_stake_ata,
				DEPOSIT,
			))
			.expect("execute Deposit");
		assert_position(
			&program.account(&position).expect("position after deposit"),
			&pool,
			&admin,
			DEPOSIT,
			position_bump,
		);

		// --- Withdraw ---
		program
			.send_instruction(withdraw_instruction(
				&program,
				&admin,
				&stake_mint,
				&pool,
				&position,
				&user_stake_ata,
				WITHDRAW,
			))
			.expect("execute Withdraw");
		assert_position(
			&program.account(&position).expect("position after withdraw"),
			&pool,
			&admin,
			DEPOSIT - WITHDRAW,
			position_bump,
		);

		// Withdrawing more than staked must fail and leave the balance.
		let error = program
			.send_instruction(withdraw_instruction(
				&program,
				&admin,
				&stake_mint,
				&pool,
				&position,
				&user_stake_ata,
				DEPOSIT - WITHDRAW + 1,
			))
			.expect_err("cannot withdraw more than the stake");
		assert_eq!(error.operation(), "execute program instruction");
		eprintln!("over-withdraw error: {}", error.message());

		// The claim path creates the user's reward ATA address safely.
		let claim = program.instruction(
			&[StakingInstruction::Claim as u8],
			vec![
				AccountMeta::new(admin, true),
				AccountMeta::new_readonly(reward_mint, false),
				AccountMeta::new_readonly(pool, false),
				AccountMeta::new(position, false),
				AccountMeta::new(user_reward_ata, false),
				AccountMeta::new_readonly(ata_program_id(), false),
				AccountMeta::new_readonly(token_program_id(), false),
				AccountMeta::new_readonly(Pubkey::default(), false),
			],
		);
		program.send_instruction(claim).expect("execute Claim");
		let reward_account = program
			.account(&user_reward_ata)
			.expect("reward ATA created by claim idempotent");
		assert_eq!(vault_amount(&reward_account), 0, "no real reward payout");

		program.stop().expect("stop isolated program test");
	});
}
