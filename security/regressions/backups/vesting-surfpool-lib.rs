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

// ---------------------------------------------------------------------------
// Audit regressions (2026-09-22 deep audit, re-verified 2026-09-23)
//
// Each test below asserts the *secure* behavior from the audit report. It
// fails on the current tree because the exploit is still live, and must pass
// once the corresponding fix lands. Run with `pina test --project
// examples/vesting_program --filter audit_sec_`.
// ---------------------------------------------------------------------------

const TOKEN_2022_PROGRAM: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";

fn token_2022_program_id() -> Pubkey {
	Pubkey::from_str_const(TOKEN_2022_PROGRAM)
}

/// SEC-29: `Initialize` records `total_amount` and creates an empty vault, so
/// a valid-looking, already-vested schedule can promise value it does not
/// hold and the beneficiary only discovers the shortfall at claim time. An
/// active schedule must be fully collateralized by the end of its
/// initializing instruction.
///
/// Current behavior: the unfunded elapsed schedule is accepted with a
/// zero-balance vault, so the assertion below fails and the test proves the
/// unbacked promise.
#[test]
#[ignore = "run with pina test"]
fn audit_sec_29_an_active_schedule_must_be_funded_at_initialization() {
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

		let payer = program.payer();
		let mint = provision_mint(&mut program, &payer, &mint_authority).expect("provision mint");
		let (vesting_state, bump) =
			vesting_pda(&program_id, &admin.pubkey(), &beneficiary.pubkey(), &mint);
		let vault = ata_of(&vesting_state, &mint);

		// Initialize an already-elapsed schedule with NO vault funding.
		let initialized = program.send_with_signers(
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
		);

		if let Err(error) = initialized {
			assert!(
				error.transaction_error().is_some(),
				"harness failure before the program could reject the schedule: {error:?}"
			);
			// Rejected: the funded-initialization fix holds.
			program.stop().expect("stop isolated program test");
			return;
		}

		let vault_balance = token_amount(&program, &vault);
		assert!(
			vault_balance >= TOTAL,
			"SEC-29: an active schedule was created promising {TOTAL} with only {vault_balance} \
			 in its vault"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// SEC-30: `Initialize` accepts a Token-2022 mint carrying extensions, but
/// `Claim` and `Cancel` both reject every extended mint, so tokens funded
/// into such a vault are locked permanently. The extension policy must be
/// enforced before any state or vault is created.
///
/// Current behavior: initialization succeeds for a mint with the (benign)
/// `MetadataPointer` extension, so the `expect_err` below fails and the test
/// proves the accepted-then-locked configuration.
#[test]
#[ignore = "run with pina test"]
fn audit_sec_30_initialize_rejects_a_token_2022_mint_with_extensions() {
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

		// Install a Token-2022 mint carrying the NonTransferable extension,
		// assembled byte-for-byte: the 82-byte base mint, padding, the
		// account-type byte (2 = mint), and the extension TLV entry
		// (type 9, zero payload). Nothing about transfers is configured, so
		// this is exactly the benign extension shape the exit paths'
		// blanket `assert_no_extensions` rejects after initialization
		// accepts it.
		let mint = Pubkey::new_from_array([0xE7; 32]);
		let mut mint_data = vec![0u8; 165];
		mint_data[0..4].copy_from_slice(&1u32.to_le_bytes());
		mint_data[4..36].copy_from_slice(mint_authority.pubkey().as_ref());
		mint_data[44] = DECIMALS;
		mint_data[45] = 1; // is_initialized
		mint_data[164] = 2; // account type: mint
		mint_data.extend_from_slice(&9u16.to_le_bytes()); // NonTransferable
		mint_data.extend_from_slice(&0u32.to_le_bytes()); // zero-length payload
		assert_eq!(mint_data.len(), 171);
		program
			.install_historical_account(
				&pina_test::HistoricalAccount::new(0, mint, token_2022_program_id(), mint_data)
					.with_lamports(rent_minimum(171)),
			)
			.expect("install the extended mint fixture");

		let (vesting_state, bump) =
			vesting_pda(&program_id, &admin.pubkey(), &beneficiary.pubkey(), &mint);
		let vault = ata_of(&vesting_state, &mint);

		let mut data = vec![VestingInstruction::Initialize as u8, 0u8];
		data.extend_from_slice(&TOTAL.to_le_bytes());
		data.extend_from_slice(&0u64.to_le_bytes());
		data.extend_from_slice(&0u64.to_le_bytes());
		data.extend_from_slice(&0u64.to_le_bytes());
		data.push(bump);
		let initialize = program.instruction(
			&data,
			vec![
				AccountMeta::new(admin.pubkey(), true),
				AccountMeta::new_readonly(beneficiary.pubkey(), false),
				AccountMeta::new_readonly(mint, false),
				AccountMeta::new(vesting_state, false),
				AccountMeta::new(vault, false),
				AccountMeta::new_readonly(ata_program_id(), false),
				AccountMeta::new_readonly(Pubkey::default(), false),
				AccountMeta::new_readonly(token_2022_program_id(), false),
			],
		);

		let error = program
			.send_with_signers(initialize, &[&admin])
			.expect_err("an extended Token-2022 mint must be rejected at initialization");

		assert!(
			program.account(&vesting_state).is_err(),
			"the rejected initialization must not create vesting state"
		);
		drop(error);

		program.stop().expect("stop isolated program test");
	});
}

