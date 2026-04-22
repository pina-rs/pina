//! SPL Token re-exports with pina compatibility aliases.

pub use pinocchio_token::*;

pub mod state {
	pub use pinocchio_token::state::*;

	pub type TokenAccount = Account;
}
