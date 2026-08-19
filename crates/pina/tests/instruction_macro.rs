#![allow(dead_code)]

use pina::*;

#[discriminator(crate = ::pina, primitive = u8, final)]
pub enum MyInstruction {
	FlipBit = 0,
	Another = 1,
}

#[instruction(crate = ::pina, discriminator = MyInstruction)]
#[derive(Debug)]
pub struct FlipBit {
	/// The data section being updated.
	pub section_index: u8,
	/// The index of the `u16` value in the array.
	pub array_index: u8,
	/// The offset of the bit being set.
	pub offset: u8,
	/// The value to set the bit to: `0` or `1`.
	pub value: u8,
}

#[test]
fn test_instruction_macro() {
	let mut bytes = [0u8; FlipBit::SIZE];
	let flip_bit = FlipBit::initialize(&mut bytes)
		.unwrap_or_else(|error| panic!("instruction initialization failed: {error:?}"));
	flip_bit.section_index = 1;
	flip_bit.array_index = 2;
	flip_bit.offset = 3;
	flip_bit.value = 1;

	assert_eq!(flip_bit.section_index, 1);
	assert_eq!(flip_bit.array_index, 2);
	assert_eq!(flip_bit.offset, 3);
	assert_eq!(flip_bit.value, 1);

	let mut expected_discriminator = [0u8; MyInstruction::BYTES];
	MyInstruction::FlipBit.write_discriminator(&mut expected_discriminator);

	assert_eq!(flip_bit.discriminator, expected_discriminator);

	let _ = flip_bit;
	let flip_bit_from_bytes = FlipBit::try_from_bytes(&bytes)
		.unwrap_or_else(|error| panic!("instruction parsing failed: {error:?}"));
	assert_eq!(flip_bit_from_bytes.section_index, 1);
	assert_eq!(flip_bit_from_bytes.array_index, 2);
	assert_eq!(flip_bit_from_bytes.offset, 3);
	assert_eq!(flip_bit_from_bytes.value, 1);
}
