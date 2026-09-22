#![cfg(test)]

//! End-to-end SPL escrow against the real SBF artifact: provision two mints
//! with raw SPL instructions, run the full Make/Take flow, and assert every
//! token balance, escrow field, and close on-chain.

use pina_test::Account;
use pina_test::AccountMeta;
use pina_test::Instruction;
use pina_test::Keypair;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Signer;
use pina_test::TestError;
use program_under_test::EscrowError;
use program_under_test::EscrowInstruction;
use program_under_test::ID;

/// SPL Token (Tokenkeg…), accepted by the example's SPL allowlist.
const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
/// SPL Associated Token Account program.
const ATA_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

const MINT_SPACE: u64 = 82;
/// SPL token account data length.
const TOKEN_ACCOUNT_SPACE: u64 = 165;
const DECIMALS: u8 = 6;
const FUND: u64 = 1_000_000_000;
const MINTED_A: u64 = 100_000_000;
const OFFER_A: u64 = 40_000_000;
const OFFER_B: u64 = 20_000_000;
const TAKER_OFFER: u64 = 30_000_000;

fn token_program_id() -> Pubkey {
	Pubkey::from_str_const(TOKEN_PROGRAM)
}

fn rent_sysvar_id() -> Pubkey {
	Pubkey::from_str_const("SysvarRent111111111111111111111111111111111")
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

fn escrow_pda(program_id: &Pubkey, maker: &Pubkey, seed: u64) -> (Pubkey, u8) {
	Pubkey::find_program_address(
		&[b"escrow", maker.as_ref(), &seed.to_le_bytes()],
		program_id,
	)
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

fn rent_minimum(space: u64) -> u64 {
	pina_test::Rent::default().minimum_balance(usize::try_from(space).expect("space"))
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

	// SPL `InitializeMint2` = tag 20 (no rent sysvar in the account list).
	// SPL `InitializeMint2` = tag 20: decimals, mint authority, freeze none.
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

/// Create the wallet's associated token account (idempotently) and optionally
/// mint `amount` into it.
fn provision_ata(
	program: &ProgramTest,
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

fn make_instruction(
	program: &ProgramTest,
	maker: &Pubkey,
	mint_a: &Pubkey,
	mint_b: &Pubkey,
	maker_ata_a: &Pubkey,
	escrow: &Pubkey,
	vault: &Pubkey,
	seed: u64,
	bump: u8,
	amount_a: u64,
	amount_b: u64,
) -> pina_test::Instruction {
	// discriminator + migration version, then seed, amounts, and bump.
	let mut data = vec![EscrowInstruction::Make as u8, 0u8];
	data.extend_from_slice(&seed.to_le_bytes());
	data.extend_from_slice(&amount_a.to_le_bytes());
	data.extend_from_slice(&amount_b.to_le_bytes());
	data.push(bump);

	program.instruction(
		&data,
		vec![
			AccountMeta::new(*maker, true),
			AccountMeta::new_readonly(*mint_a, false),
			AccountMeta::new_readonly(*mint_b, false),
			AccountMeta::new(*maker_ata_a, false),
			AccountMeta::new(*escrow, false),
			AccountMeta::new(*vault, false),
			AccountMeta::new_readonly(ata_program_id(), false),
			AccountMeta::new_readonly(Pubkey::default(), false),
			AccountMeta::new_readonly(token_program_id(), false),
		],
	)
}

fn take_instruction(
	program: &ProgramTest,
	taker: &Pubkey,
	mint_a: &Pubkey,
	mint_b: &Pubkey,
	taker_ata_a: &Pubkey,
	taker_ata_b: &Pubkey,
	maker: &Pubkey,
	maker_ata_b: &Pubkey,
	escrow: &Pubkey,
	vault: &Pubkey,
	maker_writable: bool,
) -> pina_test::Instruction {
	let maker_meta = if maker_writable {
		AccountMeta::new(*maker, false)
	} else {
		AccountMeta::new_readonly(*maker, false)
	};

	program.instruction(
		&[EscrowInstruction::Take as u8, 0u8],
		vec![
			AccountMeta::new(*taker, true),
			AccountMeta::new_readonly(*mint_a, false),
			AccountMeta::new_readonly(*mint_b, false),
			AccountMeta::new(*taker_ata_a, false),
			AccountMeta::new(*taker_ata_b, false),
			maker_meta,
			AccountMeta::new(*maker_ata_b, false),
			AccountMeta::new(*escrow, false),
			AccountMeta::new(*vault, false),
			AccountMeta::new_readonly(token_program_id(), false),
			AccountMeta::new_readonly(ata_program_id(), false),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

/// `Cancel` takes an empty payload: maker, mint_a, maker_ata_a, escrow, vault,
/// then the token, associated-token, and system programs.
fn cancel_instruction(
	program: &ProgramTest,
	maker: &Pubkey,
	mint_a: &Pubkey,
	maker_ata_a: &Pubkey,
	escrow: &Pubkey,
	vault: &Pubkey,
	maker_writable: bool,
) -> pina_test::Instruction {
	let maker_meta = if maker_writable {
		AccountMeta::new(*maker, true)
	} else {
		AccountMeta::new_readonly(*maker, true)
	};

	program.instruction(
		&[EscrowInstruction::Cancel as u8, 0u8],
		vec![
			maker_meta,
			AccountMeta::new_readonly(*mint_a, false),
			AccountMeta::new(*maker_ata_a, false),
			AccountMeta::new(*escrow, false),
			AccountMeta::new(*vault, false),
			AccountMeta::new_readonly(token_program_id(), false),
			AccountMeta::new_readonly(ata_program_id(), false),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

/// Escrow layout: discriminator + migration version + maker 32 + mint_a 32 +
/// mint_b 32 + amount_a 8 + amount_b 8 + seed 8 + bump. `tests/abi_layout.rs`
/// pins the same envelope geometry.
fn assert_escrow(
	account: &Account,
	maker: &Pubkey,
	mint_a: &Pubkey,
	mint_b: &Pubkey,
	amount_a: u64,
	amount_b: u64,
	seed: u64,
	bump: u8,
) {
	assert_eq!(account.data.len(), 123);
	assert_eq!(account.data[0], 1, "discriminator is EscrowState");
	assert_eq!(account.data[1], 0, "stored migration version is current");
	assert_eq!(&account.data[2..34], maker.to_bytes());
	assert_eq!(&account.data[34..66], mint_a.to_bytes());
	assert_eq!(&account.data[66..98], mint_b.to_bytes());
	assert_eq!(&account.data[98..106], amount_a.to_le_bytes());
	assert_eq!(&account.data[106..114], amount_b.to_le_bytes());
	assert_eq!(&account.data[114..122], seed.to_le_bytes());
	assert_eq!(account.data[122], bump);
}

fn token_amount(account: &Account) -> u64 {
	u64::from_le_bytes(account.data[64..72].try_into().expect("token amount"))
}

#[test]
#[ignore = "run with pina test"]
fn full_escrow_round_trip() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([2; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let maker = program.payer();
		let mint_a_pubkey =
			provision_mint(&program, &maker, &mint_authority, 3).expect("provision mint A");
		let mint_b_pubkey =
			provision_mint(&program, &maker, &mint_authority, 4).expect("provision mint B");

		let taker = Keypair::new_from_array([5; 32]);
		program.fund(&taker.pubkey(), FUND).expect("fund taker");

		let maker_ata_a = ata_of(&maker, &mint_a_pubkey);
		let maker_ata_b = ata_of(&maker, &mint_b_pubkey);
		let taker_ata_a = ata_of(&taker.pubkey(), &mint_a_pubkey);
		let taker_ata_b = ata_of(&taker.pubkey(), &mint_b_pubkey);

		provision_ata(
			&program,
			&maker,
			&maker,
			&mint_a_pubkey,
			Some(&mint_authority),
			MINTED_A,
		)
		.expect("maker token A ATA");
		provision_ata(&program, &maker, &maker, &mint_b_pubkey, None, 0)
			.expect("maker token B ATA");
		provision_ata(&program, &maker, &taker.pubkey(), &mint_a_pubkey, None, 0)
			.expect("taker token A ATA");
		provision_ata(
			&program,
			&maker,
			&taker.pubkey(),
			&mint_b_pubkey,
			Some(&mint_authority),
			TAKER_OFFER,
		)
		.expect("taker token B ATA");

		let seed = 1u64;
		let (escrow, bump) = escrow_pda(&program_id, &maker, seed);
		let vault = ata_of(&escrow, &mint_a_pubkey);

		program
			.send_instruction(make_instruction(
				&program,
				&maker,
				&mint_a_pubkey,
				&mint_b_pubkey,
				&maker_ata_a,
				&escrow,
				&vault,
				seed,
				bump,
				OFFER_A,
				OFFER_B,
			))
			.expect("execute Make");

		let escrow_account = program.account(&escrow).expect("escrow state exists");
		assert_escrow(
			&escrow_account,
			&maker,
			&mint_a_pubkey,
			&mint_b_pubkey,
			OFFER_A,
			OFFER_B,
			seed,
			bump,
		);
		assert_eq!(
			token_amount(&program.account(&vault).expect("vault exists after Make")),
			OFFER_A,
			"the vault holds the escrowed token A"
		);

		program
			.send_with_signers(
				take_instruction(
					&program,
					&taker.pubkey(),
					&mint_a_pubkey,
					&mint_b_pubkey,
					&taker_ata_a,
					&taker_ata_b,
					&maker,
					&maker_ata_b,
					&escrow,
					&vault,
					true,
				),
				&[&taker],
			)
			.expect("execute Take");

		assert_eq!(
			token_amount(&program.account(&taker_ata_a).expect("taker token A ATA"),),
			OFFER_A,
			"taker received the escrowed token A"
		);
		assert_eq!(
			token_amount(&program.account(&maker_ata_b).expect("maker token B ATA"),),
			OFFER_B,
			"maker received the offered token B"
		);
		assert_eq!(
			token_amount(&program.account(&taker_ata_b).expect("taker token B ATA"),),
			TAKER_OFFER - OFFER_B,
			"taker paid exactly OFFER_B"
		);
		assert!(
			program.account(&escrow).is_err(),
			"escrow closed after Take"
		);
		assert!(program.account(&vault).is_err(), "vault closed after Take");

		program.stop().expect("stop isolated program test");
	});
}

/// A zero side of the offer is rejected before any account is created or any
/// token moves, so a fat-fingered `Make` cannot give token A away for nothing.
#[test]
#[ignore = "run with pina test"]
fn make_rejects_a_zero_amount() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([12; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let maker = program.payer();
		let mint_a_pubkey =
			provision_mint(&program, &maker, &mint_authority, 6).expect("provision mint A");
		let mint_b_pubkey =
			provision_mint(&program, &maker, &mint_authority, 7).expect("provision mint B");

		let maker_ata_a = ata_of(&maker, &mint_a_pubkey);
		provision_ata(
			&program,
			&maker,
			&maker,
			&mint_a_pubkey,
			Some(&mint_authority),
			MINTED_A,
		)
		.expect("maker token A ATA");

		let seed = 21u64;
		let (escrow, bump) = escrow_pda(&program_id, &maker, seed);
		let vault = ata_of(&escrow, &mint_a_pubkey);
		let maker_balance_before = program
			.account(&maker)
			.expect("maker account before Make")
			.lamports;

		// `amount_b == 0` asks for nothing in return for the escrowed token A.
		let error = program
			.send_instruction(make_instruction(
				&program,
				&maker,
				&mint_a_pubkey,
				&mint_b_pubkey,
				&maker_ata_a,
				&escrow,
				&vault,
				seed,
				bump,
				OFFER_A,
				0,
			))
			.expect_err("a zero token B amount is not an offer");
		// The exact variant matters: printing the error passes when the program
		// fails for an unrelated reason.
		pina_test::assert_custom_error(&error, EscrowError::EmptyOffer as u32);

		// Nothing moved and nothing was created: no token A left the maker, the
		// maker paid no rent, the escrow PDA does not exist, and the vault was
		// never initialized.
		assert_eq!(
			token_amount(&program.account(&maker_ata_a).expect("maker token A ATA")),
			MINTED_A,
			"a rejected Make must not move token A"
		);
		assert_eq!(
			program
				.account(&maker)
				.expect("maker account after Make")
				.lamports,
			maker_balance_before,
			"a rejected Make must not charge rent"
		);
		assert!(
			program.account(&escrow).is_err(),
			"a rejected Make must not create the escrow"
		);
		assert!(
			program.account(&vault).is_err(),
			"a rejected Make must not create the vault"
		);

		// `amount_a == 0` has the same one-sided shape and is rejected too.
		let error = program
			.send_instruction(make_instruction(
				&program,
				&maker,
				&mint_a_pubkey,
				&mint_b_pubkey,
				&maker_ata_a,
				&escrow,
				&vault,
				seed,
				bump,
				0,
				OFFER_B,
			))
			.expect_err("a zero token A amount is not an offer");
		pina_test::assert_custom_error(&error, EscrowError::EmptyOffer as u32);
		assert!(
			program.account(&escrow).is_err(),
			"a rejected Make with zero token A must not create the escrow"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// The maker receives both the vault rent and the escrow rent during `Take`,
/// so a read-only maker must be refused instead of reaching the close CPIs.
/// Create a wallet's associated token account idempotently, paid and signed by
/// an explicit payer keypair. The ATA program never requires the wallet's
/// signature, so this is exactly how a griefer pre-creates a vault whose
/// wallet is a derivable escrow PDA.
fn create_ata_idempotent(
	program: &ProgramTest,
	payer: &Keypair,
	wallet: &Pubkey,
	mint: &Pubkey,
) -> Result<Pubkey, TestError> {
	let ata = ata_of(wallet, mint);
	// ATA `CreateIdempotent` = tag 1.
	let create = Instruction::new_with_bytes(
		ata_program_id(),
		&[1u8],
		vec![
			AccountMeta::new(payer.pubkey(), true),
			AccountMeta::new(ata, false),
			AccountMeta::new_readonly(*wallet, false),
			AccountMeta::new_readonly(*mint, false),
			AccountMeta::new_readonly(Pubkey::default(), false),
			AccountMeta::new_readonly(token_program_id(), false),
		],
	);
	program.send_with_signers(create, &[payer])?;

	Ok(ata)
}

#[test]
#[ignore = "run with pina test"]
fn make_tolerates_a_precreated_empty_vault() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([23; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let maker = program.payer();
		let mint_a_pubkey =
			provision_mint(&program, &maker, &mint_authority, 21).expect("provision mint A");
		let mint_b_pubkey =
			provision_mint(&program, &maker, &mint_authority, 22).expect("provision mint B");

		let taker = Keypair::new_from_array([24; 32]);
		program.fund(&taker.pubkey(), FUND).expect("fund taker");
		let attacker = Keypair::new_from_array([29; 32]);
		program
			.fund(&attacker.pubkey(), FUND)
			.expect("fund attacker");

		let maker_ata_a = ata_of(&maker, &mint_a_pubkey);
		let maker_ata_b = ata_of(&maker, &mint_b_pubkey);
		let taker_ata_a = ata_of(&taker.pubkey(), &mint_a_pubkey);
		let taker_ata_b = ata_of(&taker.pubkey(), &mint_b_pubkey);

		provision_ata(
			&program,
			&maker,
			&maker,
			&mint_a_pubkey,
			Some(&mint_authority),
			MINTED_A,
		)
		.expect("maker token A ATA");
		provision_ata(&program, &maker, &maker, &mint_b_pubkey, None, 0)
			.expect("maker token B ATA");
		provision_ata(&program, &maker, &taker.pubkey(), &mint_a_pubkey, None, 0)
			.expect("taker token A ATA");
		provision_ata(
			&program,
			&maker,
			&taker.pubkey(),
			&mint_b_pubkey,
			Some(&mint_authority),
			TAKER_OFFER,
		)
		.expect("taker token B ATA");

		let seed = 31u64;
		let (escrow, bump) = escrow_pda(&program_id, &maker, seed);
		let vault = ata_of(&escrow, &mint_a_pubkey);

		// The D2 grief: before the maker does anything, an attacker derives the
		// escrow PDA from public seeds and creates its vault ATA — the ATA
		// program needs no wallet signature. The vault exists and is empty, so
		// a plain empty-account rejection would strand this `(maker, seed)`
		// slot forever.
		let attacker_vault = create_ata_idempotent(&program, &attacker, &escrow, &mint_a_pubkey)
			.expect("attacker pre-creates the derived vault");
		assert_eq!(attacker_vault, vault, "the attacker created the real vault");
		assert_eq!(
			token_amount(&program.account(&vault).expect("pre-created vault exists")),
			0,
			"the pre-created vault is empty"
		);

		// The maker's `Make` now succeeds instead of dying on the vault check.
		program
			.send_instruction(make_instruction(
				&program,
				&maker,
				&mint_a_pubkey,
				&mint_b_pubkey,
				&maker_ata_a,
				&escrow,
				&vault,
				seed,
				bump,
				OFFER_A,
				OFFER_B,
			))
			.expect("Make succeeds despite the pre-created empty vault");

		let escrow_account = program.account(&escrow).expect("escrow state exists");
		assert_escrow(
			&escrow_account,
			&maker,
			&mint_a_pubkey,
			&mint_b_pubkey,
			OFFER_A,
			OFFER_B,
			seed,
			bump,
		);
		assert_eq!(
			token_amount(&program.account(&vault).expect("vault exists after Make")),
			OFFER_A,
			"the vault holds exactly the escrowed token A"
		);

		// The escrow still completes a full make/take round trip.
		program
			.send_with_signers(
				take_instruction(
					&program,
					&taker.pubkey(),
					&mint_a_pubkey,
					&mint_b_pubkey,
					&taker_ata_a,
					&taker_ata_b,
					&maker,
					&maker_ata_b,
					&escrow,
					&vault,
					true,
				),
				&[&taker],
			)
			.expect("execute Take");

		assert_eq!(
			token_amount(&program.account(&taker_ata_a).expect("taker token A ATA")),
			OFFER_A,
			"taker received the escrowed token A"
		);
		assert_eq!(
			token_amount(&program.account(&maker_ata_b).expect("maker token B ATA")),
			OFFER_B,
			"maker received the offered token B"
		);
		assert_eq!(
			token_amount(&program.account(&taker_ata_b).expect("taker token B ATA")),
			TAKER_OFFER - OFFER_B,
			"taker paid exactly OFFER_B"
		);
		assert!(
			program.account(&escrow).is_err(),
			"escrow closed after Take"
		);
		assert!(program.account(&vault).is_err(), "vault closed after Take");

		program.stop().expect("stop isolated program test");
	});
}

/// The maker's abort path: `Cancel` refunds the full vault balance to the
/// maker and closes both the vault and the escrow, returning all rent to the
/// maker.
#[test]
#[ignore = "run with pina test"]
fn cancel_refunds_the_maker_and_closes_the_escrow() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([31; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		// The maker is deliberately not the transaction payer so its lamport
		// balance isolates the escrow rent it pays and gets back.
		let maker = Keypair::new_from_array([32; 32]);
		program.fund(&maker.pubkey(), FUND).expect("fund maker");
		let maker_pubkey = maker.pubkey();

		let mint_a_pubkey = provision_mint(&program, &program.payer(), &mint_authority, 33)
			.expect("provision mint A");
		let mint_b_pubkey = provision_mint(&program, &program.payer(), &mint_authority, 34)
			.expect("provision mint B");

		let maker_ata_a = ata_of(&maker_pubkey, &mint_a_pubkey);
		provision_ata(
			&program,
			&program.payer(),
			&maker_pubkey,
			&mint_a_pubkey,
			Some(&mint_authority),
			MINTED_A,
		)
		.expect("maker token A ATA");

		let seed = 41u64;
		let (escrow, bump) = escrow_pda(&program_id, &maker_pubkey, seed);
		let vault = ata_of(&escrow, &mint_a_pubkey);

		// The vault account did not exist before `Make`: its rent is part of
		// what `Cancel` must return.
		assert!(
			program.account(&vault).is_err(),
			"the vault does not exist before Make"
		);
		let maker_balance_before = program
			.account(&maker_pubkey)
			.expect("maker account before Make")
			.lamports;

		program
			.send_with_signers(
				make_instruction(
					&program,
					&maker_pubkey,
					&mint_a_pubkey,
					&mint_b_pubkey,
					&maker_ata_a,
					&escrow,
					&vault,
					seed,
					bump,
					OFFER_A,
					OFFER_B,
				),
				&[&maker],
			)
			.expect("execute Make");
		assert_eq!(
			token_amount(&program.account(&maker_ata_a).expect("maker token A ATA")),
			MINTED_A - OFFER_A,
			"Make moved the offered token A into the vault"
		);

		program
			.send_with_signers(
				cancel_instruction(
					&program,
					&maker_pubkey,
					&mint_a_pubkey,
					&maker_ata_a,
					&escrow,
					&vault,
					true,
				),
				&[&maker],
			)
			.expect("execute Cancel");

		assert_eq!(
			token_amount(&program.account(&maker_ata_a).expect("maker token A ATA")),
			MINTED_A,
			"Cancel refunded the full vault balance"
		);
		assert!(
			program.account(&escrow).is_err(),
			"escrow closed after Cancel"
		);
		assert!(
			program.account(&vault).is_err(),
			"vault closed after Cancel"
		);
		// Both the vault rent and the escrow rent came back to the maker.
		let maker_balance_after = program
			.account(&maker_pubkey)
			.expect("maker account after Cancel")
			.lamports;
		assert!(
			maker_balance_after + 20_000 >= maker_balance_before,
			"Cancel returned the escrow and vault rent to the maker: before \
			 {maker_balance_before}, after {maker_balance_after}"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// Only the recorded maker may abort an offer: a stranger's `Cancel` is
/// refused and the escrow survives intact.
#[test]
#[ignore = "run with pina test"]
fn cancel_rejects_a_stranger() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([35; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let maker = Keypair::new_from_array([36; 32]);
		program.fund(&maker.pubkey(), FUND).expect("fund maker");
		let maker_pubkey = maker.pubkey();
		let stranger = Keypair::new_from_array([37; 32]);
		program
			.fund(&stranger.pubkey(), FUND)
			.expect("fund stranger");

		let mint_a_pubkey = provision_mint(&program, &program.payer(), &mint_authority, 38)
			.expect("provision mint A");
		let mint_b_pubkey = provision_mint(&program, &program.payer(), &mint_authority, 39)
			.expect("provision mint B");

		let maker_ata_a = ata_of(&maker_pubkey, &mint_a_pubkey);
		provision_ata(
			&program,
			&program.payer(),
			&maker_pubkey,
			&mint_a_pubkey,
			Some(&mint_authority),
			MINTED_A,
		)
		.expect("maker token A ATA");

		let seed = 42u64;
		let (escrow, bump) = escrow_pda(&program_id, &maker_pubkey, seed);
		let vault = ata_of(&escrow, &mint_a_pubkey);

		program
			.send_with_signers(
				make_instruction(
					&program,
					&maker_pubkey,
					&mint_a_pubkey,
					&mint_b_pubkey,
					&maker_ata_a,
					&escrow,
					&vault,
					seed,
					bump,
					OFFER_A,
					OFFER_B,
				),
				&[&maker],
			)
			.expect("execute Make");

		// The stranger signs its own Cancel, so the refusal comes from the
		// recorded-maker check rather than from a missing signature. Its own
		// ATA is presented as the refund destination.
		let stranger_ata_a = ata_of(&stranger.pubkey(), &mint_a_pubkey);
		provision_ata(
			&program,
			&program.payer(),
			&stranger.pubkey(),
			&mint_a_pubkey,
			None,
			0,
		)
		.expect("stranger token A ATA");

		let error = program
			.send_with_signers(
				cancel_instruction(
					&program,
					&stranger.pubkey(),
					&mint_a_pubkey,
					&stranger_ata_a,
					&escrow,
					&vault,
					true,
				),
				&[&stranger],
			)
			.expect_err("a stranger must not cancel the maker's offer");
		assert!(
			error.transaction_error().is_some(),
			"the stranger's Cancel must be a transaction failure: {error:?}"
		);

		// The escrow is untouched: only the recorded maker may abort.
		assert_escrow(
			&program
				.account(&escrow)
				.expect("escrow survives a refused Cancel"),
			&maker_pubkey,
			&mint_a_pubkey,
			&mint_b_pubkey,
			OFFER_A,
			OFFER_B,
			seed,
			bump,
		);
		assert_eq!(
			token_amount(
				&program
					.account(&vault)
					.expect("vault survives a refused Cancel")
			),
			OFFER_A,
			"the vault still holds the escrowed token A"
		);
		assert_eq!(
			token_amount(
				&program
					.account(&stranger_ata_a)
					.expect("stranger token A ATA")
			),
			0,
			"the stranger received nothing"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// `Cancel` credits the maker three times — the refunded token A, the vault
/// rent, and the escrow rent — so a read-only maker must be refused instead of
/// reaching the close CPIs.
#[test]
#[ignore = "run with pina test"]
fn cancel_requires_a_writable_maker() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([43; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let maker = Keypair::new_from_array([44; 32]);
		program.fund(&maker.pubkey(), FUND).expect("fund maker");
		let maker_pubkey = maker.pubkey();

		let mint_a_pubkey = provision_mint(&program, &program.payer(), &mint_authority, 45)
			.expect("provision mint A");
		let mint_b_pubkey = provision_mint(&program, &program.payer(), &mint_authority, 46)
			.expect("provision mint B");

		let maker_ata_a = ata_of(&maker_pubkey, &mint_a_pubkey);
		provision_ata(
			&program,
			&program.payer(),
			&maker_pubkey,
			&mint_a_pubkey,
			Some(&mint_authority),
			MINTED_A,
		)
		.expect("maker token A ATA");

		let seed = 43u64;
		let (escrow, bump) = escrow_pda(&program_id, &maker_pubkey, seed);
		let vault = ata_of(&escrow, &mint_a_pubkey);

		program
			.send_with_signers(
				make_instruction(
					&program,
					&maker_pubkey,
					&mint_a_pubkey,
					&mint_b_pubkey,
					&maker_ata_a,
					&escrow,
					&vault,
					seed,
					bump,
					OFFER_A,
					OFFER_B,
				),
				&[&maker],
			)
			.expect("execute Make");

		let error = program
			.send_with_signers(
				cancel_instruction(
					&program,
					&maker_pubkey,
					&mint_a_pubkey,
					&maker_ata_a,
					&escrow,
					&vault,
					false,
				),
				&[&maker],
			)
			.expect_err("a read-only maker cannot receive the closed rent");
		assert_eq!(
			error.transaction_error(),
			Some(pina_test::TransactionError::InstructionError(
				0,
				pina_test::InstructionError::InvalidAccountData
			)),
			"a read-only maker is refused before any token moves"
		);

		// The failed Cancel left the escrow and vault intact.
		assert_escrow(
			&program
				.account(&escrow)
				.expect("escrow survives a rejected Cancel"),
			&maker_pubkey,
			&mint_a_pubkey,
			&mint_b_pubkey,
			OFFER_A,
			OFFER_B,
			seed,
			bump,
		);
		assert_eq!(
			token_amount(
				&program
					.account(&vault)
					.expect("vault survives a rejected Cancel")
			),
			OFFER_A,
			"the vault still holds the escrowed token A"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// The tolerance is exact: a pre-created vault holding tokens, or one derived
/// from the wrong mint, still rejects `Make` with the same
/// `AccountAlreadyInitialized` error the plain empty-account check produced
/// before the tolerance existed.
#[test]
#[ignore = "run with pina test"]
fn make_still_rejects_a_prefunded_or_wrong_mint_vault() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([27; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let maker = program.payer();
		let mint_a_pubkey =
			provision_mint(&program, &maker, &mint_authority, 25).expect("provision mint A");
		let mint_b_pubkey =
			provision_mint(&program, &maker, &mint_authority, 26).expect("provision mint B");

		let attacker = Keypair::new_from_array([30; 32]);
		program
			.fund(&attacker.pubkey(), FUND)
			.expect("fund attacker");

		let maker_ata_a = ata_of(&maker, &mint_a_pubkey);
		provision_ata(
			&program,
			&maker,
			&maker,
			&mint_a_pubkey,
			Some(&mint_authority),
			MINTED_A,
		)
		.expect("maker token A ATA");

		const GRIEF_AMOUNT: u64 = 1_000;
		let seed = 32u64;
		let (escrow, bump) = escrow_pda(&program_id, &maker, seed);
		let vault = ata_of(&escrow, &mint_a_pubkey);

		// A correctly derived vault that the attacker prefunded with tokens.
		create_ata_idempotent(&program, &attacker, &escrow, &mint_a_pubkey)
			.expect("attacker pre-creates the derived vault");
		mint_into(
			&program,
			&mint_a_pubkey,
			&vault,
			&mint_authority,
			GRIEF_AMOUNT,
		)
		.expect("prefund the vault with token A");

		let error = program
			.send_instruction(make_instruction(
				&program,
				&maker,
				&mint_a_pubkey,
				&mint_b_pubkey,
				&maker_ata_a,
				&escrow,
				&vault,
				seed,
				bump,
				OFFER_A,
				OFFER_B,
			))
			.expect_err("a prefunded vault must still reject Make");
		assert_eq!(
			error.transaction_error(),
			Some(pina_test::TransactionError::InstructionError(
				0,
				pina_test::InstructionError::AccountAlreadyInitialized
			)),
			"the rejection matches the pre-tolerance empty-account error"
		);
		assert!(
			program.account(&escrow).is_err(),
			"a rejected Make must not create the escrow"
		);
		assert_eq!(
			token_amount(&program.account(&vault).expect("vault intact")),
			GRIEF_AMOUNT,
			"the griefing deposit stays untouched"
		);

		// A vault derived from the wrong mint is foreign to this escrow and is
		// rejected with the same error.
		let wrong_vault = ata_of(&escrow, &mint_b_pubkey);
		create_ata_idempotent(&program, &attacker, &escrow, &mint_b_pubkey)
			.expect("create the wrong-mint vault");
		mint_into(
			&program,
			&mint_b_pubkey,
			&wrong_vault,
			&mint_authority,
			GRIEF_AMOUNT,
		)
		.expect("prefund the wrong-mint vault");

		let error = program
			.send_instruction(make_instruction(
				&program,
				&maker,
				&mint_a_pubkey,
				&mint_b_pubkey,
				&maker_ata_a,
				&escrow,
				&wrong_vault,
				seed,
				bump,
				OFFER_A,
				OFFER_B,
			))
			.expect_err("a wrong-mint vault must still reject Make");
		assert_eq!(
			error.transaction_error(),
			Some(pina_test::TransactionError::InstructionError(
				0,
				pina_test::InstructionError::AccountAlreadyInitialized
			)),
			"the wrong-mint rejection matches the pre-tolerance error"
		);
		assert!(
			program.account(&escrow).is_err(),
			"a wrong-mint rejection must not create the escrow"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// The maker receives both the vault rent and the escrow rent during `Take`,
/// so a read-only maker must be refused instead of reaching the close CPIs.
#[test]
#[ignore = "run with pina test"]
fn take_requires_a_writable_maker() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let mint_authority = Keypair::new_from_array([13; 32]);
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		// The maker is deliberately not the transaction payer: a payer is always
		// writable, and the case under test needs a maker the caller can mark
		// read-only.
		let maker = Keypair::new_from_array([15; 32]);
		program.fund(&maker.pubkey(), FUND).expect("fund maker");
		let maker_pubkey = maker.pubkey();
		let mint_a_pubkey = provision_mint(&program, &program.payer(), &mint_authority, 8)
			.expect("provision mint A");
		let mint_b_pubkey = provision_mint(&program, &program.payer(), &mint_authority, 9)
			.expect("provision mint B");

		let taker = Keypair::new_from_array([14; 32]);
		program.fund(&taker.pubkey(), FUND).expect("fund taker");

		let maker_ata_a = ata_of(&maker_pubkey, &mint_a_pubkey);
		let maker_ata_b = ata_of(&maker_pubkey, &mint_b_pubkey);
		let taker_ata_a = ata_of(&taker.pubkey(), &mint_a_pubkey);
		let taker_ata_b = ata_of(&taker.pubkey(), &mint_b_pubkey);

		provision_ata(
			&program,
			&program.payer(),
			&maker_pubkey,
			&mint_a_pubkey,
			Some(&mint_authority),
			MINTED_A,
		)
		.expect("maker token A ATA");
		provision_ata(
			&program,
			&program.payer(),
			&maker_pubkey,
			&mint_b_pubkey,
			None,
			0,
		)
		.expect("maker token B ATA");
		provision_ata(
			&program,
			&program.payer(),
			&taker.pubkey(),
			&mint_a_pubkey,
			None,
			0,
		)
		.expect("taker token A ATA");
		provision_ata(
			&program,
			&program.payer(),
			&taker.pubkey(),
			&mint_b_pubkey,
			Some(&mint_authority),
			TAKER_OFFER,
		)
		.expect("taker token B ATA");

		let seed = 22u64;
		let (escrow, bump) = escrow_pda(&program_id, &maker_pubkey, seed);
		let vault = ata_of(&escrow, &mint_a_pubkey);

		program
			.send_with_signers(
				make_instruction(
					&program,
					&maker_pubkey,
					&mint_a_pubkey,
					&mint_b_pubkey,
					&maker_ata_a,
					&escrow,
					&vault,
					seed,
					bump,
					OFFER_A,
					OFFER_B,
				),
				&[&maker],
			)
			.expect("execute Make");

		// Presenting the maker as read-only must fail. `validate_writable`
		// returns `InvalidAccountData` for a non-writable account.
		let error = program
			.send_with_signers(
				take_instruction(
					&program,
					&taker.pubkey(),
					&mint_a_pubkey,
					&mint_b_pubkey,
					&taker_ata_a,
					&taker_ata_b,
					&maker_pubkey,
					&maker_ata_b,
					&escrow,
					&vault,
					false,
				),
				&[&taker],
			)
			.expect_err("a read-only maker cannot receive the closed rent");
		assert_eq!(
			error.transaction_error(),
			Some(pina_test::TransactionError::InstructionError(
				0,
				pina_test::InstructionError::InvalidAccountData
			)),
			"a read-only maker is refused before any token moves"
		);

		// The failed Take left the escrow and vault intact and the taker paid
		// nothing.
		assert_escrow(
			&program
				.account(&escrow)
				.expect("escrow survives a rejected Take"),
			&maker_pubkey,
			&mint_a_pubkey,
			&mint_b_pubkey,
			OFFER_A,
			OFFER_B,
			seed,
			bump,
		);
		assert_eq!(
			token_amount(
				&program
					.account(&vault)
					.expect("vault survives a rejected Take")
			),
			OFFER_A,
			"the vault still holds the escrowed token A"
		);
		assert_eq!(
			token_amount(&program.account(&taker_ata_b).expect("taker token B ATA")),
			TAKER_OFFER,
			"the taker paid nothing"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// A shared prefix for the adversarial `Take` cases: provision both mints,
/// the maker's and taker's accounts, and an open escrow to attack.
struct OpenEscrow {
	program_id: Pubkey,
	mint_a: Pubkey,
	mint_b: Pubkey,
	taker: Keypair,
	taker_ata_a: Pubkey,
	taker_ata_b: Pubkey,
	maker: Pubkey,
	maker_ata_b: Pubkey,
	escrow: Pubkey,
	vault: Pubkey,
	seed: u64,
	bump: u8,
}

fn open_escrow(program: &mut ProgramTest) -> OpenEscrow {
	let mint_authority = Keypair::new_from_array([2; 32]);
	program
		.fund(&mint_authority.pubkey(), FUND)
		.expect("fund mint authority");

	let maker = program.payer();
	let mint_a = provision_mint(program, &maker, &mint_authority, 3).expect("provision mint A");
	let mint_b = provision_mint(program, &maker, &mint_authority, 4).expect("provision mint B");

	let taker = Keypair::new_from_array([5; 32]);
	program.fund(&taker.pubkey(), FUND).expect("fund taker");

	let maker_ata_a = ata_of(&maker, &mint_a);
	let taker_ata_a = ata_of(&taker.pubkey(), &mint_a);
	let taker_ata_b = ata_of(&taker.pubkey(), &mint_b);
	let maker_ata_b = ata_of(&maker, &mint_b);

	provision_ata(
		program,
		&maker,
		&maker,
		&mint_a,
		Some(&mint_authority),
		MINTED_A,
	)
	.expect("maker token A ATA");
	provision_ata(program, &maker, &taker.pubkey(), &mint_a, None, 0).expect("taker token A ATA");
	provision_ata(
		program,
		&maker,
		&taker.pubkey(),
		&mint_b,
		Some(&mint_authority),
		TAKER_OFFER,
	)
	.expect("taker token B ATA");

	let seed = 1u64;
	let program_id = Pubkey::new_from_array(ID.to_bytes());
	let (escrow, bump) = escrow_pda(&program_id, &maker, seed);
	let vault = ata_of(&escrow, &mint_a);

	program
		.send_instruction(make_instruction(
			program,
			&maker,
			&mint_a,
			&mint_b,
			&maker_ata_a,
			&escrow,
			&vault,
			seed,
			bump,
			OFFER_A,
			OFFER_B,
		))
		.expect("execute Make");

	OpenEscrow {
		program_id,
		mint_a,
		mint_b,
		taker,
		taker_ata_a,
		taker_ata_b,
		maker,
		maker_ata_b,
		escrow,
		vault,
		seed,
		bump,
	}
}

/// The taker's token-B payment must leave the taker's canonical associated
/// token account: a foreign token-B account the taker cannot sign for is
/// rejected before any transfer, close, or escrow mutation.
#[test]
#[ignore = "run with pina test"]
fn take_rejects_a_foreign_taker_token_b_account() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let opened = open_escrow(&mut program);

		// A funded token-B account belonging to a third wallet: the address
		// is not the taker's associated token account, and the taker cannot
		// authorize a debit from it.
		let mint_authority = Keypair::new_from_array([2; 32]);
		let stranger = Keypair::new_from_array([9; 32]);
		let stranger_ata_b = provision_ata(
			&program,
			&opened.maker,
			&stranger.pubkey(),
			&opened.mint_b,
			Some(&mint_authority),
			TAKER_OFFER,
		)
		.expect("stranger token B ATA");

		let error = program
			.send_with_signers(
				take_instruction(
					&program,
					&opened.taker.pubkey(),
					&opened.mint_a,
					&opened.mint_b,
					&opened.taker_ata_a,
					&stranger_ata_b,
					&opened.maker,
					&opened.maker_ata_b,
					&opened.escrow,
					&opened.vault,
					true,
				),
				&[&opened.taker],
			)
			.expect_err("a foreign token B account must not fund the payment");

		assert_eq!(
			error.transaction_error(),
			Some(pina_test::TransactionError::InstructionError(
				0,
				pina_test::InstructionError::InvalidSeeds
			)),
			"the canonical address check rejects the foreign account"
		);

		assert_escrow(
			&program
				.account(&opened.escrow)
				.expect("escrow survives a rejected Take"),
			&opened.maker,
			&opened.mint_a,
			&opened.mint_b,
			OFFER_A,
			OFFER_B,
			opened.seed,
			opened.bump,
		);
		assert_eq!(
			token_amount(
				&program
					.account(&opened.vault)
					.expect("vault survives a rejected Take")
			),
			OFFER_A,
			"the vault still holds the escrowed token A"
		);
		assert_eq!(
			token_amount(
				&program
					.account(&stranger_ata_b)
					.expect("stranger token B ATA")
			),
			TAKER_OFFER,
			"the stranger's balance is untouched"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// A non-canonical token-B account the taker *does* control (an auxiliary
/// account initialized directly rather than derived) must not fund the
/// payment either: `Take` pins the source to the taker's canonical associated
/// token account, so the escrow's accounting matches the published account
/// layout even when the taker would gladly pay from elsewhere.
#[test]
#[ignore = "run with pina test"]
fn take_rejects_a_noncanonical_taker_token_b_account() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let opened = open_escrow(&mut program);

		// An auxiliary token-B account owned by the taker: initialized with a
		// raw SPL `InitializeAccount` (tag 1) at a vanity address, so it is a
		// fully valid account the taker can sign for — but it is not the
		// taker's associated token account for mint B.
		let mint_authority = Keypair::new_from_array([2; 32]);
		let auxiliary = Keypair::new_from_array([9; 32]);
		let create = create_account_instruction(
			&program,
			&opened.maker,
			&auxiliary.pubkey(),
			rent_minimum(TOKEN_ACCOUNT_SPACE),
			TOKEN_ACCOUNT_SPACE,
			&token_program_id(),
		);
		program
			.send_with_signers(create, &[&auxiliary])
			.expect("create the auxiliary token account");
		// SPL `InitializeAccount` = tag 1: account, mint, owner.
		let initialize = Instruction::new_with_bytes(
			token_program_id(),
			&[1u8],
			vec![
				AccountMeta::new(auxiliary.pubkey(), false),
				AccountMeta::new_readonly(opened.mint_b, false),
				AccountMeta::new_readonly(opened.taker.pubkey(), false),
				AccountMeta::new_readonly(rent_sysvar_id(), false),
			],
		);
		program
			.send_instruction(initialize)
			.expect("initialize the auxiliary token account");
		mint_into(
			&program,
			&opened.mint_b,
			&auxiliary.pubkey(),
			&mint_authority,
			TAKER_OFFER,
		)
		.expect("fund the auxiliary token account");

		let error = program
			.send_with_signers(
				take_instruction(
					&program,
					&opened.taker.pubkey(),
					&opened.mint_a,
					&opened.mint_b,
					&opened.taker_ata_a,
					&auxiliary.pubkey(),
					&opened.maker,
					&opened.maker_ata_b,
					&opened.escrow,
					&opened.vault,
					true,
				),
				&[&opened.taker],
			)
			.expect_err("a non-canonical source must not fund the payment");

		assert_eq!(
			error.transaction_error(),
			Some(pina_test::TransactionError::InstructionError(
				0,
				pina_test::InstructionError::InvalidSeeds
			)),
			"the canonical address check pins the taker's payment source"
		);

		// The escrow is untouched end to end: no token moved from anywhere,
		// and the escrow and vault survive for a legitimate retry.
		assert_escrow(
			&program
				.account(&opened.escrow)
				.expect("escrow survives a rejected Take"),
			&opened.maker,
			&opened.mint_a,
			&opened.mint_b,
			OFFER_A,
			OFFER_B,
			opened.seed,
			opened.bump,
		);
		assert_eq!(
			token_amount(
				&program
					.account(&opened.vault)
					.expect("vault survives a rejected Take")
			),
			OFFER_A,
			"the vault still holds the escrowed token A"
		);
		assert_eq!(
			token_amount(
				&program
					.account(&auxiliary.pubkey())
					.expect("auxiliary token account")
			),
			TAKER_OFFER,
			"the auxiliary account the taker controls was not debited"
		);
		assert_eq!(
			token_amount(
				&program
					.account(&opened.taker_ata_b)
					.expect("taker token B ATA")
			),
			TAKER_OFFER,
			"the taker's canonical account was not debited either"
		);

		program.stop().expect("stop isolated program test");
	});
}
