// aux-build: pina.rs
// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

extern crate pina;

use pina::parse_instruction;

#[derive(Debug, Default)]
enum Instruction {
	#[default]
	Initialize,
	Update,
}

enum Mode {
	Fast,
	Safe,
}

fn process_a() -> Result<(), ()> {
	Ok(())
}

fn process_b() -> Result<(), ()> {
	Ok(())
}

fn entrypoint(data: &[u8]) -> Result<(), ()> {
	match parse_instruction::<Instruction>(data)? {
		Instruction::Initialize => process_a(),
		Instruction::Update => process_b(),
	}
}

fn entrypoint_with_local(data: &[u8]) -> Result<(), ()> {
	let instruction: Instruction = parse_instruction(data)?;
	match instruction {
		Instruction::Initialize => process_a(),
		Instruction::Update => process_b(),
	}
}

fn dispatch(data: &[u8]) -> Result<(), ()> {
	if data.first() == Some(&0) {
		process_a()
	} else {
		process_b()
	}
}

fn entrypoint_helper(data: &[u8]) -> Result<(), ()> {
	// The match hides behind an opaque helper, so the entrypoint stays IDL
	// opaque and is reported.
	if data.len() > 8 {
		process_a()
	} else {
		process_b()
	}
}

fn entrypoint_unrelated_match(data: &[u8]) -> Result<(), ()> {
	match data.len() {
		0 => process_a(),
		_ => process_b(),
	}
}

fn entrypoint_unrelated_enum(mode: Mode) -> Result<(), ()> {
	match mode {
		Mode::Fast => process_a(),
		Mode::Safe => process_b(),
	}
}

fn entrypoint_with_trailing_expression(data: &[u8]) -> Result<(), ()> {
	let result = match parse_instruction::<Instruction>(data)? {
		Instruction::Initialize => process_a(),
		Instruction::Update => process_b(),
	};

	result
}

#[derive(Default)]
enum FakeInstruction {
	#[default]
	Initialize,
	Update,
}

fn entrypoint_with_unrelated_instruction_name(
	data: &[u8],
	fake: FakeInstruction,
) -> Result<(), ()> {
	let _: Instruction = parse_instruction(data)?;
	match fake {
		//~^ WARNING: IDL-friendly instruction dispatch should be a direct `match`
		FakeInstruction::Initialize => process_a(),
		FakeInstruction::Update => process_b(),
	}
}

fn process_instruction_variant(mode: Mode) -> Result<(), ()> {
	match mode {
		Mode::Fast => process_a(),
		Mode::Safe => process_b(),
	}
}

fn main() {}

// check-warn
