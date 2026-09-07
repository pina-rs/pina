use pina::*;

#[discriminator]
pub enum Kind {
	Profile = 0,
}

#[account(discriminator = Kind)]
pub struct Profile {
	pub name: String<32>,
	pub display_names: Vec<String<16>, 4>,
	pub note: Option<String<64>>,
	pub revisions: Option<Vec<u64, 8>>,
	pub history: PodVec<u64, 1024, 2>,
}

fn main() {}
