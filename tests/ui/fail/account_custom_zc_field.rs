use pina::*;

mod custom {
	use pina::ZcField;

	#[derive(Clone, Copy)]
	pub struct Custom;

	impl ZcField for Custom {
		type Pod = u8;

		const POD_SIZE: usize = 1;
	}
}

#[discriminator]
pub enum Kind {
	CustomState = 0,
}

#[account(discriminator = Kind)]
pub struct CustomState {
	pub custom: custom::Custom,
}

fn main() {}
