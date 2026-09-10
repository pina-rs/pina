// normalize-stderr-test: "\n$" -> ""
// aux-build: pinocchio_token.rs
// aux-build: pina.rs

#![allow(
	dead_code,
	unused_assignments,
	unused_variables,
	unused_mut,
	unreachable_code,
	unused_parens
)]

extern crate pina;
extern crate pinocchio_token;

use pina::AccountInfoValidation;
use pina::ProgramAccount as PinaProgramAccount;
use pinocchio_token::Address;
use pinocchio_token::Instruction;

static TOKEN_ID: Address = Address;

type ProgramAccount = PinaProgramAccount<Address>;

struct CpiBatch<'a> {
	instruction: &'a Instruction,
	outcome: Result<(), ()>,
}

struct ProgramHolder<'a> {
	program: &'a ProgramAccount,
}

fn process_field_proof_and_base_reassignment(
	instruction: &Instruction,
	checked: &ProgramAccount,
	attacker: &ProgramAccount,
) -> Result<(), ()> {
	let mut holder = ProgramHolder { program: checked };
	holder.program.assert_program(&TOKEN_ID)?;
	instruction.invoke_with_unverified_program(holder.program.address())?;

	holder = ProgramHolder { program: attacker };
	// Reassigning the base invalidates the proven field place.
	instruction.invoke_with_unverified_program(holder.program.address())?;
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
	Ok(())
}

fn process_guarded_match_proof_does_not_dominate(
	instruction: &Instruction,
	program: &ProgramAccount,
	condition: bool,
	other: bool,
) -> Result<(), ()> {
	match condition {
		true if other => {
			program.assert_program(&TOKEN_ID)?;
		}
		_ => {}
	}
	instruction.invoke_with_unverified_program(program.address())?;
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
	Ok(())
}

fn process_all_diverging_match(
	instruction: &Instruction,
	program: &ProgramAccount,
	condition: bool,
) -> Result<(), ()> {
	program.assert_program(&TOKEN_ID)?;
	match condition {
		true => return Ok(()),
		false => return Err(()),
	}
}

fn process_loop_invocation(
	instruction: &Instruction,
	program: &ProgramAccount,
	rounds: u32,
) -> Result<(), ()> {
	let mut remaining = rounds;
	loop {
		if remaining == 0 {
			break;
		}
		instruction.invoke_with_unverified_program(program.address())?;
		//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
		remaining -= 1;
	}
	Ok(())
}

fn process_closure_ignores_outside_proof(
	instruction: &Instruction,
	program: &ProgramAccount,
) -> Result<(), ()> {
	program.assert_program(&TOKEN_ID)?;
	let invoke = || instruction.invoke_with_unverified_program(program.address());
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
	invoke()
}

fn process_unproven_closure_invocation(
	instruction: &Instruction,
	program: &ProgramAccount,
) -> Result<(), ()> {
	let invoke = || instruction.invoke_with_unverified_program(program.address());
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
	invoke()
}

fn process_short_circuit_proof_does_not_dominate(
	instruction: &Instruction,
	program: &ProgramAccount,
	condition: bool,
) -> Result<(), ()> {
	let _ = condition && {
		program.assert_program(&TOKEN_ID)?;
		true
	};
	instruction.invoke_with_unverified_program(program.address())?;
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
	Ok(())
}

fn process_assignment_invalidates(
	instruction: &Instruction,
	checked: &ProgramAccount,
	attacker: &ProgramAccount,
) -> Result<(), ()> {
	let mut program = checked;
	program.assert_program(&TOKEN_ID)?;
	program = attacker;
	instruction.invoke_with_unverified_program(program.address())?;
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
	Ok(())
}

fn process_reborrow_invalidates(
	instruction: &Instruction,
	checked: &ProgramAccount,
	attacker: &ProgramAccount,
) -> Result<(), ()> {
	let mut program = checked;
	program.assert_program(&TOKEN_ID)?;
	let reborrow = &mut program;
	*reborrow = attacker;
	instruction.invoke_with_unverified_program(program.address())?;
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
	Ok(())
}

fn process_wrapper_expressions(
	instruction: &Instruction,
	program: &ProgramAccount,
	table: &[u8],
	index: usize,
) -> Result<(), ()> {
	let outcomes = (
		instruction.invoke_with_unverified_program(program.address()),
		//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
		instruction.invoke_signed_with_unverified_program(&[], program.address()),
		//~^ ERROR: `.invoke_signed_with_unverified_program()` called without a preceding program address verification
	);
	let _ = outcomes;

	let repeated = [instruction.invoke_with_unverified_program(program.address()); 2];
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
	let _ = repeated;

	let batch = CpiBatch {
		instruction,
		outcome: instruction.invoke_with_unverified_program(program.address()),
	};
	//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
	let _ = batch;

	let rebatch = CpiBatch {
		outcome: Ok(()),
		..batch
	};
	let _ = rebatch;

	let _ = table[index];
	Ok(())
}

fn process_let_chain_and_block_statements(
	instruction: &Instruction,
	program: &ProgramAccount,
	maybe: Option<u64>,
	proven: bool,
) -> Result<(), ()> {
	const MAX_RETRIES: u8 = 3;
	let _ = MAX_RETRIES;

	let checked: &ProgramAccount;
	if proven {
		checked = program;
	} else {
		return Err(());
	}

	if let Some(slot) = maybe
		&& slot > 0
	{
		checked.assert_program(&TOKEN_ID)?;
		instruction.invoke_with_unverified_program(checked.address())?;
	}
	Ok(())
}

fn process_return_value_position(
	instruction: &Instruction,
	program: &ProgramAccount,
) -> Result<(), ()> {
	let invoke = || {
		return instruction.invoke_with_unverified_program(program.address());
		//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
	};
	let _ = invoke();
	Ok(())
}

fn process_break_value_position(
	instruction: &Instruction,
	program: &ProgramAccount,
) -> Result<(), ()> {
	let outcome = loop {
		break instruction.invoke_with_unverified_program(program.address());
		//~^ ERROR: `.invoke_with_unverified_program()` called without a preceding program address verification
	};
	let _ = outcome;
	Ok(())
}

fn main() {}

// compile-fail
