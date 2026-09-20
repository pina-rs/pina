#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::Keypair;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Signer;
use program_under_test::FloatError;
use program_under_test::FloatInstruction;
use program_under_test::ID;

fn create_instruction(
	program: &ProgramTest,
	account: &Pubkey,
	authority: &Pubkey,
	f32_bits: u32,
	f64_bits: u64,
) -> pina_test::Instruction {
	// discriminator + migration version, then the f32 and f64 bit patterns.
	let mut data = vec![FloatInstruction::Create as u8, 0u8];
	data.extend_from_slice(&f32_bits.to_le_bytes());
	data.extend_from_slice(&f64_bits.to_le_bytes());

	program.instruction(
		&data,
		// The fresh account signs its own create-account CPI.
		vec![
			AccountMeta::new(*account, true),
			AccountMeta::new_readonly(*authority, true),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

fn update_instruction(
	program: &ProgramTest,
	account: &Pubkey,
	authority: &Pubkey,
	f32_bits: u32,
	f64_bits: u64,
) -> pina_test::Instruction {
	let mut data = vec![FloatInstruction::Update as u8, 0u8];
	data.extend_from_slice(&f32_bits.to_le_bytes());
	data.extend_from_slice(&f64_bits.to_le_bytes());

	program.instruction(
		&data,
		vec![
			AccountMeta::new(*account, false),
			AccountMeta::new_readonly(*authority, true),
		],
	)
}

/// Create stores f32/f64 as bit patterns; the exact bytes land on-chain.
#[test]
#[ignore = "run with pina test"]
fn create_roundtrips_float_bit_patterns() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let account = Keypair::new_from_array([2; 32]);

		let e_half = std::f32::consts::E;
		let e_full = std::f64::consts::E;

		program
			.send_with_signers(
				create_instruction(
					&program,
					&account.pubkey(),
					&authority,
					e_half.to_bits(),
					e_full.to_bits(),
				),
				&[&account],
			)
			.expect("execute Create");

		let raw = program
			.account(&account.pubkey())
			.expect("fetch float account");
		assert_eq!(raw.owner, program_id);
		assert_eq!(raw.data.len(), 46, "FloatDataAccount layout is 46 bytes");
		assert_eq!(raw.data[0], 1, "account discriminator is FloatDataAccount");
		assert_eq!(raw.data[1], 0, "stored migration version is current");
		// The PinaPod wire view stores the u64 before the u32.
		assert_eq!(
			&raw.data[2..10],
			e_full.to_bits().to_le_bytes(),
			"f64 bits stored"
		);
		assert_eq!(
			&raw.data[10..14],
			e_half.to_bits().to_le_bytes(),
			"f32 bits stored"
		);
		assert_eq! {
			&raw.data[14..46],
			authority.as_ref(),
			"stored authority matches"
		};

		program.stop().expect("stop isolated program test");
	});
}

/// Update replaces both floats through the authority channel.
#[test]
#[ignore = "run with pina test"]
fn update_replaces_floats() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let account = Keypair::new_from_array([2; 32]);

		program
			.send_with_signers(
				create_instruction(
					&program,
					&account.pubkey(),
					&authority,
					std::f32::consts::PI.to_bits(),
					std::f64::consts::PI.to_bits(),
				),
				&[&account],
			)
			.expect("execute Create");

		let new_f32 = -0.5_f32;
		let new_f64 = std::f64::consts::SQRT_2;
		program
			.send_instruction(update_instruction(
				&program,
				&account.pubkey(),
				&authority,
				new_f32.to_bits(),
				new_f64.to_bits(),
			))
			.expect("execute Update");

		let raw = program
			.account(&account.pubkey())
			.expect("fetch float account");
		assert_eq!(&raw.data[2..10], new_f64.to_bits().to_le_bytes());
		assert_eq!(&raw.data[10..14], new_f32.to_bits().to_le_bytes());

		program.stop().expect("stop isolated program test");
	});
}

/// Any other signer is rejected by the authority check.
#[test]
#[ignore = "run with pina test"]
fn update_rejects_a_stranger_signer() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let account = Keypair::new_from_array([2; 32]);

		program
			.send_with_signers(
				create_instruction(
					&program,
					&account.pubkey(),
					&authority,
					1.0_f32.to_bits(),
					2.0_f64.to_bits(),
				),
				&[&account],
			)
			.expect("execute Create");

		let stranger = Keypair::new_from_array([3; 32]);
		program
			.fund(&stranger.pubkey(), 1_000_000_000)
			.expect("fund stranger");

		let mut payload = vec![FloatInstruction::Update as u8, 0u8];
		payload.extend_from_slice(&3.0_f32.to_bits().to_le_bytes());
		payload.extend_from_slice(&4.0_f64.to_bits().to_le_bytes());

		let instruction = program.instruction(
			&payload,
			vec![
				AccountMeta::new(account.pubkey(), false),
				AccountMeta::new_readonly(stranger.pubkey(), true),
			],
		);
		let error = program
			.send_with_signers(instruction, &[&stranger])
			.expect_err("a stranger cannot update floats");
		assert_eq!(error.operation(), "execute program instruction");
		eprintln!("stranger update error: {}", error.message());

		program.stop().expect("stop isolated program test");
	});
}

