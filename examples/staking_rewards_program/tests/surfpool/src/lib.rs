#![cfg(test)]

//! Surfpool coverage for the staking rewards example: provision real SPL
//! mints, initialize a pool with vaults, then move stake tokens through
//! open/deposit/withdraw on-chain.

use pina_test::Account;
use pina_test::AccountMeta;
use pina_test::Instruction;
use pina_test::Keypair;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Signer;
use pina_test::TestError;
use program_under_test::ID;
use program_under_test::REWARD_INDEX_SCALE;
use program_under_test::StakingError;
use program_under_test::StakingInstruction;

/// SPL Token (Tokenkeg…), one of the example's allowlisted programs.
const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ATA_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

const MINT_SPACE: u64 = 82;
const DECIMALS: u8 = 6;
const FUND: u64 = 1_000_000_000;
const SEED_POOL: &[u8] = b"pool";
const SEED_POSITION: &[u8] = b"position";
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
		&[SEED_POOL, stake_mint.as_ref(), reward_mint.as_ref()],
		program_id,
	)
}

fn position_pda(program_id: &Pubkey, pool: &Pubkey, owner: &Pubkey) -> (Pubkey, u8) {
	Pubkey::find_program_address(&[SEED_POSITION, pool.as_ref(), owner.as_ref()], program_id)
}

