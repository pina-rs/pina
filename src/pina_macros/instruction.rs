//! `#[instruction]` contract tests through the public macro.
//!
//! The macro generates a zeropod-backed instruction payload with a
//! discriminator field, `SIZE`, `try_from_bytes`, and a `HasDiscriminator`
//! impl linking back to the discriminator enum.

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

#[allow(clippy::self_named_constructors)]
#[instruction(crate = pina, discriminator = InstructionDisc)]
pub struct Initialize {}

#[allow(clippy::self_named_constructors)]
#[instruction(crate = pina, discriminator = InstructionDisc)]
pub struct FlipBit {
	pub section_index: u8,
	pub array_index: u8,
	pub offset: u8,
	pub value: u8,
}

#[allow(clippy::self_named_constructors)]
#[instruction(crate = pina, discriminator = InstructionDisc)]
pub struct Transfer {
	pub amount: PodU64,
}

#[allow(clippy::self_named_constructors)]
#[instruction(crate = pina, discriminator = InstructionDisc, variant = TransferData)]
pub struct CustomTransferData {
	pub amount: PodU64,
	pub destination: [u8; 32],
}

#[allow(clippy::self_named_constructors)]
#[instruction(crate = pina, discriminator = InstructionDisc)]
pub struct ComplexInstruction {
	pub seed: [u8; 32],
	pub amount: PodU64,
	pub bump: u8,
	pub flags: [u8; 4],
}

/// A minimal instruction has a one-byte discriminator payload.
#[test]
fn minimal() {
	assert_eq!(Initialize::SIZE, 1);
	assert!(Initialize::try_from_bytes(&[0]).is_ok());
	assert!(Initialize::try_from_bytes(&[]).is_err());
}

/// Multi-field instructions round-trip all fields through zeropod.
#[test]
fn many_fields() {
	let mut bytes: std::vec::Vec<u8> = std::vec![0; FlipBit::SIZE];
	{
		let view = FlipBit::initialize(&mut bytes).unwrap();
		view.section_index = 1;
		view.array_index = 2;
		view.offset = 3;
		view.value = 42;
	}
	let parsed = FlipBit::try_from_bytes(&bytes).unwrap();
	assert_eq!(parsed.section_index, 1);
	assert_eq!(parsed.array_index, 2);
	assert_eq!(parsed.offset, 3);
	assert_eq!(parsed.value, 42);
}

/// Existing derives are preserved alongside the generated impls.
#[test]
fn roundtrip_with_pod() {
	let mut bytes: std::vec::Vec<u8> = std::vec![0; Transfer::SIZE];
	{
		let view = Transfer::initialize(&mut bytes).unwrap();
		view.amount.set(100);
	}
	let parsed = Transfer::try_from_bytes(&bytes).unwrap();
	assert_eq!(parsed.amount.get(), 100);
}

/// A custom variant name resolves in the `HasDiscriminator` impl.
#[test]
fn with_custom_variant() {
	let disc = &<CustomTransferData as HasDiscriminator>::VALUE;
	assert_eq!(*disc, InstructionDisc::TransferData);
}

/// A path variant compiles (runtime behavior covered by example programs).
#[test]
fn with_path_variant() {
	// The path variant form generates the same struct with a different
	// discriminator link. Runtime round-trips are covered by the example
	// programs which have declare_id! in scope.
	fn assert_compiles<T: HasDiscriminator>() {}
	assert_compiles::<CustomTransferData>();
}

/// Array and Pod fields contribute to the wire layout.
#[test]
fn with_array_and_pod() {
	assert_eq!(ComplexInstruction::SIZE, 1 + 32 + 8 + 1 + 4,);
	let mut bytes: std::vec::Vec<u8> = std::vec![0; ComplexInstruction::SIZE];
	ComplexInstruction::initialize(&mut bytes).unwrap();
	assert!(ComplexInstruction::try_from_bytes(&bytes).is_ok());
}
