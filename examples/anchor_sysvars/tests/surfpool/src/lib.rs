#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use program_under_test::ID;
use program_under_test::SysvarsInstruction;

/// Sysvar addresses used by the example, matching the program's constants.
const CLOCK: &str = "SysvarC1ock11111111111111111111111111111111";
const RENT: &str = "SysvarRent111111111111111111111111111111111";
const STAKE_HISTORY: &str = "SysvarStakeHistory1111111111111111111111111";

fn sysvar_instruction(program: &ProgramTest, clock: &Pubkey) -> pina_test::Instruction {
	program.instruction(
		&[SysvarsInstruction::Sysvars as u8],
		vec![
			AccountMeta::new_readonly(*clock, false),
			AccountMeta::new_readonly(Pubkey::from_str_const(RENT), false),
			AccountMeta::new_readonly(Pubkey::from_str_const(STAKE_HISTORY), false),
		],
	)
}

/// Real sysvars are available in the isolated surfnet at the canonical
/// addresses; the program reads and validates all three.
#[test]
#[ignore = "run with pina test"]
fn validates_real_sysvars() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		program
			.send_instruction(sysvar_instruction(&program, &Pubkey::from_str_const(CLOCK)))
			.expect("execute Sysvars with real sysvars");

		// Sysvars are read-only and repeatable.
		program
			.send_instruction(sysvar_instruction(&program, &Pubkey::from_str_const(CLOCK)))
			.expect("Sysvars is repeatable");

		program.stop().expect("stop isolated program test");
	});
}

/// Swapping the clock sysvar for an unrelated funded account is rejected by
/// the program's sysvar guard.
#[test]
#[ignore = "run with pina test"]
fn rejects_a_bogus_clock_account() {
	pina_test::run(async {
		let program_id = Pubkey::new_from_array(ID.to_bytes());
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");

		let impostor = Pubkey::new_unique();
		program
			.fund(&impostor, 1_000_000_000)
			.expect("fund impostor");

		let error = program
			.send_instruction(sysvar_instruction(&program, &impostor))
			.expect_err("a non-sysvar account must fail the clock guard");

		assert_eq!(error.operation(), "execute program instruction");
		eprintln!("bogus clock error: {}", error.message());

		program.stop().expect("stop isolated program test");
	});
}
