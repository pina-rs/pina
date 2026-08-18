//! Verifies that Pina account schemas compose with zeropod's native enum
//! support. Pina does not implement a second enum representation: the
//! `ZeroPod` derive owns the `Color` to `ColorZc` mapping and validation.

use core::mem::align_of;
use core::mem::size_of;

use pina::*;

#[derive(ZeroPod, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
	Red = 0,
	Green = 1,
	Blue = 2,
}

#[discriminator(crate = ::pina, primitive = u8, final)]
pub enum MyAccount {
	Palette = 7,
}

#[account(crate = ::pina, discriminator = MyAccount)]
pub struct Palette {
	pub color: Color,
	pub brightness: u64,
}

#[derive(ZeroPod)]
#[allow(dead_code)]
struct NestedPalette {
	pub color: Color,
	pub brightness: u64,
}

#[test]
fn zeropod_enum_companion_layout() {
	assert_eq!(size_of::<ColorZc>(), 1);
	assert_eq!(align_of::<ColorZc>(), 1);
	assert_eq!(<Color as ZeroPodFixed>::SIZE, 1);
	assert_eq!(<Color as ZcField>::POD_SIZE, 1);
}

#[test]
fn zeropod_enum_conversion_and_validation() {
	let mut bytes = [0u8; Color::SIZE];
	let color = Color::from_bytes_mut(&mut bytes).unwrap();
	*color = Color::Blue.into();
	assert!(color.is(Color::Blue));
	assert_eq!(color.try_to_enum(), Ok(Color::Blue));

	bytes[0] = 99;
	assert_eq!(
		Color::validate(&bytes),
		Err(ZeroPodError::InvalidDiscriminant)
	);
}

#[test]
fn zeropod_enum_in_account_roundtrip() {
	let mut data = [0u8; Palette::SIZE];
	{
		let palette = Palette::initialize(&mut data).unwrap();
		palette.color = Color::Green.into();
		palette.brightness.set(42);
	}

	let palette = Palette::try_from_bytes(&data).unwrap();
	assert!(palette.color.is(Color::Green));
	assert_eq!(palette.color.get(), 1);
	assert_eq!(palette.brightness.get(), 42);
}

#[test]
fn zeropod_enum_in_account_mutation() {
	let mut data = [0u8; Palette::SIZE];
	{
		let palette = Palette::initialize(&mut data).unwrap();
		palette.color = Color::Red.into();
		palette.brightness.set(10);
	}
	{
		let palette = Palette::try_from_bytes_mut(&mut data).unwrap();
		palette.color = Color::Blue.into();
		palette.brightness.set(999);
	}

	let palette = Palette::try_from_bytes(&data).unwrap();
	assert!(palette.color.is(Color::Blue));
	assert_eq!(palette.brightness.get(), 999);
}

#[test]
fn zeropod_enum_in_account_rejects_invalid_discriminant() {
	let mut data = [0u8; Palette::SIZE];
	Palette::initialize(&mut data).unwrap();
	data[1] = 99;

	assert!(Palette::try_from_bytes(&data).is_err());
	assert!(<Palette as PinaAccount>::validate_account_data(&data).is_err());
}

#[test]
fn zeropod_enum_in_nested_struct_roundtrip() {
	let mut data = [0u8; NestedPalette::SIZE];
	{
		let nested = NestedPalette::from_bytes_mut(&mut data).unwrap();
		nested.color = Color::Blue.into();
		nested.brightness.set(100);
	}

	let nested = NestedPalette::from_bytes(&data).unwrap();
	assert!(nested.color.is(Color::Blue));
	assert_eq!(nested.brightness.get(), 100);
}

#[test]
fn zeropod_enum_in_nested_struct_rejects_invalid() {
	let mut data = [0u8; NestedPalette::SIZE];
	data[0] = 55;
	assert!(NestedPalette::from_bytes(&data).is_err());
}
