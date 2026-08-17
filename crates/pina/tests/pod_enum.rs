//! Tests for the `#[derive(PodEnum)]` macro — unit enums usable as fields in
//! `#[account]` structs via their zero-copy `EnumZc` companion.

use pina::*;

// ---------------------------------------------------------------------------
// A unit enum with explicit discriminants.
// ---------------------------------------------------------------------------

#[derive(PodEnum, Debug, PartialEq)]
#[repr(u8)]
enum Color {
	Red = 0,
	Green = 1,
	Blue = 2,
}

// ---------------------------------------------------------------------------
// An account struct using the zero-copy companion as a field type.
// ---------------------------------------------------------------------------

#[discriminator(crate = ::pina, primitive = u8, final)]
pub enum MyAccount {
	Palette = 7,
}

#[account(crate = ::pina, discriminator = MyAccount)]
pub struct Palette {
	pub color: ColorZc,
	pub brightness: PodU64,
}

// ---------------------------------------------------------------------------
// A nested pod struct (non-account) using the companion as a field.
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
struct NestedPalette {
	pub color: ColorZc,
	pub brightness: PodU64,
}

impl ZcValidate for NestedPalette {
	fn validate_ref(value: &Self) -> Result<(), ZeroPodError> {
		ColorZc::validate_ref(&value.color)?;
		PodU64::validate_ref(&value.brightness)?;
		Ok(())
	}
}

// SAFETY: align 1, no padding, every bit pattern valid, validate_ref
// is load-bearing.
#[allow(unsafe_code)]
unsafe impl ZcElem for NestedPalette {}

impl ZeroPodSchema for NestedPalette {
	const LAYOUT: LayoutKind = LayoutKind::Fixed;
}

#[allow(unsafe_code)]
impl ZeroPodFixed for NestedPalette {
	type Zc = NestedPalette;

	const SIZE: usize = core::mem::size_of::<NestedPalette>();

	fn from_bytes(data: &[u8]) -> Result<&Self::Zc, ZeroPodError> {
		<Self as ZeroPodFixed>::validate(data)?;
		Ok(unsafe { &*(data.as_ptr() as *const Self::Zc) })
	}

	fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self::Zc, ZeroPodError> {
		<Self as ZeroPodFixed>::validate(data)?;
		Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self::Zc) })
	}

	fn validate(data: &[u8]) -> Result<(), ZeroPodError> {
		if data.len() < Self::SIZE {
			return Err(ZeroPodError::BufferTooSmall);
		}
		let zc = unsafe { &*(data.as_ptr() as *const Self::Zc) };
		<Self::Zc as ZcValidate>::validate_ref(zc)?;
		Ok(())
	}

	unsafe fn from_bytes_unchecked(data: &[u8]) -> &Self::Zc {
		&*(data.as_ptr() as *const Self::Zc)
	}

	unsafe fn from_bytes_mut_unchecked(data: &mut [u8]) -> &mut Self::Zc {
		&mut *(data.as_mut_ptr() as *mut Self::Zc)
	}
}

impl ZcField for NestedPalette {
	type Pod = NestedPalette;

