#![allow(dead_code)]

use pina::*;

#[discriminator(crate = ::pina, primitive = u8, final)]
pub enum MyInstruction {
	FlipBit = 0,
	Another = 1,
	Collections = 2,
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

#[instruction(crate = ::pina, discriminator = MyInstruction)]
pub struct Collections {
	pub name: PodString<4>,
	pub values: PodVec<PodU16, 2>,
}

#[test]
fn test_instruction_macro() {
	let flip_bit = FlipBit::builder()
		.section_index(1)
		.array_index(2)
		.offset(3)
		.value(1)
		.build();

	assert_eq!(flip_bit.section_index, 1);
	assert_eq!(flip_bit.array_index, 2);
	assert_eq!(flip_bit.offset, 3);
	assert_eq!(flip_bit.value, 1);

	let mut expected_discriminator = [0u8; MyInstruction::BYTES];
	MyInstruction::FlipBit.write_discriminator(&mut expected_discriminator);

	assert_eq!(flip_bit.discriminator, expected_discriminator);

	let bytes = flip_bit.to_bytes();
	let flip_bit_from_bytes = FlipBit::try_from_bytes(&bytes).unwrap();

	assert_eq!(flip_bit, *flip_bit_from_bytes);
}

#[test]
fn partial_collections_serialize_without_reading_inactive_capacity() {
	let mut name = PodString::<4>::default();
	assert!(name.set("hi"));

	let mut values = PodVec::<PodU16, 2>::default();
	assert!(values.push(PodU16::from(0x1234)));

	let data = Collections::builder().name(name).values(values).build();
	let bytes = data.to_bytes();

	assert_eq!(bytes, [2, 2, b'h', b'i', 0, 0, 1, 0, 0x34, 0x12, 0, 0]);
	assert!(Collections::try_from_bytes(&bytes).is_ok());
}
