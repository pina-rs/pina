//! Verifies standalone `PinaPod` enum support.
//!
//! Pina's macro-generated account, instruction, and event schemas reject
//! custom field mappings, including these enums. Direct `PinaPod` users may
//! still compose audited standalone schemas outside that closed boundary.

use core::mem::align_of;
use core::mem::size_of;

use pina::*;

#[derive(PinaPod, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
	Red = 0,
	Green = 1,
	Blue = 2,
}

#[derive(PinaPod, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum NonZeroColor {
	Red = 1,
	Blue = 7,
}

#[derive(PinaPod)]
#[allow(dead_code)]
struct NestedPalette {
	pub color: Color,
	pub brightness: u64,
}

#[test]
fn pinapod_enum_companion_layout() {
	assert_eq!(size_of::<ColorZc>(), 1);
	assert_eq!(align_of::<ColorZc>(), 1);
	assert_eq!(core::mem::size_of::<ColorZc>(), 1);
	assert_eq!(size_of::<<Color as ZcField>::Pod>(), 1);
}

#[test]
fn pinapod_enum_conversion_and_validation() {
	let mut bytes = [0u8; Color::SIZE];
	let color = Color::read_exact_mut(&mut bytes).unwrap();
	*color = Color::Blue.into();
	assert!(color.is(Color::Blue));
	assert_eq!(color.try_to_enum(), Ok(Color::Blue));

	bytes[0] = 99;
	assert_eq!(
		Color::validate_exact(&bytes),
		Err(PinaPodError::InvalidDiscriminant)
	);
}

#[test]
fn nonzero_enum_rejects_zeroed_storage() {
	let mut data = [0u8; NonZeroColor::SIZE];

	assert!(NonZeroColor::read_exact_mut(&mut data).is_err());
}

#[test]
fn pinapod_enum_in_nested_struct_roundtrip() {
	let mut data = [0u8; NestedPalette::SIZE];
	{
		let nested = NestedPalette::read_exact_mut(&mut data).unwrap();
		nested.color = Color::Blue.into();
		nested.brightness.set(100);
	}

	let nested = NestedPalette::read_exact(&data).unwrap();
	assert!(nested.color.is(Color::Blue));
	assert_eq!(nested.brightness.get(), 100);
}

#[test]
fn pinapod_enum_in_nested_struct_rejects_invalid() {
	let mut data = [0u8; NestedPalette::SIZE];
	data[0] = 55;
	assert!(NestedPalette::read_exact(&data).is_err());
}
