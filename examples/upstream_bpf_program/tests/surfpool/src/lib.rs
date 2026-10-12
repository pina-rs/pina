#![cfg(test)]

use pina_test::AccountMeta;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::Signer;

// The program is a cdylib only (see ../../Cargo.toml), so its real types
// come in through a source include rather than an rlib dependency.
#[path = "../../../src/lib.rs"]
mod program;

use program::ID;
use program::UpstreamInstruction;

/// Sysvar address matching the program's `CLOCK_SYSVAR_ID`.
const CLOCK: &str = "SysvarC1ock11111111111111111111111111111111";

/// Fixed identity so recorded benchmarks stay deterministic.
fn caller() -> Pubkey {
	Pubkey::new_from_array(ID.to_bytes())
}

#[test]
#[ignore = "run with pina test"]
fn hello_logs_the_greeting() {
	pina_test::run(async {
		let program_id = caller();
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");
		let user = program.payer();

		// Send first so the benchmark harness records a compute-unit sample.
		program
			.send(
				&[UpstreamInstruction::Hello as u8],
				vec![AccountMeta::new_readonly(user, true)],
			)
			.expect("execute Hello");

		let logs = program
			.simulate_logs(
				&[UpstreamInstruction::Hello as u8],
				vec![AccountMeta::new_readonly(user, true)],
			)
			.expect("simulate Hello");
		assert!(
			logs.iter().any(|log| log.contains("Hello, upstream BPF!")),
			"expected the greeting in the logs: {logs:?}"
		);

		program.stop().expect("stop isolated program test");
	});
}

#[test]
#[ignore = "run with pina test"]
fn ping_reports_the_clock_slot() {
	pina_test::run(async {
		let program_id = caller();
		let mut program = ProgramTest::start(program_id)
			.await
			.expect("start isolated program test");
		let user = program.payer();

		// Send first so the benchmark harness records a compute-unit sample.
		program
			.send(
				&[UpstreamInstruction::Ping as u8],
				vec![
					AccountMeta::new_readonly(user, true),
					AccountMeta::new_readonly(Pubkey::from_str_const(CLOCK), false),
				],
			)
			.expect("execute Ping");

		// The program validates the sysvar account and reads it zero-copy, so
		// the static marker proves the account reached the processor intact
		// under both toolchains. The slot value itself is deliberately not in
		// the log: the format-arg form of `log!` would link `core::fmt`.
		let logs = program
			.simulate_logs(
				&[UpstreamInstruction::Ping as u8],
				vec![
					AccountMeta::new_readonly(user, true),
					AccountMeta::new_readonly(Pubkey::from_str_const(CLOCK), false),
				],
			)
			.expect("simulate Ping");
		assert!(
			logs.iter().any(|log| log.contains("clock sysvar read")),
			"expected the clock sysvar marker in the logs: {logs:?}"
		);

		program.stop().expect("stop isolated program test");
	});
}
