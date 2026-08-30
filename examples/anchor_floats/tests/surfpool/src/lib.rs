#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::Keypair;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Signer;
use program_under_test::FloatInstruction;
use program_under_test::ID;

fn create_instruction(
	program: &ProgramTest,
	account: &Pubkey,
	authority: &Pubkey,
	f32_bits: u32,
	f64_bits: u64,
) -> pina_test::Instruction {
	let mut data = vec![FloatInstruction::Create as u8];
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
	let mut data = vec![FloatInstruction::Update as u8];
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
		let account = Keypair::new();

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
		assert_eq!(raw.data.len(), 45, "FloatDataAccount layout is 45 bytes");
		assert_eq!(raw.data[0], 1, "account discriminator is FloatDataAccount");
		// The zeropod wire view stores the u64 before the u32.
		assert_eq!(
			&raw.data[1..9],
			e_full.to_bits().to_le_bytes(),
			"f64 bits stored"
		);
		assert_eq!(
			&raw.data[9..13],
			e_half.to_bits().to_le_bytes(),
			"f32 bits stored"
		);
		assert_eq! {
			&raw.data[13..45],
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
		let account = Keypair::new();

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
		assert_eq!(&raw.data[1..9], new_f64.to_bits().to_le_bytes());
		assert_eq!(&raw.data[9..13], new_f32.to_bits().to_le_bytes());

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
		let account = Keypair::new();

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

		let stranger = Keypair::new();
		program
			.fund(&stranger.pubkey(), 1_000_000_000)
			.expect("fund stranger");

		let mut payload = vec![FloatInstruction::Update as u8];
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
