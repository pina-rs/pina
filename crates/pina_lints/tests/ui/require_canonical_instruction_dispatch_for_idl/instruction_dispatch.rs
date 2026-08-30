// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

#[derive(Debug)]
enum Instruction {
	Initialize,
	Update,
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

impl Instruction {
	fn try_from_data(_data: &[u8]) -> Result<Self, ()> {
		Ok(Self::Initialize)
	}
}

fn main() {}
