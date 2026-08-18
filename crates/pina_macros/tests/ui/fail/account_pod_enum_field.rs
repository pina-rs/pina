use pina::*;

#[derive(ZeroPod, Clone, Copy)]
#[repr(u8)]
pub enum Color {
	Red = 0,
	Blue = 1,
}

#[discriminator]
pub enum Kind {
	Palette = 0,
}

#[account(discriminator = Kind)]
pub struct Palette {
	pub color: Color,
}

fn main() {}
