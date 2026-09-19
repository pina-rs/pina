//! The generated `#[pda]` surface must bind the caller's `Address` spelling to
//! the crate's type. A user type named `Address` would otherwise resolve in the
//! generated signatures and silently change which address type a seed takes.

use pina::*;

#[derive(Clone, Copy)]
pub struct Address([u8; 32]);

unsafe impl ZcField for Address {
	type Pod = [u8; 32];
}

#[discriminator]
pub enum Kind {
	State = 0,
}

#[pda(seeds = [b"vault", authority: Address])]
pub struct Vault {
	pub authority: Address,
}

fn main() {}
