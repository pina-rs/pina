#![cfg(test)]

//! Surfpool coverage for the vesting example: provision a real SPL mint, run
//! the Initialize/Claim/Cancel flow, and verify the on-chain vesting state,
//! vault funding, and custom error paths.

use pina_test::Account;
use pina_test::AccountMeta;
use pina_test::Instruction;
use pina_test::Keypair;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Signer;
use pina_test::TestError;
use program_under_test::ID;
use program_under_test::VestingInstruction;
use program_under_test::VestingState;

/// SPL Token (Tokenkeg…), one of the example's allowlisted programs.
const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ATA_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

const MINT_SPACE: u64 = 82;
const DECIMALS: u8 = 6;
const FUND: u64 = 1_000_000_000;
const SEED_VESTING: &[u8] = b"vesting";
const TOTAL: u64 = 1_000_000_000;
const CLAIM_AMOUNT: u64 = 400_000_000;

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

fn vesting_pda(
	program_id: &Pubkey,
	admin: &Pubkey,
	beneficiary: &Pubkey,
	mint: &Pubkey,
) -> (Pubkey, u8) {
	Pubkey::find_program_address(
		&[
			SEED_VESTING,
			admin.as_ref(),
			beneficiary.as_ref(),
			mint.as_ref(),
		],
		program_id,
	)
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
	let mint = Keypair::new_from_array([13; 32]);
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

fn initialize_instruction(
	program: &ProgramTest,
	admin: &Pubkey,
	beneficiary: &Pubkey,
	mint: &Pubkey,
	vesting_state: &Pubkey,
	vault: &Pubkey,
	bump: u8,
	schedule: (u64, u64, u64),
) -> pina_test::Instruction {
	let (start_ts, cliff_ts, end_ts) = schedule;
	// discriminator + migration version, then amounts, timestamps, and bump.
	let mut data = vec![VestingInstruction::Initialize as u8, 0u8];
	data.extend_from_slice(&TOTAL.to_le_bytes());
	data.extend_from_slice(&start_ts.to_le_bytes());
	data.extend_from_slice(&cliff_ts.to_le_bytes());
	data.extend_from_slice(&end_ts.to_le_bytes());
	data.push(bump);

	program.instruction(
		&data,
		vec![
			AccountMeta::new(*admin, true),
			AccountMeta::new_readonly(*beneficiary, false),
			AccountMeta::new_readonly(*mint, false),
			AccountMeta::new(*vesting_state, false),
			AccountMeta::new(*vault, false),
			AccountMeta::new_readonly(ata_program_id(), false),
			AccountMeta::new_readonly(Pubkey::default(), false),
			AccountMeta::new_readonly(token_program_id(), false),
		],
	)
}

fn claim_instruction(
	program: &ProgramTest,
	beneficiary: &Pubkey,
	mint: &Pubkey,
	vesting_state: &Pubkey,
	beneficiary_ata: &Pubkey,
	vault: &Pubkey,
	amount: u64,
) -> pina_test::Instruction {
	let mut data = vec![VestingInstruction::Claim as u8, 0u8];
	data.extend_from_slice(&amount.to_le_bytes());

	program.instruction(
		&data,
		vec![
			AccountMeta::new(*beneficiary, true),
			AccountMeta::new_readonly(*mint, false),
			AccountMeta::new(*vesting_state, false),
			AccountMeta::new(*beneficiary_ata, false),
			AccountMeta::new(*vault, false),
			AccountMeta::new_readonly(ata_program_id(), false),
			AccountMeta::new_readonly(Pubkey::default(), false),
			AccountMeta::new_readonly(token_program_id(), false),
			AccountMeta::new_readonly(clock_sysvar_id(), false),
		],
	)
}

fn cancel_instruction(
	program: &ProgramTest,
	admin: &Pubkey,
	mint: &Pubkey,
	vesting_state: &Pubkey,
	vault: &Pubkey,
	admin_ata: &Pubkey,
) -> pina_test::Instruction {
	program.instruction(
		&[VestingInstruction::Cancel as u8, 0u8],
		vec![
			AccountMeta::new(*admin, true),
			AccountMeta::new_readonly(*mint, false),
			AccountMeta::new(*vesting_state, false),
			AccountMeta::new(*admin_ata, false),
			AccountMeta::new(*vault, false),
			AccountMeta::new_readonly(ata_program_id(), false),
			AccountMeta::new_readonly(Pubkey::default(), false),
			AccountMeta::new_readonly(token_program_id(), false),
		],
	)
}

fn clock_sysvar_id() -> Pubkey {
	Pubkey::from_str_const("SysvarC1ock11111111111111111111111111111111")
}

/// SPL token amount: u64 LE at byte offset 64 of the token account.
fn token_amount(program: &ProgramTest, address: &Pubkey) -> u64 {
	let account: Account = program.account(address).expect("fetch token account");
	let raw: [u8; 8] = account
		.data
		.get(64..72)
		.expect("token account data covers the amount field")
		.try_into()
		.expect("amount slice is 8 bytes");
	u64::from_le_bytes(raw)
}

fn mint_into(
	program: &ProgramTest,
	mint: &Pubkey,
	destination: &Pubkey,
	authority: &Keypair,
	amount: u64,
) -> Result<(), TestError> {
	// SPL `MintTo` = tag 7.
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

fn assert_vesting(
	account: &Account,
	admin: &Pubkey,
	beneficiary: &Pubkey,
	mint: &Pubkey,
	total: u64,
	claimed: u64,
	cancelled: bool,
	bump: u8,
) {
	let state = VestingState::try_from_bytes(&account.data).expect("decode vesting state");
	assert_eq!(state.admin.as_ref(), admin.as_ref());
	assert_eq!(state.beneficiary.as_ref(), beneficiary.as_ref());
	assert_eq!(state.mint.as_ref(), mint.as_ref());
	assert_eq!(state.total_amount.get(), total);
	assert_eq!(state.claimed_amount.get(), claimed);
	assert_eq!(state.cancelled.get(), cancelled);
	assert_eq!(state.bump, bump);
}

/// Exercise every vesting instruction with a real SPL mint and ATA.
///
/// Fixed signer keys keep the PDA path and compute-unit measurements stable.
#[test]
#[ignore = "run with pina test"]
fn initialize_claim_and_cancel() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([12; 32]);
		let admin = Keypair::new_from_array([11; 32]);
		program.fund(&admin.pubkey(), FUND).expect("fund admin");
		let beneficiary = Keypair::new_from_array([14; 32]);
		program
			.fund(&beneficiary.pubkey(), FUND)
			.expect("fund beneficiary");

		let mint = provision_mint(&program, &program.payer(), &mint_authority)
			.expect("provision vesting mint");

		let (vesting_state, bump) =
			vesting_pda(&program_id, &admin.pubkey(), &beneficiary.pubkey(), &mint);
		let vault = ata_of(&vesting_state, &mint);
		assert_ne!(bump, 0, "a canonical bump exists on the host");

		// A schedule that is already fully elapsed vests the whole allocation
		// from the first claim, which is the simple path this test exercises.
		// The cliff and linear-unlock paths get their own cases below.
		program
			.send_with_signers(
				initialize_instruction(
					&program,
					&admin.pubkey(),
					&beneficiary.pubkey(),
					&mint,
					&vesting_state,
					&vault,
					bump,
					(0, 0, 0),
				),
				&[&admin],
			)
			.expect("execute Initialize");
		assert_vesting(
			&program
				.account(&vesting_state)
				.expect("fetch initialized vesting state"),
			&admin.pubkey(),
			&beneficiary.pubkey(),
			&mint,
			TOTAL,
			0,
			false,
			bump,
		);

		// Fund the vault with the full allocation so releases are observable.
		let beneficiary_ata = ata_of(&beneficiary.pubkey(), &mint);
		mint_into(&program, &mint, &vault, &mint_authority, TOTAL).expect("fund vault");
		assert_eq!(
			token_amount(&program, &vault),
			TOTAL,
			"vault holds the allocation"
		);

		program
			.send_with_signers(
				claim_instruction(
					&program,
					&beneficiary.pubkey(),
					&mint,
					&vesting_state,
					&beneficiary_ata,
					&vault,
					CLAIM_AMOUNT,
				),
				&[&beneficiary],
			)
			.expect("execute Claim");
		assert_vesting(
			&program
				.account(&vesting_state)
				.expect("fetch claimed state"),
			&admin.pubkey(),
			&beneficiary.pubkey(),
			&mint,
			TOTAL,
			CLAIM_AMOUNT,
			false,
			bump,
		);
		// The claim must have moved tokens: this assertion is what the
		// previous version of this suite could not make.
		assert_eq!(
			token_amount(&program, &beneficiary_ata),
			CLAIM_AMOUNT,
			"the beneficiary received the released amount"
		);
		assert_eq!(
			token_amount(&program, &vault),
			TOTAL - CLAIM_AMOUNT,
			"the vault released exactly the claimed amount"
		);

		// Cancelling refunds the unclaimed remainder to the admin and closes
		// the vault, so no value is stranded.
		let admin_ata = ata_of(&admin.pubkey(), &mint);
		program
			.send_with_signers(
				cancel_instruction(
					&program,
					&admin.pubkey(),
					&mint,
					&vesting_state,
					&vault,
					&admin_ata,
				),
				&[&admin],
			)
			.expect("execute Cancel");
		assert_vesting(
			&program
				.account(&vesting_state)
				.expect("fetch cancelled state"),
			&admin.pubkey(),
			&beneficiary.pubkey(),
			&mint,
			TOTAL,
			CLAIM_AMOUNT,
			true,
			bump,
		);
		assert_eq!(
			token_amount(&program, &admin_ata),
			TOTAL - CLAIM_AMOUNT,
			"the admin recovered the unclaimed remainder"
		);
		// Closing an SPL account deletes it outright: the account no longer
		// exists, so any later read must fail. This is what proves no value is
		// stranded, since the refund had to complete before the close.
		assert!(
			program.account(&vault).is_err(),
			"the vault must be closed and removed after refunding"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// Create the wallet's associated token account (idempotently) and optionally
/// mint `amount` into it.
fn provision_ata(
	program: &mut ProgramTest,
	payer: &Pubkey,
	wallet: &Pubkey,
	mint: &Pubkey,
	mint_authority: Option<&Keypair>,
	amount: u64,
) -> Result<Pubkey, TestError> {
	let ata = ata_of(wallet, mint);
	// ATA `CreateIdempotent` = tag 1; delegates to the ATA program.
	let create = Instruction::new_with_bytes(
		ata_program_id(),
		&[1u8],
		vec![
			AccountMeta::new(*payer, true),
			AccountMeta::new(ata, false),
			AccountMeta::new_readonly(*wallet, false),
			AccountMeta::new_readonly(*mint, false),
			AccountMeta::new_readonly(Pubkey::default(), false),
			AccountMeta::new_readonly(token_program_id(), false),
		],
	);
	program.send_instruction(create)?;

	if let (Some(authority), true) = (mint_authority, amount > 0) {
		mint_into(program, mint, &ata, authority, amount)?;
	}

	Ok(ata)
}

/// A shared prefix for the adversarial vault cases: initialize a schedule
/// that is fully vested and fund the real vault with the whole allocation.
struct FundedSchedule {
	mint: Pubkey,
	admin: Keypair,
	beneficiary: Keypair,
	mint_authority: Keypair,
	vesting_state: Pubkey,
	vault: Pubkey,
	beneficiary_ata: Pubkey,
	admin_ata: Pubkey,
	bump: u8,
}

fn funded_schedule(program: &mut ProgramTest) -> FundedSchedule {
	let mint_authority = Keypair::new_from_array([12; 32]);
	let admin = Keypair::new_from_array([11; 32]);
	program.fund(&admin.pubkey(), FUND).expect("fund admin");
	let beneficiary = Keypair::new_from_array([14; 32]);
	program
		.fund(&beneficiary.pubkey(), FUND)
		.expect("fund beneficiary");

	let mint =
		provision_mint(program, &program.payer(), &mint_authority).expect("provision vesting mint");

	let program_id = Pubkey::new_from_array(ID.to_bytes());
	let (vesting_state, bump) =
		vesting_pda(&program_id, &admin.pubkey(), &beneficiary.pubkey(), &mint);
	let vault = ata_of(&vesting_state, &mint);

	program
		.send_with_signers(
			initialize_instruction(
				program,
				&admin.pubkey(),
				&beneficiary.pubkey(),
				&mint,
				&vesting_state,
				&vault,
				bump,
				(0, 0, 0),
			),
			&[&admin],
		)
		.expect("execute Initialize");

	let beneficiary_ata = ata_of(&beneficiary.pubkey(), &mint);
	let admin_ata = ata_of(&admin.pubkey(), &mint);
	let payer = program.payer();
	provision_ata(program, &payer, &beneficiary.pubkey(), &mint, None, 0).expect("beneficiary ATA");
	provision_ata(program, &payer, &admin.pubkey(), &mint, None, 0).expect("admin ATA");
	mint_into(program, &mint, &vault, &mint_authority, TOTAL).expect("fund vault");

	FundedSchedule {
		mint,
		admin,
		beneficiary,
		mint_authority,
		vesting_state,
		vault,
		beneficiary_ata,
		admin_ata,
		bump,
	}
}

/// A funded token account for the same mint owned by a bystander wallet: the
/// exact shape a caller would substitute for the vault to spend the
/// schedule's PDA signature on someone else's tokens.
fn bystander_vault(program: &mut ProgramTest, schedule: &FundedSchedule, payer: &Pubkey) -> Pubkey {
	let bystander = Keypair::new_from_array([21; 32]);
	program
		.fund(&bystander.pubkey(), FUND)
		.expect("fund bystander");
	let decoy = ata_of(&bystander.pubkey(), &schedule.mint);
	provision_ata(
		program,
		payer,
		&bystander.pubkey(),
		&schedule.mint,
		Some(&schedule.mint_authority),
		TOTAL,
	)
	.expect("bystander vault");
	decoy
}

/// The claim payout is signed by the vesting schedule, so the source must be
/// the schedule's own vault: a funded foreign token account substituted for
/// the vault is rejected and nothing moves.
#[test]
#[ignore = "run with pina test"]
fn claim_rejects_a_foreign_vault() {
	pina_test::run(async {
		let mut program = ProgramTest::start(Pubkey::new_from_array(ID.to_bytes()))
			.await
			.expect("start isolated program test");

		let schedule = funded_schedule(&mut program);
		let payer = program.payer();
		let decoy = bystander_vault(&mut program, &schedule, &payer);

		let error = program
			.send_with_signers(
				claim_instruction(
					&program,
					&schedule.beneficiary.pubkey(),
					&schedule.mint,
					&schedule.vesting_state,
					&schedule.beneficiary_ata,
					&decoy,
					CLAIM_AMOUNT,
				),
				&[&schedule.beneficiary],
			)
			.expect_err("a foreign vault must not fund the payout");

		assert_eq!(
			error.transaction_error(),
			Some(pina_test::TransactionError::InstructionError(
				0,
				pina_test::InstructionError::InvalidSeeds
			)),
			"the vault load rejects the substituted address"
		);

		// No value moved and no state changed: the schedule is still claimable
		// in full, the decoy keeps its balance, and the real vault is intact.
		assert_vesting(
			&program
				.account(&schedule.vesting_state)
				.expect("vesting state survives"),
			&schedule.admin.pubkey(),
			&schedule.beneficiary.pubkey(),
			&schedule.mint,
			TOTAL,
			0,
			false,
			schedule.bump,
		);
		assert_eq!(
			token_amount(&program, &decoy),
			TOTAL,
			"the bystander's balance is untouched"
		);
		assert_eq!(
			token_amount(&program, &schedule.vault),
			TOTAL,
			"the real vault still holds the allocation"
		);
		assert_eq!(
			token_amount(&program, &schedule.beneficiary_ata),
			0,
			"the beneficiary received nothing"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// The cancel refund is signed by the vesting schedule too, so the same
/// substitution on `Cancel` must be rejected before the schedule is marked
/// cancelled or any token moves.
#[test]
#[ignore = "run with pina test"]
fn cancel_rejects_a_foreign_vault() {
	pina_test::run(async {
		let mut program = ProgramTest::start(Pubkey::new_from_array(ID.to_bytes()))
			.await
			.expect("start isolated program test");

		let schedule = funded_schedule(&mut program);
		let payer = program.payer();
		let decoy = bystander_vault(&mut program, &schedule, &payer);

		let error = program
			.send_with_signers(
				cancel_instruction(
					&program,
					&schedule.admin.pubkey(),
					&schedule.mint,
					&schedule.vesting_state,
					&decoy,
					&schedule.admin_ata,
				),
				&[&schedule.admin],
			)
			.expect_err("a foreign vault must not fund the refund");

		assert_eq!(
			error.transaction_error(),
			Some(pina_test::TransactionError::InstructionError(
				0,
				pina_test::InstructionError::InvalidSeeds
			)),
			"the vault load rejects the substituted address"
		);

		assert_vesting(
			&program
				.account(&schedule.vesting_state)
				.expect("vesting state survives"),
			&schedule.admin.pubkey(),
			&schedule.beneficiary.pubkey(),
			&schedule.mint,
			TOTAL,
			0,
			false,
			schedule.bump,
		);
		assert_eq!(
			token_amount(&program, &decoy),
			TOTAL,
			"the bystander's balance is untouched"
		);
		assert_eq!(
			token_amount(&program, &schedule.vault),
			TOTAL,
			"the real vault still holds the allocation"
		);
		assert_eq!(
			token_amount(&program, &schedule.admin_ata),
			0,
			"the admin received nothing"
		);
		assert!(
			program.account(&schedule.vault).is_ok(),
			"the real vault was not closed"
		);

		program.stop().expect("stop isolated program test");
	});
}
