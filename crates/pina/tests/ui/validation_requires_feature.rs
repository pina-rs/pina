use pina::*;

#[discriminator(primitive = u8)]
enum Kind {
	Instruction = 1,
}

#[instruction(discriminator = Kind)]
struct InvalidInstruction {
	#[pina(validate(min = 1))]
	amount: u64,
}

fn main() {}
