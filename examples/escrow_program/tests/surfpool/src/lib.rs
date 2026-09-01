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
use program_under_test::EscrowInstruction;
use program_under_test::ID;

/// SPL Token (Tokenkeg…), accepted by the example's SPL allowlist.
const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
/// SPL Associated Token Account program.
const ATA_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

const MINT_SPACE: u64 = 82;
const DECIMALS: u8 = 6;
const FUND: u64 = 1_000_000_000;
const MINTED_A: u64 = 100_000_000;
const OFFER_A: u64 = 40_000_000;
const OFFER_B: u64 = 20_000_000;
const TAKER_OFFER: u64 = 30_000_000;

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
) -> pina_test::Instruction {
	let mut data = vec![EscrowInstruction::Make as u8];
	data.extend_from_slice(&seed.to_le_bytes());
	data.extend_from_slice(&OFFER_A.to_le_bytes());
	data.extend_from_slice(&OFFER_B.to_le_bytes());
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
) -> pina_test::Instruction {
	program.instruction(
		&[EscrowInstruction::Take as u8],
		vec![
			AccountMeta::new(*taker, true),
			AccountMeta::new_readonly(*mint_a, false),
			AccountMeta::new_readonly(*mint_b, false),
			AccountMeta::new(*taker_ata_a, false),
			AccountMeta::new(*taker_ata_b, false),
			AccountMeta::new(*maker, false),
			AccountMeta::new(*maker_ata_b, false),
			AccountMeta::new(*escrow, false),
			AccountMeta::new(*vault, false),
			AccountMeta::new_readonly(token_program_id(), false),
			AccountMeta::new_readonly(ata_program_id(), false),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

/// Escrow layout: 1 discriminator + maker 32 + mint_a 32 + mint_b 32 +
/// amount_a 8 + amount_b 8 + seed 8 + bump.
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
	assert_eq!(account.data.len(), 122);
	assert_eq!(account.data[0], 1, "discriminator is EscrowState");
	assert_eq!(&account.data[1..33], maker.to_bytes());
	assert_eq!(&account.data[33..65], mint_a.to_bytes());
	assert_eq!(&account.data[65..97], mint_b.to_bytes());
	assert_eq!(&account.data[97..105], amount_a.to_le_bytes());
	assert_eq!(&account.data[105..113], amount_b.to_le_bytes());
	assert_eq!(&account.data[113..121], seed.to_le_bytes());
	assert_eq!(account.data[121], bump);
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

		let mint_authority = Keypair::new();
		program
			.fund(&mint_authority.pubkey(), FUND)
			.expect("fund mint authority");

		let maker = program.payer();
		let mint_a_pubkey =
			provision_mint(&program, &maker, &mint_authority).expect("provision mint A");
		let mint_b_pubkey =
			provision_mint(&program, &maker, &mint_authority).expect("provision mint B");

		let taker = Keypair::new();
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
