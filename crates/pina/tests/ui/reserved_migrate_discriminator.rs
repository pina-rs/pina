use pina::*;

// The all-ones discriminator belongs to the framework `Migrate` instruction.
#[discriminator]
pub enum Instruction {
	Update = 0,
	Reserved = 0xff,
}

fn main() {}