/// Create rejects non-finite payloads: quiet NaN, signaling NaN with a
/// payload, and ±Inf all die with `NonFiniteFloat` and never create an
/// account.
#[test]
#[ignore = "run with pina test"]
fn create_rejects_non_finite_floats() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();

		// Quiet NaN in both widths.
		let nan_account = Keypair::new_from_array([4; 32]);
		let error = program
			.send_with_signers(
				create_instruction(
					&program,
					&nan_account.pubkey(),
					&authority,
					f32::NAN.to_bits(),
					f64::NAN.to_bits(),
				),
				&[&nan_account],
			)
			.expect_err("a quiet-NaN payload must not create an account");
		pina_test::assert_custom_error(&error, FloatError::NonFiniteFloat as u32);

		// Signaling NaN with a nonzero payload (raw bit patterns).
		let snan_account = Keypair::new_from_array([5; 32]);
		let error = program
			.send_with_signers(
				create_instruction(
					&program,
					&snan_account.pubkey(),
					&authority,
					0x7f80_0001u32,
					0x7ff0_0000_0000_0001u64,
				),
				&[&snan_account],
			)
			.expect_err("a signaling-NaN payload must not create an account");
		pina_test::assert_custom_error(&error, FloatError::NonFiniteFloat as u32);

		// ±Infinity.
		let inf_account = Keypair::new_from_array([6; 32]);
		let error = program
			.send_with_signers(
				create_instruction(
					&program,
					&inf_account.pubkey(),
					&authority,
					f32::INFINITY.to_bits(),
					f64::NEG_INFINITY.to_bits(),
				),
				&[&inf_account],
			)
			.expect_err("an infinite payload must not create an account");
		pina_test::assert_custom_error(&error, FloatError::NonFiniteFloat as u32);

		program.stop().expect("stop isolated program test");
	});
}

/// Update rejects non-finite payloads: overwriting finite stored values with
/// NaN or +Inf fails with `NonFiniteFloat` and leaves the stored bits alone.
#[test]
#[ignore = "run with pina test"]
fn update_rejects_non_finite_floats() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let account = Keypair::new_from_array([2; 32]);

		program
			.send_with_signers(
				create_instruction(
					&program,
					&account.pubkey(),
					&authority,
					1.0_f32.to_bits(),
					2.0_f64.to_bits(),
				),
				&[&account],
			)
			.expect("execute Create");

		let error = program
			.send_instruction(update_instruction(
				&program,
				&account.pubkey(),
				&authority,
				f32::NAN.to_bits(),
				f64::INFINITY.to_bits(),
			))
			.expect_err("non-finite payloads must not overwrite stored values");
		pina_test::assert_custom_error(&error, FloatError::NonFiniteFloat as u32);

		// The stored values are untouched by the rejected update.
		let raw = program
			.account(&account.pubkey())
			.expect("fetch float account");
		assert_eq!(
			&raw.data[2..10],
			2.0_f64.to_bits().to_le_bytes(),
			"f64 bits unchanged"
		);
		assert_eq!(
			&raw.data[10..14],
			1.0_f32.to_bits().to_le_bytes(),
			"f32 bits unchanged"
		);

		program.stop().expect("stop isolated program test");
	});
}

/// Finite edge values are accepted bit-exact: −0.0 and the minimum
/// subnormals land on-chain, and a normal value still round-trips through
/// Update afterwards.
#[test]
#[ignore = "run with pina test"]
fn finite_edge_values_are_accepted_bit_exact() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let authority = program.payer();
		let account = Keypair::new_from_array([7; 32]);

		let neg_zero_f32 = -0.0_f32;
		let min_subnormal_f32 = f32::from_bits(1);
		let min_subnormal_f64 = f64::from_bits(1);

		program
			.send_with_signers(
				create_instruction(
					&program,
					&account.pubkey(),
					&authority,
					min_subnormal_f32.to_bits(),
					min_subnormal_f64.to_bits(),
				),
				&[&account],
			)
			.expect("the minimum subnormals are finite");

		let raw = program
			.account(&account.pubkey())
			.expect("fetch float account");
		assert_eq!(
			&raw.data[2..10],
			min_subnormal_f64.to_bits().to_le_bytes(),
			"min-subnormal f64 stored bit-exact"
		);
		assert_eq!(
			&raw.data[10..14],
			min_subnormal_f32.to_bits().to_le_bytes(),
			"min-subnormal f32 stored bit-exact"
		);

		// −0.0 is finite and accepted over Update.
		program
			.send_instruction(update_instruction(
				&program,
				&account.pubkey(),
				&authority,
				neg_zero_f32.to_bits(),
				(-0.0_f64).to_bits(),
			))
			.expect("−0.0 is finite");

		let raw = program
			.account(&account.pubkey())
			.expect("fetch float account");
		assert_eq!(
			&raw.data[2..10],
			(-0.0_f64).to_bits().to_le_bytes(),
			"−0.0 f64 stored bit-exact"
		);
		assert_eq!(
			&raw.data[10..14],
			neg_zero_f32.to_bits().to_le_bytes(),
			"−0.0 f32 stored bit-exact"
		);

		// A normal value still round-trips afterwards.
		program
			.send_instruction(update_instruction(
				&program,
				&account.pubkey(),
				&authority,
				1.5_f32.to_bits(),
				2.5_f64.to_bits(),
			))
			.expect("execute Update with finite values");
		let raw = program
			.account(&account.pubkey())
			.expect("fetch float account");
		assert_eq!(&raw.data[2..10], 2.5_f64.to_bits().to_le_bytes());
		assert_eq!(&raw.data[10..14], 1.5_f32.to_bits().to_le_bytes());

		program.stop().expect("stop isolated program test");
	});
}
