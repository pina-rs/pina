//! Recover deposits admitted under the old mint-extension policy through
//! the deployed program and the real Token-2022 processor.

use pina_test::HistoricalAccount;
use pina_test::InstructionError;
use pina_test::TransactionError;

use super::*;

fn token_2022_id() -> Pubkey {
	Pubkey::new_from_array(pina::token_2022::ID.to_bytes())
}

fn token_2022_ata(wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
	Pubkey::find_program_address(
		&[wallet.as_ref(), token_2022_id().as_ref(), mint.as_ref()],
		&ata_program_id(),
	)
	.0
}

/// Initialize an optionally extended mint with a freeze authority so refund
/// failure can be exercised in the token CPI, after escrow validation passes.
fn mint(program: &ProgramTest, authority: &Keypair, seed: u8, extended: bool) -> Pubkey {
	let mint = Keypair::new_from_array([seed; 32]);
	// Token-2022 extended mint: padded base (165), account type (1), TLV
	// header (4), and MintCloseAuthority's optional-nonzero address (32).
	let space = if extended { 202 } else { MINT_SPACE };
	program
		.send_with_signers(
			create_account_instruction(
				program,
				&program.payer(),
				&mint.pubkey(),
				rent_minimum(space),
				space,
				&token_2022_id(),
			),
			&[&mint],
		)
		.unwrap_or_else(|error| panic!("create Token-2022 mint: {error:?}"));

	if extended {
		let mut data = vec![25, 1]; // InitializeMintCloseAuthority(Some).
		data.extend_from_slice(authority.pubkey().as_ref());
		program
			.send_instruction(Instruction::new_with_bytes(
				token_2022_id(),
				&data,
				vec![AccountMeta::new(mint.pubkey(), false)],
			))
			.unwrap_or_else(|error| panic!("initialize mint close authority: {error:?}"));
	}

	let mut data = vec![20, DECIMALS]; // InitializeMint2.
	data.extend_from_slice(authority.pubkey().as_ref());
	data.push(1);
	data.extend_from_slice(authority.pubkey().as_ref());
	program
		.send_instruction(Instruction::new_with_bytes(
			token_2022_id(),
			&data,
			vec![AccountMeta::new(mint.pubkey(), false)],
		))
		.unwrap_or_else(|error| panic!("initialize Token-2022 mint: {error:?}"));

	mint.pubkey()
}

fn create_ata(program: &ProgramTest, wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
	let ata = token_2022_ata(wallet, mint);
	program
		.send_instruction(Instruction::new_with_bytes(
			ata_program_id(),
			&[1],
			vec![
				AccountMeta::new(program.payer(), true),
				AccountMeta::new(ata, false),
				AccountMeta::new_readonly(*wallet, false),
				AccountMeta::new_readonly(*mint, false),
				AccountMeta::new_readonly(Pubkey::default(), false),
				AccountMeta::new_readonly(token_2022_id(), false),
			],
		))
		.unwrap_or_else(|error| panic!("create Token-2022 ATA: {error:?}"));

	ata
}

/// Assert the precise execution failure, then prove custody state rolled back.
fn assert_rejected(
	program: &ProgramTest,
	instruction: Instruction,
	signers: &[&dyn Signer],
	error: InstructionError,
	protected: &[Pubkey],
) {
	let before = program
		.snapshot_accounts(protected)
		.unwrap_or_else(|error| panic!("snapshot custody: {error:?}"));
	let failure = program
		.send_with_signers(instruction, signers)
		.expect_err("reject instruction");
	assert_eq!(
		failure.transaction_error(),
		Some(TransactionError::InstructionError(0, error))
	);
	assert_eq!(
		program
			.snapshot_accounts(protected)
			.unwrap_or_else(|error| panic!("snapshot after rejection: {error:?}")),
		before,
		"rejection must preserve balances and escrow bytes"
	);
}

