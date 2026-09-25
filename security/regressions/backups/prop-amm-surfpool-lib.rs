#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::Keypair;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Signer;
use program_under_test::ID;
use program_under_test::PropAmmInstruction;

fn initialize_instruction(
	program: &ProgramTest,
	payer: &Pubkey,
	oracle: &Pubkey,
) -> pina_test::Instruction {
	program.instruction(
		&[PropAmmInstruction::Initialize as u8, 0u8],
		vec![
			AccountMeta::new(*payer, true),
			AccountMeta::new(*oracle, true),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

fn update_instruction(
	program: &ProgramTest,
	oracle: &Pubkey,
	authority: &Pubkey,
	new_price: u64,
) -> pina_test::Instruction {
	let mut data = vec![PropAmmInstruction::Update as u8, 0u8];
	data.extend_from_slice(&new_price.to_le_bytes());

	program.instruction(
		&data,
		vec![
			AccountMeta::new(*oracle, false),
			AccountMeta::new_readonly(*authority, true),
		],
	)
}

fn rotate_instruction(
	program: &ProgramTest,
	oracle: &Pubkey,
	authority: &Pubkey,
	new_authority: &Pubkey,
) -> pina_test::Instruction {
	let mut data = vec![PropAmmInstruction::RotateAuthority as u8, 0u8];
	data.extend_from_slice(new_authority.as_ref());

	program.instruction(
		&data,
		vec![
			AccountMeta::new(*oracle, false),
			AccountMeta::new_readonly(*authority, true),
		],
	)
}

/// The full encoded `OracleState` image: discriminator, migration version,
/// authority, then price.
fn oracle_bytes(authority: &Pubkey, price: u64) -> [u8; 42] {
	let mut data = [0u8; 42];
	data[0] = 1;
	data[2..34].copy_from_slice(authority.as_ref());
	data[34..42].copy_from_slice(&price.to_le_bytes());

	data
}

/// The oracle account the signer chooses becomes the program's oracle. The
/// initializer becomes its authority.
#[test]
#[ignore = "run with pina test"]
fn initialize_records_the_payer_as_authority() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let payer = program.payer();
		let oracle = Keypair::new_from_array([2; 32]);

		// The oracle account must sign its own create-account CPI.
		// No pre-funding: the create-account CPI inside the program funds the
		// oracle, and only its keypair must provide a signature.
		let instruction = initialize_instruction(&program, &payer, &oracle.pubkey());
		program
			.send_with_signers(instruction, &[&oracle])
			.expect("execute Initialize");

		let account = program.account(&oracle.pubkey()).expect("fetch oracle");
		assert_eq!(account.owner, program_id);
		assert_eq!(
			account.data[0..42],
			oracle_bytes(&payer, 0),
			"oracle state records the payer and zero price"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// The hardcoded update authority can always update the price.
#[test]
#[ignore = "run with pina test"]
fn update_requires_the_update_authority() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let payer = program.payer();
		let oracle = Keypair::new_from_array([2; 32]);

		program
			.send_with_signers(
				initialize_instruction(&program, &payer, &oracle.pubkey()),
				&[&oracle],
			)
			.expect("execute Initialize");

		let update = update_instruction(&program, &oracle.pubkey(), &payer, 99);
		let error = program
			.send_instruction(update)
			.expect_err("only the static update authority may update");
		assert_eq!(error.operation(), "execute program instruction");
		eprintln!("unauthorized update error: {}", error.message());

		program.stop().expect("stop isolated program test");
	});
}

/// Rotate hands the oracle authority to another wallet, which then owns
/// accounting, though the STATIC update gate still applies to `Update`.
#[test]
#[ignore = "run with pina test"]
fn rotate_hands_over_the_oracle_authority() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let payer = program.payer();
		let oracle = Keypair::new_from_array([2; 32]);

		program
			.send_with_signers(
				initialize_instruction(&program, &payer, &oracle.pubkey()),
				&[&oracle],
			)
			.expect("execute Initialize");

		let new_authority = Keypair::new_from_array([3; 32]);

		program
			.send_instruction(rotate_instruction(
				&program,
				&oracle.pubkey(),
				&payer,
				&new_authority.pubkey(),
			))
			.expect("the current authority rotates to the new wallet");

		// The CURRENT holder is now `new_authority`; rotating again with the
		// stale holder fails the recorded-authority check.
		let error = program
			.send_instruction(rotate_instruction(
				&program,
				&oracle.pubkey(),
				&payer,
				&program.payer(),
			))
			.expect_err("the previous authority may no longer rotate");
		assert_eq!(error.operation(), "execute program instruction");
		eprintln!("stale rotate error: {}", error.message());

		let account = program.account(&oracle.pubkey()).expect("fetch oracle");
		assert_eq!(
			account.data[2..34],
			new_authority.pubkey().to_bytes(),
			"the oracle authority is rotated on-chain"
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
// examples/prop_amm_program --filter audit_sec_`.
// ---------------------------------------------------------------------------

/// SEC-32: `RotateAuthority` changes the stored authority, but `Update`
/// still authorizes the immutable global `UPDATE_AUTHORITY` constant, so the
/// advertised rotation cannot transfer publishing power and the old global
/// key remains authorized forever. After a rotation, the new authority must
/// be able to publish and the previous one must not.
///
/// Current behavior: the rotated-in authority cannot update, so the `expect`
/// below fails and the test proves the rotation is inert.
#[test]
#[ignore = "run with pina test"]
fn audit_sec_32_update_follows_the_rotated_authority() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let payer = program.payer();
		let oracle = Keypair::new_from_array([2; 32]);

		program
			.send_with_signers(
				initialize_instruction(&program, &payer, &oracle.pubkey()),
				&[&oracle],
			)
			.expect("execute Initialize");

		let new_authority = Keypair::new_from_array([3; 32]);
		program
			.send_instruction(rotate_instruction(
				&program,
				&oracle.pubkey(),
				&payer,
				&new_authority.pubkey(),
			))
			.expect("rotate to the new authority");

		// The rotated-in authority publishes a new price.
		program
			.send_with_signers(
				update_instruction(
					&program,
					&oracle.pubkey(),
					&new_authority.pubkey(),
					1_234_567_u64,
				),
				&[&new_authority],
			)
			.expect("the rotated-in authority must be able to publish prices");

		let account = program.account(&oracle.pubkey()).expect("fetch oracle");
		assert_eq!(
			u64::from_le_bytes(account.data[34..42].try_into().expect("price")),
			1_234_567,
			"the rotated-in authority's price landed on-chain"
		);

		program.stop().expect("stop isolated program test");
	});
}
