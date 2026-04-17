use pina::*;

#[discriminator]
pub enum InstructionKind {
	TupleInstruction = 0,
}

#[instruction(discriminator = InstructionKind, variant = TupleInstruction)]
pub struct TupleInstruction(u8);

fn main() {}
