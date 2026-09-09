use pina::*;

#[discriminator(primitive = u8)]
enum Kind {
	Instruction = 1,
}

#[instruction(discriminator = Kind)]
struct InvalidInstruction {
	#[pina(validate(exact_len = 4))]
	#[pina(validate(max_len = 8))]
	label: String<32>,
}

fn main() {}