/// Find a bump that derives a valid PDA for `seeds` without being canonical.
///
/// `find_program_address` returns the highest bump that yields a valid address,
/// and any lower bump that also happens to be valid yields a *different*
/// account for the same seeds. This is the shadow account an attacker creates
/// by calling a creation instruction a second time with that bump.
fn noncanonical_pda(seeds: &[&[u8]], program_id: &Pubkey) -> (Pubkey, u8) {
	let (_, canonical) = Pubkey::find_program_address(seeds, program_id);

	for candidate in (0..canonical).rev() {
		let bump = [candidate];
		let mut with_bump: Vec<&[u8]> = seeds.to_vec();
		with_bump.push(&bump);
		if let Ok(address) = Pubkey::create_program_address(&with_bump, program_id) {
			return (address, candidate);
		}
	}

	panic!("seeds with a noncanonical valid bump");
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
	seed: u8,
) -> Result<Pubkey, TestError> {
	let mint = Keypair::new_from_array([seed; 32]);
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
		// discriminator + migration version + bump.
		&[StakingInstruction::InitializePool as u8, 0u8, bump],
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
		// discriminator + migration version + bump.
		&[StakingInstruction::OpenPosition as u8, 0u8, bump],
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
	stake_vault: &Pubkey,
	amount: u64,
) -> pina_test::Instruction {
	// discriminator + migration version, then the u64 amount.
	let mut data = vec![StakingInstruction::Deposit as u8, 0u8];
	data.extend_from_slice(&amount.to_le_bytes());

	program.instruction(
		&data,
		vec![
			AccountMeta::new(*user, true),
			AccountMeta::new_readonly(*stake_mint, false),
			AccountMeta::new(*pool, false),
			AccountMeta::new(*position, false),
			AccountMeta::new(*user_stake_ata, false),
			AccountMeta::new(*stake_vault, false),
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
	stake_vault: &Pubkey,
	amount: u64,
) -> pina_test::Instruction {
	let mut data = vec![StakingInstruction::Withdraw as u8, 0u8];
	data.extend_from_slice(&amount.to_le_bytes());

	program.instruction(
		&data,
		vec![
			AccountMeta::new_readonly(*user, true),
			AccountMeta::new_readonly(*stake_mint, false),
			AccountMeta::new(*pool, false),
			AccountMeta::new(*position, false),
			AccountMeta::new(*user_stake_ata, false),
			AccountMeta::new(*stake_vault, false),
			AccountMeta::new_readonly(token_program_id(), false),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

/// PoolState content: [disc][version][admin 32][stake_mint 32][reward_mint 32]
/// [total_staked 8][reward_index 8][outstanding_rewards 8][paused][bump].
/// `tests/abi_layout.rs` pins the same envelope geometry.
fn set_reward_index_instruction(
	program: &ProgramTest,
	admin: &Pubkey,
	pool: &Pubkey,
	reward_mint: &Pubkey,
	token_program: &Pubkey,
	reward_vault: &Pubkey,
	new_index: u64,
) -> pina_test::Instruction {
	let mut data = vec![StakingInstruction::SetRewardIndex as u8, 0u8];
	data.extend_from_slice(&new_index.to_le_bytes());

	program.instruction(
		&data,
		vec![
			AccountMeta::new_readonly(*admin, true),
			AccountMeta::new(*pool, false),
			AccountMeta::new_readonly(*reward_mint, false),
			AccountMeta::new_readonly(*token_program, false),
			AccountMeta::new(*reward_vault, false),
		],
	)
}

fn claim_instruction(
	program: &ProgramTest,
	user: &Pubkey,
	reward_mint: &Pubkey,
	pool: &Pubkey,
	position: &Pubkey,
	user_reward_ata: &Pubkey,
	reward_vault: &Pubkey,
) -> pina_test::Instruction {
	program.instruction(
		&[StakingInstruction::Claim as u8, 0u8],
		vec![
			AccountMeta::new(*user, true),
			AccountMeta::new_readonly(*reward_mint, false),
			AccountMeta::new(*pool, false),
			AccountMeta::new(*position, false),
			AccountMeta::new(*user_reward_ata, false),
			AccountMeta::new(*reward_vault, false),
			AccountMeta::new_readonly(ata_program_id(), false),
			AccountMeta::new_readonly(token_program_id(), false),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

/// SPL `MintTo` = tag 7.
fn mint_into(
	program: &ProgramTest,
	mint: &Pubkey,
	destination: &Pubkey,
	authority: &Keypair,
	amount: u64,
) -> Result<(), TestError> {
	let mut data = vec![7u8];
	data.extend_from_slice(&amount.to_le_bytes());
	let instruction = Instruction::new_with_bytes(
		token_program_id(),
		&data,
		vec![
			AccountMeta::new(*mint, false),
			AccountMeta::new(*destination, false),
			AccountMeta::new_readonly(authority.pubkey(), true),
		],
	);

	program
		.send_with_signers(instruction, &[authority])
		.map(|_| ())
}

/// Create a wallet's associated token account directly through the associated
/// token program (`Create` carries no data).
///
/// `Deposit` no longer creates a token account out of nothing for the
/// depositor to fund later: a deposit moves tokens, so the tests fund the ATA
/// *before* depositing. This is the helper that makes the account mintable.
fn create_ata(
	program: &ProgramTest,
	payer: &Pubkey,
	wallet: &Pubkey,
	mint: &Pubkey,
) -> Result<(), TestError> {
	let instruction = Instruction::new_with_bytes(
		ata_program_id(),
		&[],
		vec![
			AccountMeta::new(*payer, true),
			AccountMeta::new(ata_of(wallet, mint), false),
			AccountMeta::new_readonly(*wallet, false),
			AccountMeta::new_readonly(*mint, false),
			AccountMeta::new_readonly(Pubkey::default(), false),
			AccountMeta::new_readonly(token_program_id(), false),
		],
	);

	program.send_instruction(instruction).map(|_| ())
}

/// Fund a wallet's stake ATA with freshly minted tokens, creating it first.
fn fund_stake_ata(
	program: &ProgramTest,
	payer: &Pubkey,
	wallet: &Pubkey,
	stake_mint: &Pubkey,
	mint_authority: &Keypair,
	amount: u64,
) -> Result<(), TestError> {
	create_ata(program, payer, wallet, stake_mint)?;
	mint_into(
		program,
		stake_mint,
		&ata_of(wallet, stake_mint),
		mint_authority,
		amount,
	)
}

fn assert_pool(
	account: &Account,
	admin: &Pubkey,
	stake_mint: &Pubkey,
	reward_mint: &Pubkey,
	total_staked: u64,
	bump: u8,
) {
	assert_eq!(account.data[0], 1, "discriminator is PoolState");
	assert_eq!(account.data[1], 0, "stored migration version is current");
	assert_eq!(&account.data[2..34], admin.to_bytes());
	assert_eq!(&account.data[34..66], stake_mint.to_bytes());
	assert_eq!(&account.data[66..98], reward_mint.to_bytes());
	assert_eq!(
		&account.data[98..106],
		total_staked.to_le_bytes(),
		"total_staked on-chain"
	);
	assert_eq!(
		&account.data[106..114],
		0u64.to_le_bytes(),
		"reward_index zero"
	);
	assert_eq!(
		&account.data[114..122],
		0u64.to_le_bytes(),
		"outstanding_rewards zero"
	);
	assert_eq!(account.data[122], 0, "pool is unpaused");
	assert_eq!(account.data[123], bump);
}

/// PositionState content: [disc][version][pool 32][owner 32][staked 8]
/// [reward_debt 8][pending 8][bump].
fn assert_position(account: &Account, pool: &Pubkey, owner: &Pubkey, staked: u64, bump: u8) {
	assert_eq!(account.data[0], 2, "discriminator is PositionState");
	assert_eq!(account.data[1], 0, "stored migration version is current");
	assert_eq!(&account.data[2..34], pool.to_bytes());
	assert_eq!(&account.data[34..66], owner.to_bytes());
	assert_eq!(&account.data[66..74], staked.to_le_bytes());
	assert_eq!(account.data[90], bump);
}

fn vault_amount(account: &Account) -> u64 {
	u64::from_le_bytes(account.data[64..72].try_into().expect("token amount"))
}

/// The pool's `total_staked`, read straight from the on-chain account bytes
/// (offsets pinned by `assert_pool` above).
fn pool_total_staked(account: &Account) -> u64 {
	assert_eq!(account.data[0], 1, "discriminator is PoolState");
	u64::from_le_bytes(account.data[98..106].try_into().expect("total_staked"))
}

/// The backing invariant the stake custody must hold: every staked token the
/// ledger credits is sitting in the pool's stake vault, so
/// `stake_vault == pool.total_staked` at every pause between instructions.
fn assert_stake_backing(program: &ProgramTest, stake_vault: &Pubkey, pool: &Pubkey) {
	let vault = vault_amount(&program.account(stake_vault).expect("fetch stake vault"));
	let total = pool_total_staked(&program.account(pool).expect("fetch pool state"));
	assert_eq!(
		vault, total,
		"stake vault balance must equal pool.total_staked"
	);
}

/// A deposit naming an account that is not the canonical ATA for its wallet and
/// stake mint is rejected.
///
/// Every pina-side check passes: the pool, the position, and the mint agree, the
/// account is writable, and the instruction payload is well formed. Only the
/// address binding is wrong. The associated token program derives
/// `[wallet, token_program, mint]` itself inside `CreateIdempotent` and rejects
/// the mismatch before creating anything, which is the check the program now
/// relies on instead of restating. This test pins that delegation.
#[test]
#[ignore = "run with pina test"]
fn rejects_a_non_canonical_stake_ata_on_deposit() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([2; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let admin = program.payer();
		let stake_mint =
			provision_mint(&program, &admin, &mint_authority, 3).expect("provision stake mint");
		let reward_mint =
			provision_mint(&program, &admin, &mint_authority, 4).expect("provision reward mint");

		let (pool, pool_bump) = pool_pda(&program_id, &stake_mint, &reward_mint);
		let stake_vault = ata_of(&pool, &stake_mint);
		let reward_vault = ata_of(&pool, &reward_mint);

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

		// A valid ATA for a different wallet is not the canonical ATA for `admin`.
		let other_wallet = Keypair::new_from_array([9; 32]).pubkey();
		let wrong_ata = ata_of(&other_wallet, &stake_mint);

		let error = program
			.send_instruction(deposit_instruction(
				&program,
				&admin,
				&stake_mint,
				&pool,
				&position,
				&wrong_ata,
				&stake_vault,
				10,
			))
			.expect_err("reject a non-canonical stake ATA");
		assert_eq!(error.operation(), "execute program instruction");
		// Pin that the rejection came from the associated token program's own
		// derivation rather than from an earlier pina check. Without this the
		// test would pass even if the account failed for an unrelated reason.
		let message = error.message();
		assert!(
			message.contains("Associated address does not match seed derivation")
				|| message.contains("Provided seeds do not result in a valid address"),
			"expected the ATA program to reject the address, got: {message}"
		);

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn pool_positions_and_stake_accounting() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([2; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let admin = program.payer();
		let stake_mint =
			provision_mint(&program, &admin, &mint_authority, 3).expect("provision stake mint");
		let reward_mint =
			provision_mint(&program, &admin, &mint_authority, 4).expect("provision reward mint");

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
		//
		// A deposit now moves tokens, so the depositor funds their stake ATA
		// first; the instruction transfers the deposit into the stake vault
		// before crediting the position.
		fund_stake_ata(
			&program,
			&admin,
			&admin,
			&stake_mint,
			&mint_authority,
			DEPOSIT,
		)
		.expect("fund depositor's stake ATA");

		program
			.send_instruction(deposit_instruction(
				&program,
				&admin,
				&stake_mint,
				&pool,
				&position,
				&user_stake_ata,
				&stake_vault,
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
		assert_eq!(
			vault_amount(
				&program
					.account(&user_stake_ata)
					.expect("fetch user stake ATA")
			),
			0,
			"the deposit left the depositor's ATA"
		);
		assert_eq!(
			vault_amount(&program.account(&stake_vault).expect("fetch stake vault")),
			DEPOSIT,
			"the deposit arrived in the stake vault"
		);
		assert_stake_backing(&program, &stake_vault, &pool);

		// --- Withdraw ---
		//
		// The principal comes back out of the stake vault under the pool's
		// signature, so the withdrawal moves real tokens this time.
		program
			.send_instruction(withdraw_instruction(
				&program,
				&admin,
				&stake_mint,
				&pool,
				&position,
				&user_stake_ata,
				&stake_vault,
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
		assert_eq!(
			vault_amount(
				&program
					.account(&user_stake_ata)
					.expect("fetch user stake ATA")
			),
			WITHDRAW,
			"the withdrawn principal returned to the depositor's ATA"
		);
		assert_eq!(
			vault_amount(&program.account(&stake_vault).expect("fetch stake vault")),
			DEPOSIT - WITHDRAW,
			"the withdrawn principal left the stake vault"
		);
		assert_stake_backing(&program, &stake_vault, &pool);

		// Withdrawing more than staked must fail and leave the balance.
		let error = program
			.send_instruction(withdraw_instruction(
				&program,
				&admin,
				&stake_mint,
				&pool,
				&position,
				&user_stake_ata,
				&stake_vault,
				DEPOSIT - WITHDRAW + 1,
			))
			.expect_err("cannot withdraw more than the stake");
		// The exact variant matters: printing the error passes when the program
		// fails for an unrelated reason, and the balance check is the behavior
		// this test exists to pin.
		pina_test::assert_custom_error(&error, StakingError::InsufficientBalance as u32);
		assert_eq!(
			vault_amount(&program.account(&stake_vault).expect("fetch stake vault")),
			DEPOSIT - WITHDRAW,
			"the refused withdrawal moved no principal"
		);
		assert_stake_backing(&program, &stake_vault, &pool);

		// No reward index has moved, so the position has accrued nothing and a
		// claim is refused rather than creating an empty payout. The reward
		// release path has its own end-to-end test.
		let error = program
			.send_instruction(claim_instruction(
				&program,
				&admin,
				&reward_mint,
				&pool,
				&position,
				&user_reward_ata,
				&reward_vault,
			))
			.expect_err("a claim with no accrual is refused");
		pina_test::assert_custom_error(&error, StakingError::NothingToClaim as u32);

		program.stop().expect("stop isolated program test");
	});
}

/// A pool's seeds are `[pool, stake_mint, reward_mint]` and contain no signer,
/// so the pool for a mint pair is a global singleton. This test pins that a
/// second pool for the same pair cannot be created at a noncanonical bump.
///
/// `InitializePool` requires the caller to sign and stores them as admin, but
/// the signature authorizes the *creator*, not the namespace: the seeds do not
/// include the caller. Before pool creation was canonicalized, anyone could
/// pass a noncanonical bump and create a shadow pool for an existing mint pair,
/// with themselves as admin, at an address canonical derivation never returns.
/// Deposits were accepted into it, because every read path validates stored
/// fields (`pool.stake_mint`, `pool.reward_mint`, `pool.admin`) and the
/// position's stored pool address, never the seeds.
///
/// The vaults make this worse rather than better: they are ATAs of the shadow
/// pool's own address, so they are fresh accounts the attacker controls
/// outright, and the shadow pool is fully functional while being invisible to
/// anything that derives the canonical address.
#[test]
#[ignore = "run with pina test"]
fn rejects_a_shadow_pool_at_a_noncanonical_bump() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([2; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let admin = program.payer();
		let stake_mint =
			provision_mint(&program, &admin, &mint_authority, 3).expect("provision stake mint");
		let reward_mint =
			provision_mint(&program, &admin, &mint_authority, 4).expect("provision reward mint");

		let (pool, pool_bump) = pool_pda(&program_id, &stake_mint, &reward_mint);
		program
			.send_instruction(initialize_pool_instruction(
				&program,
				&admin,
				&stake_mint,
				&reward_mint,
				&pool,
				&ata_of(&pool, &stake_mint),
				&ata_of(&pool, &reward_mint),
				pool_bump,
			))
			.expect("execute InitializePool at the canonical address");

		// The attacker creates a second pool for the same mint pair. Their own
		// signature satisfies `assert_signer`, and the seeds never bounded them,
		// so nothing but canonical derivation distinguishes the two pools.
		let attacker = Keypair::new_from_array([77; 32]);
		program
			.fund(&attacker.pubkey(), FUND)
			.expect("fund attacker");
		let seeds: &[&[u8]] = &[SEED_POOL, stake_mint.as_ref(), reward_mint.as_ref()];
		let (shadow_pool, shadow_bump) = noncanonical_pda(seeds, &program_id);
		assert_ne!(shadow_pool, pool, "shadow pool is a distinct address");

		let error = program
			.send_with_signers(
				initialize_pool_instruction(
					&program,
					&attacker.pubkey(),
					&stake_mint,
					&reward_mint,
					&shadow_pool,
					&ata_of(&shadow_pool, &stake_mint),
					&ata_of(&shadow_pool, &reward_mint),
					shadow_bump,
				),
				&[&attacker],
			)
			.expect_err("reject a shadow pool");

		assert_eq!(error.operation(), "execute program instruction");
		assert!(
			program.account(&shadow_pool).is_err(),
			"the shadow pool must not exist"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// A position's seeds are `[position, pool, owner]`, so one position per
/// (pool, owner) is the invariant every deposit and claim assumes. This test
/// pins that a second position for the same pair cannot be opened at a
/// noncanonical bump.
///
/// The duplicate is the double-claim vector. Reward accrual is flat per
/// position with no pro-rata term, so two positions for one owner each accrue
/// the full amount while `staked_amount` is split between them. Both pass every
/// read path, because each is self-consistent and validates against its own
/// stored pool, owner, and bump.
#[test]
#[ignore = "run with pina test"]
fn rejects_a_duplicate_position_at_a_noncanonical_bump() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([2; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let admin = program.payer();
		let stake_mint =
			provision_mint(&program, &admin, &mint_authority, 3).expect("provision stake mint");
		let reward_mint =
			provision_mint(&program, &admin, &mint_authority, 4).expect("provision reward mint");

		let (pool, pool_bump) = pool_pda(&program_id, &stake_mint, &reward_mint);
		program
			.send_instruction(initialize_pool_instruction(
				&program,
				&admin,
				&stake_mint,
				&reward_mint,
				&pool,
				&ata_of(&pool, &stake_mint),
				&ata_of(&pool, &reward_mint),
				pool_bump,
			))
			.expect("execute InitializePool");

		// The user opens their real position...
		let (position, position_bump) = position_pda(&program_id, &pool, &admin);
		program
			.send_instruction(open_position_instruction(
				&program,
				&admin,
				&pool,
				&position,
				position_bump,
			))
			.expect("execute OpenPosition at the canonical address");

		// ...and a second one for the same (pool, owner) pair. `assert_empty`
		// only guards the address being created, and this address is empty.
		let seeds: &[&[u8]] = &[SEED_POSITION, pool.as_ref(), admin.as_ref()];
		let (duplicate, duplicate_bump) = noncanonical_pda(seeds, &program_id);
		assert_ne!(duplicate, position, "duplicate is a distinct address");

		let error = program
			.send_instruction(open_position_instruction(
				&program,
				&admin,
				&pool,
				&duplicate,
				duplicate_bump,
			))
			.expect_err("reject a duplicate position");

		assert_eq!(error.operation(), "execute program instruction");
		assert!(
			program.account(&duplicate).is_err(),
			"the duplicate position must not exist"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// Rewards accrue per index movement and release once.
///
/// The previous version of this program credited `reward_index` on every claim
/// with no per-position checkpoint and never transferred anything, so a claim
/// could be repeated indefinitely. This test is the regression that pins the
/// corrected behavior: one drip pays each unit of stake exactly once, a second
/// claim without a new drip is refused, and the vault balance proves the
/// payout left custody.
#[test]
#[ignore = "run with pina test"]
fn rewards_accrue_once_per_index_and_release() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([2; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let admin = program.payer();
		let stake_mint =
			provision_mint(&program, &admin, &mint_authority, 3).expect("provision stake mint");
		let reward_mint =
			provision_mint(&program, &admin, &mint_authority, 4).expect("provision reward mint");

		let (pool, pool_bump) = pool_pda(&program_id, &stake_mint, &reward_mint);
		let stake_vault = ata_of(&pool, &stake_mint);
		let reward_vault = ata_of(&pool, &reward_mint);
		let user_stake_ata = ata_of(&admin, &stake_mint);
		let user_reward_ata = ata_of(&admin, &reward_mint);
		let (position, position_bump) = position_pda(&program_id, &pool, &admin);

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
		program
			.send_instruction(open_position_instruction(
				&program,
				&admin,
				&pool,
				&position,
				position_bump,
			))
			.expect("execute OpenPosition");

		// Fund the depositor's stake ATA, then deposit: the instruction moves
		// the stake into the vault, so the tokens must exist before it runs.
		let staked = 1_000u64;
		fund_stake_ata(
			&program,
			&admin,
			&admin,
			&stake_mint,
			&mint_authority,
			staked,
		)
		.expect("fund stake ATA");
		program
			.send_instruction(deposit_instruction(
				&program,
				&admin,
				&stake_mint,
				&pool,
				&position,
				&user_stake_ata,
				&stake_vault,
				staked,
			))
			.expect("execute Deposit");
		assert_eq!(
			vault_amount(&program.account(&stake_vault).expect("fetch stake vault")),
			staked,
			"the deposit moved the stake into the vault"
		);
		assert_eq!(
			vault_amount(
				&program
					.account(&user_stake_ata)
					.expect("fetch user stake ATA")
			),
			0,
			"the deposit left the depositor's ATA"
		);
		assert_stake_backing(&program, &stake_vault, &pool);

		// Fund the reward vault so a payout has something to release.
		mint_into(&program, &reward_mint, &reward_vault, &mint_authority, FUND)
			.expect("fund reward vault");
		let vault_before =
			vault_amount(&program.account(&reward_vault).expect("fetch reward vault"));

		// Time alone accrues nothing in this design: the admin-controlled index
		// is the only clock. Advance it well past any block boundary and confirm
		// a claim is still refused until the index moves.
		program
			.time_travel_to_timestamp_millis(2_500_000_000_000)
			.expect("time travel forward");
		let error = program
			.send_instruction(claim_instruction(
				&program,
				&admin,
				&reward_mint,
				&pool,
				&position,
				&user_reward_ata,
				&reward_vault,
			))
			.expect_err("a claim after time travel but no drip is refused");
		pina_test::assert_custom_error(&error, StakingError::NothingToClaim as u32);

		// A drip of one full index unit: one reward token per staked token.
		let index = REWARD_INDEX_SCALE;
		program
			.send_instruction(set_reward_index_instruction(
				&program,
				&admin,
				&pool,
				&reward_mint,
				&token_program_id(),
				&reward_vault,
				index,
			))
			.expect("execute SetRewardIndex");

		// The drip may not move rewards backwards: that is what would let a
		// position re-claim rewards it already released.
		let error = program
			.send_instruction(set_reward_index_instruction(
				&program,
				&admin,
				&pool,
				&reward_mint,
				&token_program_id(),
				&reward_vault,
				index - 1,
			))
			.expect_err("reject a regressed reward index");
		assert_eq!(
			error.transaction_error(),
			Some(pina_test::TransactionError::InstructionError(
				0,
				pina_test::InstructionError::Custom(StakingError::RewardIndexRegressed as u32)
			)),
			"a lower index must be refused with RewardIndexRegressed"
		);

		// Claim releases exactly the accrued amount.
		program
			.send_instruction(claim_instruction(
				&program,
				&admin,
				&reward_mint,
				&pool,
				&position,
				&user_reward_ata,
				&reward_vault,
			))
			.expect("execute Claim");

		let expected = staked;
		let claimed = vault_amount(&program.account(&user_reward_ata).expect("fetch reward ATA"));
		assert_eq!(
			claimed, expected,
			"the position received its accrued rewards"
		);
		let vault_after =
			vault_amount(&program.account(&reward_vault).expect("fetch reward vault"));
		assert_eq!(
			vault_before - vault_after,
			expected,
			"the vault released exactly the payout"
		);

		// A second claim with no new drip must be refused: the checkpoint was
		// advanced, so nothing further has accrued.
		let error = program
			.send_instruction(claim_instruction(
				&program,
				&admin,
				&reward_mint,
				&pool,
				&position,
				&user_reward_ata,
				&reward_vault,
			))
			.expect_err("reject a second claim with no new accrual");
		assert_eq!(
			error.transaction_error(),
			Some(pina_test::TransactionError::InstructionError(
				0,
				pina_test::InstructionError::Custom(StakingError::NothingToClaim as u32)
			)),
			"a claim with nothing accrued must be refused with NothingToClaim"
		);
		assert_eq!(
			vault_amount(&program.account(&user_reward_ata).expect("fetch reward ATA")),
			claimed,
			"the refused claim moved no rewards"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// The confirmed fund drain is closed: a deposit from a wallet that owns no
/// stake tokens cannot credit a position.
///
/// Before deposit took custody, this exact sequence drained the reward vault
/// end-to-end: the attacker deposited `1_000_000_000_000` with an empty stake
/// ATA, the ledger credited the position from the argument alone, and one drip
/// plus one claim moved the entire vault to the attacker. Now the deposit
/// itself fails on the SPL transfer, the ledger never grows, and the claim has
/// nothing to release.
#[test]
#[ignore = "run with pina test"]
fn rejects_a_deposit_without_stake_tokens() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([2; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let admin = program.payer();
		let stake_mint =
			provision_mint(&program, &admin, &mint_authority, 3).expect("provision stake mint");
		let reward_mint =
			provision_mint(&program, &admin, &mint_authority, 4).expect("provision reward mint");

		let (pool, pool_bump) = pool_pda(&program_id, &stake_mint, &reward_mint);
		let stake_vault = ata_of(&pool, &stake_mint);
		let reward_vault = ata_of(&pool, &reward_mint);

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

		// Fill the reward vault: the payout the attacker is about to chase.
		mint_into(&program, &reward_mint, &reward_vault, &mint_authority, FUND)
			.expect("fund reward vault");

		// The attacker funds a wallet and opens a canonical position. Every step
		// of the original exploit up to the deposit is still available.
		let attacker = Keypair::new_from_array([78; 32]);
		program
			.fund(&attacker.pubkey(), FUND)
			.expect("fund attacker");
		let attacker_stake_ata = ata_of(&attacker.pubkey(), &stake_mint);
		let attacker_reward_ata = ata_of(&attacker.pubkey(), &reward_mint);
		let (position, position_bump) = position_pda(&program_id, &pool, &attacker.pubkey());
		program
			.send_with_signers(
				open_position_instruction(
					&program,
					&attacker.pubkey(),
					&pool,
					&position,
					position_bump,
				),
				&[&attacker],
			)
			.expect("execute OpenPosition for the attacker");

		// The exploit deposit: a stake ATA that exists nowhere and a zero token
		// balance, for a `staked_amount` ten orders of magnitude past anything
		// the attacker could fund. The custody transfer must fail it closed.
		let error = program
			.send_with_signers(
				deposit_instruction(
					&program,
					&attacker.pubkey(),
					&stake_mint,
					&pool,
					&position,
					&attacker_stake_ata,
					&stake_vault,
					1_000_000_000_000,
				),
				&[&attacker],
			)
			.expect_err("a deposit without stake tokens must fail");
		// Pin the reason: SPL Token's `InsufficientFunds` (custom code 1), raised
		// by the custody transfer — not an unrelated rejection.
		assert_eq!(
			error.transaction_error(),
			Some(pina_test::TransactionError::InstructionError(
				0,
				pina_test::InstructionError::Custom(1)
			)),
			"the deposit must fail on the SPL transfer (InsufficientFunds)"
		);

		// The ledger never grew, so there is no unbacked share weight anywhere.
		assert_position(
			&program.account(&position).expect("fetch attacker position"),
			&pool,
			&attacker.pubkey(),
			0,
			position_bump,
		);
		assert_stake_backing(&program, &stake_vault, &pool);

		// Even with the index dripped, the attacker's claim has nothing to
		// release and the reward vault keeps every token.
		program
			.send_instruction(set_reward_index_instruction(
				&program,
				&admin,
				&pool,
				&reward_mint,
				&token_program_id(),
				&reward_vault,
				1_000_000_000,
			))
			.expect("execute SetRewardIndex");
		let error = program
			.send_with_signers(
				claim_instruction(
					&program,
					&attacker.pubkey(),
					&reward_mint,
					&pool,
					&position,
					&attacker_reward_ata,
					&reward_vault,
				),
				&[&attacker],
			)
			.expect_err("a claim on an unbacked position is refused");
		pina_test::assert_custom_error(&error, StakingError::NothingToClaim as u32);
		assert_eq!(
			vault_amount(&program.account(&reward_vault).expect("fetch reward vault")),
			FUND,
			"the reward vault must be untouched"
		);
		assert!(
			program.account(&attacker_reward_ata).is_err(),
			"the attacker never receives a reward payout"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// The stake vault backs the ledger across multiple depositors and a partial
/// withdrawal.
///
/// Two positions deposit different amounts into one pool; after every step the
/// vault balance equals `pool.total_staked`, and a withdrawal moves the
/// principal back to the withdrawing depositor's ATA without breaking the
/// invariant for the staker who remains.
#[test]
#[ignore = "run with pina test"]
fn stake_vault_backs_total_staked() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([2; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let admin = program.payer();
		let stake_mint =
			provision_mint(&program, &admin, &mint_authority, 3).expect("provision stake mint");
		let reward_mint =
			provision_mint(&program, &admin, &mint_authority, 4).expect("provision reward mint");

		let (pool, pool_bump) = pool_pda(&program_id, &stake_mint, &reward_mint);
		let stake_vault = ata_of(&pool, &stake_mint);
		let reward_vault = ata_of(&pool, &reward_mint);

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

		let depositor_a = Keypair::new_from_array([79; 32]);
		let depositor_b = Keypair::new_from_array([80; 32]);
		program
			.fund(&depositor_a.pubkey(), FUND)
			.expect("fund depositor A");
		program
			.fund(&depositor_b.pubkey(), FUND)
			.expect("fund depositor B");

		let deposit_a = 1_000u64;
		let deposit_b = 700u64;
		let withdraw_b = 200u64;
		let mut total = 0u64;

		for (depositor, amount) in [(&depositor_a, deposit_a), (&depositor_b, deposit_b)] {
			let wallet = depositor.pubkey();
			let (position, position_bump) = position_pda(&program_id, &pool, &wallet);
			program
				.send_with_signers(
					open_position_instruction(&program, &wallet, &pool, &position, position_bump),
					&[depositor],
				)
				.expect("execute OpenPosition");
			fund_stake_ata(
				&program,
				&admin,
				&wallet,
				&stake_mint,
				&mint_authority,
				amount,
			)
			.expect("fund depositor's stake ATA");

			program
				.send_with_signers(
					deposit_instruction(
						&program,
						&wallet,
						&stake_mint,
						&pool,
						&position,
						&ata_of(&wallet, &stake_mint),
						&stake_vault,
						amount,
					),
					&[depositor],
				)
				.expect("execute Deposit");
			total += amount;

			assert_position(
				&program.account(&position).expect("fetch position"),
				&pool,
				&wallet,
				amount,
				position_bump,
			);
			assert_eq!(
				vault_amount(&program.account(&stake_vault).expect("fetch stake vault")),
				total,
				"the vault holds every staked token"
			);
			assert_stake_backing(&program, &stake_vault, &pool);
		}

		// A partial withdrawal returns principal without breaking the backing.
		let (position_b, position_b_bump) = position_pda(&program_id, &pool, &depositor_b.pubkey());
		program
			.send_with_signers(
				withdraw_instruction(
					&program,
					&depositor_b.pubkey(),
					&stake_mint,
					&pool,
					&position_b,
					&ata_of(&depositor_b.pubkey(), &stake_mint),
					&stake_vault,
					withdraw_b,
				),
				&[&depositor_b],
			)
			.expect("execute Withdraw");
		assert_position(
			&program.account(&position_b).expect("fetch position B"),
			&pool,
			&depositor_b.pubkey(),
			deposit_b - withdraw_b,
			position_b_bump,
		);
		assert_eq!(
			vault_amount(
				&program
					.account(&ata_of(&depositor_b.pubkey(), &stake_mint))
					.expect("fetch depositor B stake ATA")
			),
			withdraw_b,
			"the withdrawn principal returned to depositor B"
		);
		assert_eq!(
			pool_total_staked(&program.account(&pool).expect("fetch pool state")),
			total - withdraw_b,
			"the ledger released the withdrawn stake"
		);
		assert_stake_backing(&program, &stake_vault, &pool);

		program.stop().expect("stop isolated program test");
	});
}

// ---------------------------------------------------------------------------
// Audit regressions (2026-09-22 deep audit, re-verified 2026-09-23)
//
// Each test below asserts the *secure* behavior from the audit report. It
// fails on the current tree because the exploit is still live, and must pass
// once the corresponding fix lands. They are the acceptance tests for those
// fixes: run them with `pina test --project examples/staking_rewards_program
// --filter audit_sec_`.
// ---------------------------------------------------------------------------

/// SEC-26: the canonical pool PDA for a mint pair is a singleton, and
/// `InitializePool` stores whichever signer arrives first as the permanent
/// administrator. An unapproved first caller must not be able to occupy it.
///
/// Current behavior: any funded signer initializes the canonical pool and
/// becomes the stored administrator, so the first `expect_err` below fails
/// and the test proves the capture is live.
/// Dormant acceptance test for issue #502: the exploit this test proves is
/// deferred pending the initialization trust-model decision. Enable the
/// `audit-deferred` feature and run this test explicitly once the fix lands.
#[cfg(feature = "audit-deferred")]
#[test]
#[ignore = "deferred: issue #502"]
fn audit_sec_26_unapproved_first_initializer_cannot_capture_the_singleton_pool() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([2; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let intended_admin = program.payer();
		let attacker = Keypair::new_from_array([0xAD; 32]);
		program
			.fund(&attacker.pubkey(), FUND)
			.expect("fund attacker");

		let stake_mint = provision_mint(&program, &intended_admin, &mint_authority, 3)
			.expect("provision stake mint");
		let reward_mint = provision_mint(&program, &intended_admin, &mint_authority, 4)
			.expect("provision reward mint");

		let (pool, pool_bump) = pool_pda(&program_id, &stake_mint, &reward_mint);
		let stake_vault = ata_of(&pool, &stake_mint);
		let reward_vault = ata_of(&pool, &reward_mint);

		// The attacker races the intended administrator to the canonical pool.
		let error = program
			.send_with_signers(
				initialize_pool_instruction(
					&program,
					&attacker.pubkey(),
					&stake_mint,
					&reward_mint,
					&pool,
					&stake_vault,
					&reward_vault,
					pool_bump,
				),
				&[&attacker],
			)
			.expect_err("an unapproved signer must not capture the canonical pool");

		// The rejection must not leave any state behind: no pool, no vaults.
		assert!(
			program.account(&pool).is_err(),
			"authorization failure must not create pool state"
		);
		let message = error.message();
		assert!(
			message.contains("Unauthorized") || message.contains("custom program error"),
			"expected an authorization rejection, got: {message}"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// SEC-27 (reserves): `SetRewardIndex` accepts any monotone index without
/// consulting the reward vault or tracking aggregate liabilities. An update
/// that promises more rewards than the vault holds must be rejected before
/// the index moves.
///
/// Current behavior: the update is accepted, so the `expect_err` below fails
/// and the test proves unbacked liabilities are live.
#[test]
#[ignore = "run with pina test"]
fn audit_sec_27_set_reward_index_rejects_liabilities_beyond_reward_vault_reserves() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([2; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let admin = program.payer();
		let stake_mint =
			provision_mint(&program, &admin, &mint_authority, 3).expect("provision stake mint");
		let reward_mint =
			provision_mint(&program, &admin, &mint_authority, 4).expect("provision reward mint");

		let (pool, pool_bump) = pool_pda(&program_id, &stake_mint, &reward_mint);
		let stake_vault = ata_of(&pool, &stake_mint);
		let reward_vault = ata_of(&pool, &reward_mint);

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

		// Two depositors stake 10 each, so the ledger credits 20 staked.
		let staked: u64 = 10;
		for seed in [5u8, 6u8] {
			let depositor = Keypair::new_from_array([seed; 32]);
			program
				.fund(&depositor.pubkey(), FUND)
				.expect("fund depositor");
			let (position, position_bump) = position_pda(&program_id, &pool, &depositor.pubkey());
			program
				.send_with_signers(
					open_position_instruction(
						&program,
						&depositor.pubkey(),
						&pool,
						&position,
						position_bump,
					),
					&[&depositor],
				)
				.expect("execute OpenPosition");
			fund_stake_ata(
				&program,
				&admin,
				&depositor.pubkey(),
				&stake_mint,
				&mint_authority,
				staked,
			)
			.expect("fund stake ATA");
			program
				.send_with_signers(
					deposit_instruction(
						&program,
						&depositor.pubkey(),
						&stake_mint,
						&pool,
						&position,
						&ata_of(&depositor.pubkey(), &stake_mint),
						&stake_vault,
						staked,
					),
					&[&depositor],
				)
				.expect("execute Deposit");
		}
		assert_stake_backing(&program, &stake_vault, &pool);

		// Fund the reward vault with exactly ONE of the two promised payouts.
		mint_into(
			&program,
			&reward_mint,
			&reward_vault,
			&mint_authority,
			staked,
		)
		.expect("fund reward vault with half the liability");

		// One full index unit promises one reward token per staked token:
		// 20 owed against a 10-token vault. The update must be rejected.
		let error = program
			.send_instruction(set_reward_index_instruction(
				&program,
				&admin,
				&pool,
				&reward_mint,
				&token_program_id(),
				&reward_vault,
				REWARD_INDEX_SCALE,
			))
			.expect_err("an index update beyond the vault's reserves must be rejected");

		// The rejected update must leave the index unchanged at zero.
		let pool_account = program.account(&pool).expect("fetch pool state");
		assert_eq!(
			u64::from_le_bytes(
				pool_account.data[106..114]
					.try_into()
					.expect("reward index")
			),
			0,
			"a rejected index update must not move the index: {}",
			error.message()
		);

		program.stop().expect("stop isolated program test");
	});
}

/// SEC-27 (claim order): when an index update is accepted, equal entitlements
/// must not depend on the order in which users claim. With the current
/// underfunded index accepted, the first claim drains the vault and the
/// second fails, so the second claim below fails and the test proves the
/// order dependence.
///
/// After the fix this test passes either because the underfunded update is
/// rejected (the branch returns early) or because reserves always cover the
/// accepted liability and both claims succeed.
#[test]
#[ignore = "run with pina test"]
fn audit_sec_27_equal_entitlements_do_not_depend_on_claim_order() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([2; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let admin = program.payer();
		let stake_mint =
			provision_mint(&program, &admin, &mint_authority, 3).expect("provision stake mint");
		let reward_mint =
			provision_mint(&program, &admin, &mint_authority, 4).expect("provision reward mint");

		let (pool, pool_bump) = pool_pda(&program_id, &stake_mint, &reward_mint);
		let stake_vault = ata_of(&pool, &stake_mint);
		let reward_vault = ata_of(&pool, &reward_mint);

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

		let staked: u64 = 10;
		let mut positions = Vec::new();
		for seed in [5u8, 6u8] {
			let depositor = Keypair::new_from_array([seed; 32]);
			program
				.fund(&depositor.pubkey(), FUND)
				.expect("fund depositor");
			let (position, position_bump) = position_pda(&program_id, &pool, &depositor.pubkey());
			program
				.send_with_signers(
					open_position_instruction(
						&program,
						&depositor.pubkey(),
						&pool,
						&position,
						position_bump,
					),
					&[&depositor],
				)
				.expect("execute OpenPosition");
			fund_stake_ata(
				&program,
				&admin,
				&depositor.pubkey(),
				&stake_mint,
				&mint_authority,
				staked,
			)
			.expect("fund stake ATA");
			program
				.send_with_signers(
					deposit_instruction(
						&program,
						&depositor.pubkey(),
						&stake_mint,
						&pool,
						&position,
						&ata_of(&depositor.pubkey(), &stake_mint),
						&stake_vault,
						staked,
					),
					&[&depositor],
				)
				.expect("execute Deposit");
			positions.push((depositor, position));
		}

		mint_into(
			&program,
			&reward_mint,
			&reward_vault,
			&mint_authority,
			staked,
		)
		.expect("fund reward vault with half the liability");

		// If the underfunded update is rejected — the SEC-27 fix — equal
		// treatment is upheld and this test has nothing left to prove.
		if let Err(error) = program.send_instruction(set_reward_index_instruction(
			&program,
			&admin,
			&pool,
			&reward_mint,
			&token_program_id(),
			&reward_vault,
			REWARD_INDEX_SCALE,
		)) {
			assert!(
				error.transaction_error().is_some(),
				"harness failure before the program could reject the update: {error:?}"
			);
			program.stop().expect("stop isolated program test");
			return;
		}

		// The update was accepted, so both equal entitlements must be payable,
		// regardless of who claims first.
		for (depositor, position) in &positions {
			program
				.send_with_signers(
					claim_instruction(
						&program,
						&depositor.pubkey(),
						&reward_mint,
						&pool,
						position,
						&ata_of(&depositor.pubkey(), &reward_mint),
						&reward_vault,
					),
					&[depositor],
				)
				.unwrap_or_else(|error| {
					panic!(
						"an accepted index must honor every equal entitlement: {}",
						error.message()
					)
				});
			let received = vault_amount(
				&program
					.account(&ata_of(&depositor.pubkey(), &reward_mint))
					.expect("fetch reward ATA"),
			);
			assert_eq!(
				received, staked,
				"each position must receive its full entitlement"
			);
		}

		program.stop().expect("stop isolated program test");
	});
}

/// SEC-27 (unrepresentable liabilities): a large accepted index makes the
/// per-position accrual unrepresentable in `u64`, after which claim, deposit,
/// and withdraw all fail with `ArithmeticOverflow` and the position is
/// frozen. The index update that creates an unrepresentable obligation must
/// be rejected before state moves.
///
/// Current behavior: `u64::MAX` is accepted and the subsequent withdrawal
/// fails with `Program arithmetic overflowed`, so the `expect` below fails
/// and the test proves the freeze.
#[test]
#[ignore = "run with pina test"]
fn audit_sec_27_an_unrepresentable_index_must_not_freeze_existing_positions() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([2; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let admin = program.payer();
		let stake_mint =
			provision_mint(&program, &admin, &mint_authority, 3).expect("provision stake mint");
		let reward_mint =
			provision_mint(&program, &admin, &mint_authority, 4).expect("provision reward mint");

		let (pool, pool_bump) = pool_pda(&program_id, &stake_mint, &reward_mint);
		let stake_vault = ata_of(&pool, &stake_mint);
		let reward_vault = ata_of(&pool, &reward_mint);

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
		// Two trillion base units (two million tokens at six decimals): large
		// enough that `staked * u64::MAX / SCALE` no longer fits a `u64` payout.
		let staked = 2_000_000_000_000u64;
		fund_stake_ata(
			&program,
			&admin,
			&admin,
			&stake_mint,
			&mint_authority,
			staked,
		)
		.expect("fund stake ATA");
		program
			.send_instruction(deposit_instruction(
				&program,
				&admin,
				&stake_mint,
				&pool,
				&position,
				&ata_of(&admin, &stake_mint),
				&stake_vault,
				staked,
			))
			.expect("execute Deposit");

		// `u64::MAX` scaled units over the staked balance cannot be represented
		// as a `u64` payout. Whatever the reserves say, this update must not be
		// accepted: it would freeze the position above.
		if let Err(error) = program.send_instruction(set_reward_index_instruction(
			&program,
			&admin,
			&pool,
			&reward_mint,
			&token_program_id(),
			&reward_vault,
			u64::MAX,
		)) {
			assert!(
				error.transaction_error().is_some(),
				"harness failure before the program could reject the update: {error:?}"
			);
			// Rejected: the SEC-27 fix holds, nothing left to prove.
			program.stop().expect("stop isolated program test");
			return;
		}

		// Accepted today: the staked principal can no longer leave the pool,
		// because every checkpoint that re-computes the accrual overflows.
		// Demanding the withdrawal succeed turns the observed freeze into the
		// test failure that proves the exploit.
		program
			.send_instruction(withdraw_instruction(
				&program,
				&admin,
				&stake_mint,
				&pool,
				&position,
				&ata_of(&admin, &stake_mint),
				&stake_vault,
				staked,
			))
			.expect("the staked principal must remain withdrawable after any accepted index");

		program.stop().expect("stop isolated program test");
	});
}

/// The outstanding-liability counter must fall when a claim pays out: rewards
/// that have left the vault are no longer a promise the vault must cover, so a
/// later index advance is accepted against the smaller remaining liability
/// even though the vault only holds the outstanding remainder. Before the fix,
/// SetRewardIndex re-derived the full liability from genesis on every update
/// (`total_staked * new_index / SCALE`), so this advance was rejected against
/// rewards that had already been paid and banked stake that had already left.
#[test]
#[ignore = "run with pina test"]
fn claims_reduce_the_liability_the_vault_must_back() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([2; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let admin = program.payer();
		let stake_mint =
			provision_mint(&program, &admin, &mint_authority, 3).expect("provision stake mint");
		let reward_mint =
			provision_mint(&program, &admin, &mint_authority, 4).expect("provision reward mint");

		let (pool, pool_bump) = pool_pda(&program_id, &stake_mint, &reward_mint);
		let stake_vault = ata_of(&pool, &stake_mint);
		let reward_vault = ata_of(&pool, &reward_mint);
		let user_stake_ata = ata_of(&admin, &stake_mint);
		let user_reward_ata = ata_of(&admin, &reward_mint);
		let (position, position_bump) = position_pda(&program_id, &pool, &admin);

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
		program
			.send_instruction(open_position_instruction(
				&program,
				&admin,
				&pool,
				&position,
				position_bump,
			))
			.expect("execute OpenPosition");

		let staked = 1_000u64;
		fund_stake_ata(
			&program,
			&admin,
			&admin,
			&stake_mint,
			&mint_authority,
			staked,
		)
		.expect("fund stake ATA");
		program
			.send_instruction(deposit_instruction(
				&program,
				&admin,
				&stake_mint,
				&pool,
				&position,
				&user_stake_ata,
				&stake_vault,
				staked,
			))
			.expect("execute Deposit");

		// One full drip is owed on the staked balance, so fund the vault with
		// exactly that payout.
		mint_into(
			&program,
			&reward_mint,
			&reward_vault,
			&mint_authority,
			staked,
		)
		.expect("fund the first drip");
		program
			.send_instruction(set_reward_index_instruction(
				&program,
				&admin,
				&pool,
				&reward_mint,
				&token_program_id(),
				&reward_vault,
				REWARD_INDEX_SCALE,
			))
			.expect("execute the first SetRewardIndex");

		// The claim pays the accrued rewards out of the vault, so the vault is
		// now empty and the liability must have fallen with it — to zero.
		program
			.send_instruction(claim_instruction(
				&program,
				&admin,
				&reward_mint,
				&pool,
				&position,
				&user_reward_ata,
				&reward_vault,
			))
			.expect("execute Claim");
		assert_eq!(
			u64::from_le_bytes(
				program.account(&pool).expect("fetch pool state").data[114..122]
					.try_into()
					.expect("outstanding rewards")
			),
			0,
			"a paid-out claim must leave no outstanding liability"
		);

		// A second drip needs only the new increment covered. Before the fix,
		// this was rejected: the gate re-derived `staked * new_index / SCALE`
		// from genesis against an emptied vault.
		mint_into(
			&program,
			&reward_mint,
			&reward_vault,
			&mint_authority,
			staked,
		)
		.expect("fund the second drip");
		program
			.send_instruction(set_reward_index_instruction(
				&program,
				&admin,
				&pool,
				&reward_mint,
				&token_program_id(),
				&reward_vault,
				2 * REWARD_INDEX_SCALE,
			))
			.expect("an index advance after a claim must cover only the new increment");

		program.stop().expect("stop isolated program test");
	});
}

/// Banked rewards keep counting toward the outstanding liability until they
/// are claimed: a withdrawal checkpoints a position's earned rewards into
/// `pending_rewards`, and the vault must still cover them after the position's
/// stake has left the pool. Before the fix, SetRewardIndex measured the
/// liability as `total_staked * index`, so a withdrawal after a drip silently
/// dropped the banked rewards from what the gate demanded the vault cover.
#[test]
#[ignore = "run with pina test"]
fn banked_pending_rewards_stay_reserved_until_claimed() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([2; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let admin = program.payer();
		let stake_mint =
			provision_mint(&program, &admin, &mint_authority, 3).expect("provision stake mint");
		let reward_mint =
			provision_mint(&program, &admin, &mint_authority, 4).expect("provision reward mint");

		let (pool, pool_bump) = pool_pda(&program_id, &stake_mint, &reward_mint);
		let stake_vault = ata_of(&pool, &stake_mint);
		let reward_vault = ata_of(&pool, &reward_mint);
		let user_stake_ata = ata_of(&admin, &stake_mint);
		let user_reward_ata = ata_of(&admin, &reward_mint);
		let (position, position_bump) = position_pda(&program_id, &pool, &admin);

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
		program
			.send_instruction(open_position_instruction(
				&program,
				&admin,
				&pool,
				&position,
				position_bump,
			))
			.expect("execute OpenPosition");

		let staked = 1_000u64;
		fund_stake_ata(
			&program,
			&admin,
			&admin,
			&stake_mint,
			&mint_authority,
			staked,
		)
		.expect("fund stake ATA");
		program
			.send_instruction(deposit_instruction(
				&program,
				&admin,
				&stake_mint,
				&pool,
				&position,
				&user_stake_ata,
				&stake_vault,
				staked,
			))
			.expect("execute Deposit");

		// One full drip over the staked balance, fully funded.
		mint_into(
			&program,
			&reward_mint,
			&reward_vault,
			&mint_authority,
			staked,
		)
		.expect("fund the drip");
		program
			.send_instruction(set_reward_index_instruction(
				&program,
				&admin,
				&pool,
				&reward_mint,
				&token_program_id(),
				&reward_vault,
				REWARD_INDEX_SCALE,
			))
			.expect("execute SetRewardIndex");

		// Withdraw the full stake: the accrual over the staked balance is
		// banked into `pending_rewards` before the stake leaves the pool.
		program
			.send_instruction(withdraw_instruction(
				&program,
				&admin,
				&stake_mint,
				&pool,
				&position,
				&user_stake_ata,
				&stake_vault,
				staked,
			))
			.expect("execute Withdraw");

		// The banked rewards are still owed, so the outstanding liability must
		// have survived the withdrawal.
		assert_eq!(
			u64::from_le_bytes(
				program.account(&pool).expect("fetch pool state").data[114..122]
					.try_into()
					.expect("outstanding rewards")
			),
			staked,
			"banked pending rewards must stay reserved after the stake leaves"
		);

		// The claim still pays the banked rewards from the vault.
		program
			.send_instruction(claim_instruction(
				&program,
				&admin,
				&reward_mint,
				&pool,
				&position,
				&user_reward_ata,
				&reward_vault,
			))
			.expect("the banked rewards must remain payable after the withdrawal");
		assert_eq!(
			u64::from_le_bytes(
				program.account(&pool).expect("fetch pool state").data[114..122]
					.try_into()
					.expect("outstanding rewards")
			),
			0,
			"the liability must fall to zero once the banked rewards are paid"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// The reserve gate must bind the vault to the pool's own stored reward mint:
/// a caller naming any other mint gets that mint's pool-owned ATA, and without
/// the binding check anyone could create a worthless mint, fund its pool ATA
/// to any balance, and move the reward index with no real reward tokens
/// behind it. The update must be refused with `InvalidPool` before the vault
/// balance is read.
#[test]
#[ignore = "run with pina test"]
fn set_reward_index_rejects_a_foreign_reward_mint() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([2; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let admin = program.payer();
		let stake_mint =
			provision_mint(&program, &admin, &mint_authority, 3).expect("provision stake mint");
		let reward_mint =
			provision_mint(&program, &admin, &mint_authority, 4).expect("provision reward mint");

		let (pool, pool_bump) = pool_pda(&program_id, &stake_mint, &reward_mint);
		let stake_vault = ata_of(&pool, &stake_mint);
		let reward_vault = ata_of(&pool, &reward_mint);

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

		// An attacker's mint with its own pool-owned ATA, funded far beyond
		// the real reward vault: without the stored-mint binding this balance
		// would satisfy the reserve gate.
		let foreign_mint =
			provision_mint(&program, &admin, &mint_authority, 7).expect("provision foreign mint");
		let foreign_vault = ata_of(&pool, &foreign_mint);
		fund_stake_ata(
			&program,
			&admin,
			&pool,
			&foreign_mint,
			&mint_authority,
			1_000_000,
		)
		.expect("fund the foreign mint's pool ATA");

		let error = program
			.send_instruction(set_reward_index_instruction(
				&program,
				&admin,
				&pool,
				&foreign_mint,
				&token_program_id(),
				&foreign_vault,
				REWARD_INDEX_SCALE,
			))
			.expect_err("a foreign reward mint must not satisfy the reserve gate");
		pina_test::assert_custom_error(&error, StakingError::InvalidPool as u32);

		// The rejected update moved neither the index nor the liability.
		let pool_account = program.account(&pool).expect("fetch pool state");
		assert_eq!(
			u64::from_le_bytes(
				pool_account.data[106..114]
					.try_into()
					.expect("reward index")
			),
			0,
			"the rejected update must not move the index"
		);
		assert_eq!(
			u64::from_le_bytes(
				pool_account.data[114..122]
					.try_into()
					.expect("outstanding rewards")
			),
			0,
			"the rejected update must not move the liability"
		);

		program.stop().expect("stop isolated program test");
	});
}
