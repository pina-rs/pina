use pina::*;

#[discriminator]
#[derive(Debug)]
pub enum InstructionDisc {
	Initialize = 0,
	FlipBit = 1,
	Transfer = 2,
	TransferData = 3,
	ComplexInstruction = 4,
}

#[instruction(crate = pina, discriminator = InstructionDisc)]
pub struct Initialize {}

#[instruction(crate = pina, discriminator = InstructionDisc)]
pub struct FlipBit {
	pub section_index: u8,
	pub array_index: u8,
	pub offset: u8,
	pub value: u8,
}

#[instruction(crate = pina, discriminator = InstructionDisc)]
pub struct Transfer {
	pub amount: PodU64,
}

#[instruction(crate = pina, discriminator = InstructionDisc, variant = TransferData)]
pub struct CustomTransferData {
	pub amount: PodU64,
	pub destination: [u8; 32],
}

#[instruction(crate = pina, discriminator = InstructionDisc)]
pub struct ComplexInstruction {
	pub seed: [u8; 32],
	pub amount: PodU64,
	pub bump: u8,
	pub flags: [u8; 4],
}