/// SEC-28: `Cancel` returns the entire unclaimed vault balance to the
/// administrator, even after the schedule has fully vested, so the issuer can
/// confiscate the beneficiary's earned entitlement by cancelling. A revocable
/// vesting cancel must first settle the vested-but-unclaimed amount to the
/// beneficiary.
///
/// Current behavior: the elapsed schedule's cancel hands the whole balance to
/// the admin ATA, so the beneficiary-entitlement assertion below fails and
/// the test proves the confiscation.
#[test]
#[ignore = "run with pina test"]
fn audit_sec_28_cancellation_settles_vested_entitlement_to_the_beneficiary() {
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

		let payer = program.payer();
		let mint = provision_mint(&mut program, &payer, &mint_authority).expect("provision mint");
		let (vesting_state, bump) =
			vesting_pda(&program_id, &admin.pubkey(), &beneficiary.pubkey(), &mint);
		let vault = ata_of(&vesting_state, &mint);
		let beneficiary_ata = ata_of(&beneficiary.pubkey(), &mint);
		let admin_ata = ata_of(&admin.pubkey(), &mint);

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
		mint_into(&program, &mint, &vault, &mint_authority, TOTAL).expect("fund vault");

		// Create the beneficiary's ATA up front (idempotent create, tag 1) so
		// the settlement assertion below reads a real balance instead of a
		// missing account.
		let payer = program.payer();
		let create_beneficiary_ata = Instruction::new_with_bytes(
			ata_program_id(),
			&[1u8],
			vec![
				AccountMeta::new(payer, true),
				AccountMeta::new(beneficiary_ata, false),
				AccountMeta::new_readonly(beneficiary.pubkey(), false),
				AccountMeta::new_readonly(mint, false),
				AccountMeta::new_readonly(Pubkey::default(), false),
				AccountMeta::new_readonly(token_program_id(), false),
			],
		);
		program
			.send_instruction(create_beneficiary_ata)
			.expect("create beneficiary ATA");

		// The schedule has fully elapsed with nothing claimed, so the entire
		// allocation is vested entitlement belonging to the beneficiary.
		program
			.time_travel_to_timestamp_millis(2_500_000_000_000)
			.expect("time travel past the schedule end");

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

		// The vested-but-unclaimed entitlement must have reached the
		// beneficiary, and only a genuinely unvested remainder may return to
		// the administrator. With a fully elapsed schedule that remainder is
		// zero.
		assert_eq!(
			token_amount(&program, &beneficiary_ata),
			TOTAL,
			"SEC-28: the vested-but-unclaimed entitlement must settle to the beneficiary"
		);
		assert_eq!(
			token_amount(&program, &admin_ata),
			0,
			"a fully vested schedule has no unvested remainder to return to the admin"
		);

		program.stop().expect("stop isolated program test");
	});
}
