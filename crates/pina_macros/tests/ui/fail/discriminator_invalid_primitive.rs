use pina::*;

#[discriminator(primitive = u128)]
pub enum BadPrimitiveDiscriminator {
	Value = 0,
}

fn main() {}
