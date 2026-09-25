#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::Keypair;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Signer;
use program_under_test::ID;
use program_under_test::UPDATE_AUTHORITY;

/// The committed fixture seed: the 32 ASCII bytes of a descriptive phrase,
/// documented on `UPDATE_AUTHORITY` in `src/lib.rs`. Deterministic, and
/// outside the uniform-seed brute-force space the old `[7u8; 32]` fixture
/// lived in.
const UPDATE_AUTHORITY_SEED: [u8; 32] = *b"pina example fixture updater key";

fn fixture_update_authority() -> Keypair {
	let keypair = Keypair::new_from_array(UPDATE_AUTHORITY_SEED);
	assert_eq!(
		keypair.pubkey(),
		UPDATE_AUTHORITY,
		"the committed fixture seed must keep deriving UPDATE_AUTHORITY"
	);
	keypair
}
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

/// The oracle's recorded authority can update the price. At initialize that is
/// the payer, and the gate stays narrow: a key that is neither the static
/// updater nor the recorded authority is still refused.
#[test]
#[ignore = "run with pina test"]
fn update_requires_the_recorded_or_the_static_authority() {
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

		// `payer` is the stored authority, so its update is accepted.
		program
			.send_instruction(update_instruction(&program, &oracle.pubkey(), &payer, 99))
			.expect("the recorded oracle authority may update");
		let account = program.account(&oracle.pubkey()).expect("fetch oracle");
		assert_eq!(
			&account.data[34..42],
			&99_u64.to_le_bytes(),
			"the recorded authority wrote the new price on-chain"
		);
		drop(account);

		// A stranger — neither the stored authority nor the static updater —
		// is still refused, so the stored-authority branch does not widen the
		// gate to everyone.
		let stranger = Keypair::new_from_array([4; 32]);
		program
			.fund(&stranger.pubkey(), 1_000_000_000)
			.expect("fund stranger");
		let error = program
			.send_with_signers(
				update_instruction(&program, &oracle.pubkey(), &stranger.pubkey(), 123),
				&[&stranger],
			)
			.expect_err("a stranger may not update");
		pina_test::assert_custom_error(
			&error,
			program_under_test::PropAmmError::UnauthorizedUpdateAuthority as u32,
		);
		// The refused update wrote nothing.
		let account = program.account(&oracle.pubkey()).expect("fetch oracle");
		assert_eq!(
			&account.data[34..42],
			&99_u64.to_le_bytes(),
			"a refused update leaves the price untouched"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// Rotate hands the oracle authority to another wallet, which then owns
/// accounting — including the price updates the rotation implies.
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
		drop(account);

		// The rotated-in authority now publishes prices: rotation grants the
		// capability it implies rather than being cosmetic.
		program
			.send_with_signers(
				update_instruction(&program, &oracle.pubkey(), &new_authority.pubkey(), 4_242),
				&[&new_authority],
			)
			.expect("the rotated-in authority publishes a price");
		let account = program.account(&oracle.pubkey()).expect("fetch oracle");
		assert_eq!(
			&account.data[34..42],
			&4_242_u64.to_le_bytes(),
			"the rotated-in authority wrote the new price on-chain"
		);
		drop(account);

		// The key it replaced is no longer the stored authority and is not the
		// static updater, so it has no update capability left.
		let error = program
			.send_instruction(update_instruction(
				&program,
				&oracle.pubkey(),
				&payer,
				1_111,
			))
			.expect_err("the rotated-away authority may no longer update");
		pina_test::assert_custom_error(
			&error,
			program_under_test::PropAmmError::UnauthorizedUpdateAuthority as u32,
		);
		let account = program.account(&oracle.pubkey()).expect("fetch oracle");
		assert_eq!(
			&account.data[34..42],
			&4_242_u64.to_le_bytes(),
			"the rotated-away key wrote nothing"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// The rebuilt fixture update authority — deterministic from the committed
/// seed, and no longer recoverable from a uniform one — can still sign a
/// price update, so the D1 reseed introduced no functional regression.
#[test]
#[ignore = "run with pina test"]
fn fixture_update_authority_still_publishes_prices() {
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

		let update_authority = fixture_update_authority();
		program
			.fund(&update_authority.pubkey(), 1_000_000_000)
			.expect("fund the fixture update authority");

		program
			.send_with_signers(
				update_instruction(
					&program,
					&oracle.pubkey(),
					&update_authority.pubkey(),
					999_999,
				),
				&[&update_authority],
			)
			.expect("the fixture update authority signs the price update");

		let account = program.account(&oracle.pubkey()).expect("fetch oracle");
		assert_eq!(
			&account.data[34..42],
			&999_999_u64.to_le_bytes(),
			"the authorized update wrote the new price on-chain"
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