#[test]
#[ignore = "run with pina test"]
fn cancel_recovers_legacy_token_2022_deposits() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.unwrap_or_else(|error| panic!("start program: {error:?}"));
		let maker = Keypair::new_from_array([101; 32]);
		let authority = Keypair::new_from_array([102; 32]);
		program
			.fund(&maker.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund maker: {error:?}"));
		program
			.fund(&authority.pubkey(), FUND)
			.unwrap_or_else(|error| panic!("fund mint authority: {error:?}"));

		// Cover the original pre-envelope state and the current versioned
		// state, with an extension on either stored mint in each layout.
		for (seed, extended_a, legacy_layout) in [
			(107, true, false),
			(109, false, false),
			(103, true, true),
			(105, false, true),
		] {
			let mint_a = mint(&program, &authority, seed, extended_a);
			let mint_b = mint(&program, &authority, seed + 1, !extended_a);
			let (canonical_escrow, canonical_bump) =
				escrow_pda(&program_id, &maker.pubkey(), u64::from(seed));
			let (escrow, bump) = if legacy_layout && extended_a {
				// Old Make admitted valid noncanonical bumps, so recovery must
				// verify the stored address without imposing new canonicality.
				(0..canonical_bump)
					.rev()
					.find_map(|bump| {
						Pubkey::create_program_address(
							&[
								b"escrow",
								maker.pubkey().as_ref(),
								&u64::from(seed).to_le_bytes(),
								&[bump],
							],
							&program_id,
						)
						.ok()
						.map(|address| (address, bump))
					})
					.unwrap_or_else(|| panic!("derive historical noncanonical PDA"))
			} else {
				(canonical_escrow, canonical_bump)
			};
			let vault = create_ata(&program, &escrow, &mint_a);
			let maker_ata = create_ata(&program, &maker.pubkey(), &mint_a);

			let mut make = make_instruction(
				&program,
				&maker.pubkey(),
				&mint_a,
				&mint_b,
				&maker_ata,
				&escrow,
				&vault,
				u64::from(seed),
				bump,
				OFFER_A,
				OFFER_B,
			);
			make.accounts[8].pubkey = token_2022_id();
			assert_rejected(
				&program,
				make,
				&[&maker],
				InstructionError::InvalidAccountData,
				&[maker.pubkey(), maker_ata, vault],
			);
			// The maker may have closed its empty ATA while the offer was open.
			program
				.send_with_signers(
					Instruction::new_with_bytes(
						token_2022_id(),
						&[9], // CloseAccount.
						vec![
							AccountMeta::new(maker_ata, false),
							AccountMeta::new(maker.pubkey(), false),
							AccountMeta::new_readonly(maker.pubkey(), true),
						],
					),
					&[&maker],
				)
				.unwrap_or_else(|error| panic!("close empty maker ATA: {error:?}"));

			// Materialize the custody state admitted by the old policy without
			// weakening Make. The vault is funded by the real token processor.
			// The scan-era layout (7c85cbcf) had no schema-version byte.
			let mut data = vec![1];

			if !legacy_layout {
				data.push(0);
			}

			data.extend_from_slice(maker.pubkey().as_ref());
			data.extend_from_slice(mint_a.as_ref());
			data.extend_from_slice(mint_b.as_ref());
			data.extend_from_slice(&OFFER_A.to_le_bytes());
			data.extend_from_slice(&OFFER_B.to_le_bytes());
			data.extend_from_slice(&u64::from(seed).to_le_bytes());
			data.push(bump);
			let escrow_rent = rent_minimum(if legacy_layout { 122 } else { 123 });
			let historical = HistoricalAccount::new(0, escrow, program_id, data.clone())
				.with_lamports(escrow_rent);
			program
				.install_historical_account(&historical)
				.unwrap_or_else(|error| panic!("install legacy-policy escrow: {error:?}"));
			let mut mint_to = vec![7];
			mint_to.extend_from_slice(&OFFER_A.to_le_bytes());
			program
				.send_with_signers(
					Instruction::new_with_bytes(
						token_2022_id(),
						&mint_to,
						vec![
							AccountMeta::new(mint_a, false),
							AccountMeta::new(vault, false),
							AccountMeta::new_readonly(authority.pubkey(), true),
						],
					),
					&[&authority],
				)
				.unwrap_or_else(|error| panic!("fund legacy vault: {error:?}"));

			let mut cancel = cancel_instruction(
				&program,
				&maker.pubkey(),
				&mint_a,
				&maker_ata,
				&escrow,
				&vault,
				true,
			);
			cancel.accounts[5].pubkey = token_2022_id();
			let protected = [maker.pubkey(), escrow, vault];

			if legacy_layout && extended_a {
				// Raw decoding must reject a forged owner, discriminator, seed,
				// or bump before touching the vault or creating the refund ATA.
				let foreign = Keypair::new_from_array([111; 32]);
				program
					.send_with_signers(
						create_account_instruction(
							&program,
							&program.payer(),
							&foreign.pubkey(),
							escrow_rent,
							122,
							&Pubkey::default(),
						),
						&[&foreign],
					)
					.unwrap_or_else(|error| {
						panic!("create foreign-owned escrow substitute: {error:?}")
					});
				let mut foreign_owner = cancel.clone();
				foreign_owner.accounts[3].pubkey = foreign.pubkey();
				assert_rejected(
					&program,
					foreign_owner,
					&[&maker],
					InstructionError::InvalidAccountOwner,
					&[maker.pubkey(), escrow, vault, foreign.pubkey()],
				);
				for (offset, error) in [
					(0, InstructionError::InvalidAccountData),
					(113, InstructionError::InvalidSeeds),
					(121, InstructionError::InvalidSeeds),
				] {
					let mut malformed = data.clone();
					malformed[offset] ^= u8::MAX;
					program
						.install_historical_account(
							&HistoricalAccount::new(0, escrow, program_id, malformed)
								.with_lamports(escrow_rent),
						)
						.unwrap_or_else(|error| {
							panic!("install malformed historical state: {error:?}")
						});
					assert_rejected(&program, cancel.clone(), &[&maker], error, &protected);
				}
				program
					.install_historical_account(&historical)
					.unwrap_or_else(|error| panic!("restore historical escrow: {error:?}"));
			}

			let mut unsigned = cancel.clone();
			unsigned.accounts[0].is_signer = false;
			assert_rejected(
				&program,
				unsigned,
				&[],
				InstructionError::MissingRequiredSignature,
				&protected,
			);

			// Freeze and thaw use the mint's authority, independent of the
			// escrow PDA. A token CPI rejection must roll back ATA/rent changes.
			for tag in [10, 11] {
				program
					.send_with_signers(
						Instruction::new_with_bytes(
							token_2022_id(),
							&[tag],
							vec![
								AccountMeta::new(vault, false),
								AccountMeta::new_readonly(mint_a, false),
								AccountMeta::new_readonly(authority.pubkey(), true),
							],
						),
						&[&authority],
					)
					.unwrap_or_else(|error| panic!("freeze or thaw vault: {error:?}"));

				if tag == 10 {
					assert_rejected(
						&program,
						cancel.clone(),
						&[&maker],
						InstructionError::Custom(17),
						&protected,
					);
					assert!(
						program.account(&maker_ata).is_err(),
						"ATA creation rolled back"
					);
				}
			}

			let rent_before = program
				.balance(&maker.pubkey())
				.unwrap_or_else(|error| panic!("maker rent: {error:?}"));
			let returned_rent = program
				.balance(&escrow)
				.unwrap_or_else(|error| panic!("escrow rent: {error:?}"))
				+ program
					.balance(&vault)
					.unwrap_or_else(|error| panic!("vault rent: {error:?}"));
			program
				.send_with_signers(cancel, &[&maker])
				.unwrap_or_else(|error| panic!("refund legacy deposit: {error:?}"));
			let refunded_ata = program
				.account(&maker_ata)
				.unwrap_or_else(|error| panic!("recreated and refunded ATA: {error:?}"));
			assert_eq!(token_amount(&refunded_ata), OFFER_A);
			assert!(program.account(&escrow).is_err(), "escrow closed");
			assert!(program.account(&vault).is_err(), "vault closed");
			assert_eq!(
				program
					.balance(&maker.pubkey())
					.unwrap_or_else(|error| panic!("returned rent: {error:?}")),
				rent_before + returned_rent - refunded_ata.lamports
			);
		}

		program
			.stop()
			.unwrap_or_else(|error| panic!("stop program: {error:?}"));
	});
}