	const POD_SIZE: usize = core::mem::size_of::<NestedPalette>();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn pod_enum_companion_layout() {
	assert_eq!(core::mem::size_of::<ColorZc>(), 1);
	assert_eq!(core::mem::align_of::<ColorZc>(), 1);
	assert_eq!(<Color as ZeroPodFixed>::SIZE, 1);
	assert_eq!(<Color as ZcField>::POD_SIZE, 1);
}

#[test]
fn pod_enum_from_and_get() {
	let zc: ColorZc = Color::Red.into();
	assert_eq!(zc.get(), 0);
	assert_eq!(ColorZc::from(Color::Blue), ColorZc([2]));
	assert_eq!(u8::from(Color::Green), 1);
}

#[test]
fn pod_enum_comparisons() {
	let zc: ColorZc = Color::Green.into();
	assert!(zc.is(Color::Green));
	assert!(!zc.is(Color::Red));
	assert_eq!(zc, Color::Green);
	assert_eq!(zc, 1u8);
	assert_eq!(zc, ColorZc([1]));
}

#[test]
fn pod_enum_try_to_enum() {
	let zc: ColorZc = Color::Blue.into();
	assert_eq!(zc.try_to_enum(), Ok(Color::Blue));

	// Invalid discriminant is rejected.
	let invalid = ColorZc([99]);
	assert_eq!(
		invalid.try_to_enum(),
		Err(ZeroPodError::InvalidDiscriminant)
	);
}

#[test]
fn pod_enum_validate_ref() {
	assert!(ColorZc::validate_ref(&Color::Red.into()).is_ok());
	assert!(ColorZc::validate_ref(&Color::Blue.into()).is_ok());
	assert!(ColorZc::validate_ref(&ColorZc([99])).is_err());
}

#[test]
fn pod_enum_display_debug() {
	assert_eq!(format!("{}", ColorZc::from(Color::Red)), "Red");
	assert_eq!(
		format!("{:?}", ColorZc::from(Color::Green)),
		"ColorZc(Green)"
	);
	assert_eq!(format!("{}", ColorZc([42])), "Color(invalid: 42)");
}

#[test]
fn pod_enum_in_account_roundtrip() {
	let mut data = [0u8; 1 + 1 + 8];
	data[0] = 7; // discriminator
	data[1] = 1; // color = Green
	data[2] = 42; // brightness (LE u64 low byte)

	let palette = Palette::try_from_bytes(&data).unwrap();
	assert_eq!(palette.color, Color::Green);
	assert_eq!(palette.color.get(), 1);
	assert_eq!(u64::from(palette.brightness), 42);
}

#[test]
fn pod_enum_in_account_mutation() {
	let mut data = [0u8; 1 + 1 + 8];
	data[0] = 7;
	data[1] = 0; // Red
	data[2] = 10;

	{
		let palette = Palette::try_from_bytes_mut(&mut data).unwrap();
		palette.color = Color::Blue.into();
		palette.brightness = PodU64::from(999);
	}

	let palette = Palette::try_from_bytes(&data).unwrap();
	assert_eq!(palette.color, Color::Blue);
	assert_eq!(u64::from(palette.brightness), 999);
}

#[test]
fn pod_enum_in_account_rejects_invalid_discriminant() {
	let mut data = [0u8; 1 + 1 + 8];
	data[0] = 7;
	data[1] = 99; // invalid Color discriminant

	assert!(Palette::try_from_bytes(&data).is_err());
	assert!(<Palette as PinaAccount>::validate(&data).is_err());
}

// `as_account` / `as_account_mut` use `PinaAccount::try_from_bytes` internally,
// so the boundary validation tested above covers the loader path. This test
// confirms the account implements the full `PinaAccount` surface.
#[test]
fn pod_enum_account_implements_pina_account() {
	let mut data = [0u8; 1 + 1 + 8];
	data[0] = 7;
	data[1] = 2; // Blue
	data[2] = 7;

	assert!(<Palette as PinaAccount>::validate(&data).is_ok());
	let palette = <Palette as PinaAccount>::try_from_bytes(&data).unwrap();
	assert_eq!(palette.color, Color::Blue);

	// Invalid discriminant rejected by validate.
	let mut bad = data.clone();
	bad[1] = 100;
	assert!(<Palette as PinaAccount>::validate(&bad).is_err());
}

#[test]
fn pod_enum_in_nested_struct_roundtrip() {
	let mut data = [0u8; 1 + 8];
	data[0] = 2; // Blue
	data[1] = 100;

	let nested = NestedPalette::from_bytes(&data).unwrap();
	assert_eq!(nested.color, Color::Blue);
	assert_eq!(u64::from(nested.brightness), 100);
}

#[test]
fn pod_enum_in_nested_struct_rejects_invalid() {
	let mut data = [0u8; 1 + 8];
	data[0] = 55; // invalid
	assert!(NestedPalette::from_bytes(&data).is_err());
}

#[test]
fn pod_enum_u16_repr() {
	#[derive(PodEnum, Debug, PartialEq)]
	#[repr(u16)]
	enum Level {
		Low = 0,
		Medium = 1000,
		High = 65535,
	}

	assert_eq!(core::mem::size_of::<LevelZc>(), 2);
	assert_eq!(<Level as ZeroPodFixed>::SIZE, 2);

	let zc: LevelZc = Level::High.into();
	assert_eq!(zc.get(), 65535u16);
	assert!(zc.is(Level::High));
	assert_eq!(zc.try_to_enum(), Ok(Level::High));

	let invalid = LevelZc([0x01, 0x00]); // 1 — not a valid discriminant
	assert!(LevelZc::validate_ref(&invalid).is_err());
}

#[test]
fn pod_enum_crate_path_attribute() {
	// The derive supports #[pina(crate = ::pina)] for renamed dependencies.
	#[derive(PodEnum)]
	#[pina(crate = ::pina)]
	#[repr(u8)]
	enum Direction {
		North = 0,
		South = 1,
		East = 2,
		West = 3,
	}

	assert_eq!(core::mem::size_of::<DirectionZc>(), 1);
	let zc: DirectionZc = Direction::West.into();
	assert_eq!(zc.get(), 3);
	assert!(DirectionZc::validate_ref(&zc).is_ok());
	assert!(DirectionZc::validate_ref(&DirectionZc([9])).is_err());
}
