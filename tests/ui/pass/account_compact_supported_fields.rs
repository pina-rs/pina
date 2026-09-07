use pina::*;

#[discriminator]
pub enum Kind {
	State = 1,
}

#[account(discriminator = Kind, compact)]
pub struct State {
	pub revision: Option<u64>,
	pub name: String<32>,
	pub values: Vec<u64, 8>,
	pub note: Option<String<64>>,
	pub maybe_values: Option<Vec<u16, 4>>,
	pub labels: Vec<String<16>, 4>,
}

fn main() {}
