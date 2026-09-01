#![allow(non_camel_case_types)]

use pina::*;

mod shadow {
	use pina::ZcField;

	#[derive(Clone, Copy)]
	pub struct u8(pub ::core::primitive::u8);

	impl ZcField for u8 {
		type Pod = ::core::primitive::u8;

		const POD_SIZE: usize = 1;
	}
}

#[discriminator]
pub enum Kind {
	ShadowedScalar = 0,
}

#[account(discriminator = Kind)]
pub struct ShadowedScalar {
	pub value: shadow::u8,
}

fn main() {}
