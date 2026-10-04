//! The generated compute unit limit, proven against the deployed program.
//!
//! `compute-units.json` records what `increment` consumed in the Surfpool
//! suite, and `pina generate` turned it into
//! `INCREMENT_COMPUTE_UNIT_LIMIT`. This sends real transactions built entirely
//! from the generated Rust client: the instruction, the limit constant, and
//! the `SetComputeUnitLimit` helper.
//!
//! It lives outside the suite's library target on purpose. `pina test
//! --record-compute-units` and the benchmark harness run only that target, so
//! a program that outgrows its recorded limit fails here without blocking the
//! recording that would fix it.
//!
//! ```sh
//! PINA_SBF_ARTIFACT=target/surfpool/examples/counter_program.so \
//!   cargo test -p counter-program-surfpool-tests --test compute_unit_budget -- --ignored
//! ```

use counter_program_client::compute_budget::COMPUTE_BUDGET_PROGRAM_ID;
use counter_program_client::instructions::INCREMENT_COMPUTE_UNIT_LIMIT;
use counter_program_client::instructions::Increment;
use counter_program_client::instructions::IncrementInstructionData;
use counter_program_client::instructions::Initialize;
use counter_program_client::instructions::InitializeInstructionData;
use counter_program_client::programs::COUNTER_PROGRAM_ID;
use counter_program_client::set_compute_unit_limit_instruction;
use pina_test::Instruction;
use pina_test::InstructionError;
use pina_test::ProgramTest;
use pina_test::Pubkey;
use pina_test::TestError;
use pina_test::TransactionError;

/// Compute units the `SetComputeUnitLimit` instruction itself consumes,
/// matching `pina_cli::compute_units::SET_COMPUTE_UNIT_LIMIT_COST`.
const SET_COMPUTE_UNIT_LIMIT_COST: u32 = 150;

/// Compute units the `SetComputeUnitPrice` instruction consumes, matching
/// `pina_cli::compute_units::SET_COMPUTE_UNIT_PRICE_COST`.
const SET_COMPUTE_UNIT_PRICE_COST: u32 = 150;

/// `ComputeBudgetInstruction::SetComputeUnitPrice(micro_lamports)`, the
/// priority fee every limit reserves room for.
fn set_compute_unit_price_instruction(micro_lamports: u64) -> Instruction {
	let mut data = vec![3];
	data.extend_from_slice(&micro_lamports.to_le_bytes());

	Instruction::new_with_bytes(COMPUTE_BUDGET_PROGRAM_ID, &data, Vec::new())
}

fn increment(authority: Pubkey) -> Instruction {
	let data = IncrementInstructionData::new(|_data| {})
		.unwrap_or_else(|error| panic!("encode Increment: {error}"));

	Increment::new(authority).instruction(data)
}

/// Whether a transaction failed because the program instruction at
/// `program_index` ran out of the compute units the limit allowed.
///
/// An SBF program that exhausts the meter fails to complete, and the runtime
/// logs why; a builtin would report `ComputationalBudgetExceeded` instead.
fn exhausted_its_budget(error: &TestError, program_index: u8) -> bool {
	let failed_to_complete = matches!(
		error.transaction_error(),
		Some(TransactionError::InstructionError(
			index,
			InstructionError::ProgramFailedToComplete
		)) if index == program_index
	);

	failed_to_complete && error.message().contains("exceeded CUs meter")
}

#[test]
#[ignore = "needs PINA_SBF_ARTIFACT naming a counter_program SBF build"]
fn the_generated_limit_covers_increment_and_the_measurement_is_exact() {
	pina_test::run(async {
		let mut program = ProgramTest::start(COUNTER_PROGRAM_ID)
			.await
			.unwrap_or_else(|error| panic!("start isolated program test: {error}"));
		let authority = program.payer();
		let initialize = Initialize::new(authority);
		let (_, bump) =
			Pubkey::find_program_address(&[b"counter", authority.as_ref()], &COUNTER_PROGRAM_ID);
		let data = InitializeInstructionData::new(|data| data.bump = bump)
			.unwrap_or_else(|error| panic!("encode Initialize: {error}"));

		program
			.send_instruction(initialize.instruction(data))
			.unwrap_or_else(|error| panic!("execute Initialize: {error}"));

		// What `increment` consumes against this build, measured the way
		// `pina test --record-compute-units` measures it.
		let measured = u32::try_from(
			program
				.simulate_compute_units(&[increment(authority)], &[])
				.unwrap_or_else(|error| panic!("measure Increment: {error}")),
		)
		.unwrap_or_else(|error| panic!("a measurement fits u32: {error}"));
		let exact = measured + SET_COMPUTE_UNIT_LIMIT_COST;

		// The generated limit lands the transaction.
		program
			.send_instructions(
				&[
					set_compute_unit_limit_instruction(INCREMENT_COMPUTE_UNIT_LIMIT),
					increment(authority),
				],
				&[],
			)
			.unwrap_or_else(|error| panic!("Increment under its generated limit: {error}"));

		// So does the measurement plus the limit instruction's own cost, and one
		// unit less exhausts the budget: the measurement is exact.
		program
			.send_instructions(
				&[
					set_compute_unit_limit_instruction(exact),
					increment(authority),
				],
				&[],
			)
			.unwrap_or_else(|error| panic!("Increment under its exact limit: {error}"));
		let error = program
			.send_instructions(
				&[
					set_compute_unit_limit_instruction(exact - 1),
					increment(authority),
				],
				&[],
			)
			.expect_err("a limit one unit below the measurement must fail");
		assert!(
			exhausted_its_budget(&error, 1),
			"expected the budget to run out, got {error}"
		);

		// A priority fee instruction costs as much again, and the generated
		// limit still covers both.
		let with_price = exact + SET_COMPUTE_UNIT_PRICE_COST;
		assert!(
			with_price <= INCREMENT_COMPUTE_UNIT_LIMIT,
			"the generated limit {INCREMENT_COMPUTE_UNIT_LIMIT} must cover {with_price} units"
		);
		let error = program
			.send_instructions(
				&[
					set_compute_unit_limit_instruction(with_price - 1),
					set_compute_unit_price_instruction(1),
					increment(authority),
				],
				&[],
			)
			.expect_err("a limit one unit below the priced measurement must fail");
		assert!(
			exhausted_its_budget(&error, 2),
			"expected the budget to run out, got {error}"
		);
		program
			.send_instructions(
				&[
					set_compute_unit_limit_instruction(INCREMENT_COMPUTE_UNIT_LIMIT),
					set_compute_unit_price_instruction(1),
					increment(authority),
				],
				&[],
			)
			.unwrap_or_else(|error| panic!("priced Increment under its generated limit: {error}"));

		program
			.stop()
			.unwrap_or_else(|error| panic!("stop isolated program test: {error}"));
	});
}
