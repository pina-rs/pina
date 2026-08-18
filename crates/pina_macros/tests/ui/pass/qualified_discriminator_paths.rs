use pina::*;

mod discriminators {
	use pina::*;

	#[discriminator]
	pub enum AccountKind {
		State = 1,
	}

	#[discriminator]
	pub enum InstructionKind {
		Initialize = 1,
	}
}

#[account(discriminator = crate::discriminators::AccountKind, variant = State)]
pub struct QualifiedAccount {}

#[instruction(discriminator = crate::discriminators::InstructionKind::Initialize)]
pub struct QualifiedInstruction {}

fn main() {}
