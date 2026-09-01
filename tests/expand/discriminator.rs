use pina::*;

#[discriminator]
#[derive(Debug)]
pub enum MyDiscriminator {
	First = 0,
	Second = 1,
	Third = 2,
}

#[discriminator(primitive = u16, crate = ::pina)]
#[derive(Debug)]
pub enum U16Discriminator {
	First = 0,
	Second = 1,
}

#[discriminator(primitive = u32)]
#[derive(Debug)]
pub enum U32Discriminator {
	First = 0,
	Second = 1,
}

#[discriminator(primitive = u64, crate = ::pina)]
#[derive(Debug)]
pub enum U64Discriminator {
	First = 0,
	Second = 1,
}

#[discriminator(final)]
#[derive(Debug)]
pub enum FinalDiscriminator {
	Only = 0,
}
