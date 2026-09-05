#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use program_under_test::ID;
use program_under_test::PinaBpfInstruction;
use program_under_test::SEED_STATE_PREFIX;

fn state_pda(program_id: &Pubkey) -> (Pubkey, u8) {
	Pubkey::find_program_address(&[SEED_STATE_PREFIX], program_id)
}

fn hello(program: &ProgramTest) -> pina_test::Instruction {
	program.instruction(&[PinaBpfInstruction::Hello as u8], vec![])
}

fn create_pda(
	program: &ProgramTest,
	payer: &Pubkey,
	state: &Pubkey,
	bump: u8,
) -> pina_test::Instruction {
	program.instruction(
		&[PinaBpfInstruction::CreatePda as u8, bump],
		vec![
			AccountMeta::new_readonly(*payer, true),
			AccountMeta::new(*state, false),
			AccountMeta::new_readonly(Pubkey::default(), false),
		],
	)
}

/// The Hello path is stateless and repeatedly confirms.
#[test]
#[ignore = "run with pina test"]
fn hello_confirms_repeatedly() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		for _ in 0..3 {
			program
				.send_instruction(hello(&program))
				.expect("execute Hello");
		}

		program.stop().expect("stop isolated program test");
	});
}

/// Everything except `Hello` is feature-gated behind `cpi-runtime-tests`.
/// The artifact under `pina test` is built without that feature, so CreatePda
/// and the forwarded CPI paths must return the documented
/// InvalidInstructionData error instead of performing on-chain writes.
#[test]
#[ignore = "run with pina test"]
fn gated_instructions_report_invalid_instructions_without_the_feature() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let (state, bump) = state_pda(&program_id);

		// CreatePda is gated.
		let error = program
			.send_instruction(create_pda(&program, &program.payer(), &state, bump))
			.expect_err("CreatePda is feature-gated off");
		assert_eq!(error.operation(), "execute program instruction");
		eprintln!("gated create-pda error: {}", error.message());
		assert!(
			program.account(&state).is_err(),
			"the gated instruction must not create the state PDA"
		);

		// The forwarded CPI path is gated too.
		let foreign_oracle = Pubkey::new_unique();
		program
			.fund(&foreign_oracle, 1_000_000_000)
			.expect("fund oracle stub");
		let mut data = vec![PinaBpfInstruction::ForwardRotateWithSigner as u8];
		data.extend_from_slice(Pubkey::default().as_ref());
		let instruction = program.instruction(
			&data,
			vec![
				AccountMeta::new(foreign_oracle, false),
				AccountMeta::new_readonly(program.payer(), true),
				AccountMeta::new_readonly(Pubkey::default(), false),
			],
		);
		let error = program
			.send_instruction(instruction)
			.expect_err("forwarded CPI is feature-gated off");
		assert_eq!(error.operation(), "execute program instruction");

		program.stop().expect("stop isolated program test");
	});
}
