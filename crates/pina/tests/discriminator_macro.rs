use pina::*;

#[discriminator(primitive = u16)]
#[derive(Debug, PartialEq)]
pub enum MyDiscriminator {
	One = 0,
	Two = 1,
}

#[test]
fn test_discriminator_macro() {
	// Check size
	assert_eq!(size_of::<MyDiscriminator>(), 2);

	// Check conversion to primitive
	let prim: u16 = MyDiscriminator::Two.into();
	assert_eq!(prim, 1);

	// Check conversion from primitive
	let disc: MyDiscriminator = 1u16.try_into().unwrap();
	assert_eq!(disc, MyDiscriminator::Two);

	// Check discriminator matching
	let mut bytes = [0u8; 2];
	MyDiscriminator::Two.write_discriminator(&mut bytes);
	assert_eq!(bytes, [1, 0]); // little-endian for u16

	assert!(MyDiscriminator::Two.matches_discriminator(&bytes));
	assert!(!MyDiscriminator::One.matches_discriminator(&bytes));
}

#[discriminator(crate = ::pina, primitive = u16, final)]
pub enum MyAccount {
	ConfigState = 0,
	GameState = 1,
	SectionState = 2,
}