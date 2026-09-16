use pina::*;

#[discriminator]
pub enum Kind {
	Weights = 0,
}

#[account(discriminator = Kind)]
pub struct Weights {
	pub words: [u16; 4],
	pub values: [u64; 8],
	pub spelled: [PodU64; 2],
	pub flags: [bool; 2],
	pub owners: [Address; 2],
	pub nested: [[u8; 4]; 2],
	pub maybe: Option<[u64; 2]>,
}

fn main() {
	let mut bytes = [0u8; Weights::SIZE];
	let _ = Weights::initialize(&mut bytes, |_| Ok(()));
}
