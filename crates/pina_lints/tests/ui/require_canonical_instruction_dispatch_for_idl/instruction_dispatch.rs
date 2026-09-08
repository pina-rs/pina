// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

#[derive(Debug)]
enum Instruction {
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
	match Instruction::try_from_data(data)? {
		Instruction::Initialize => process_a(),
		Instruction::Update => process_b(),
	}
}

fn entrypoint_with_local(data: &[u8]) -> Result<(), ()> {
	let instruction = Instruction::try_from_data(data)?;
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
	match data.first() {
		Some(_) => process_a(),
		None => process_b(),
	}
}

fn entrypoint_unrelated_enum(mode: Mode) -> Result<(), ()> {
	match mode {
		Mode::Fast => process_a(),
		Mode::Safe => process_b(),
	}
}

impl Instruction {
	fn try_from_data(_data: &[u8]) -> Result<Self, ()> {
		Ok(Self::Initialize)
	}
}

fn main() {}

// check-warn
