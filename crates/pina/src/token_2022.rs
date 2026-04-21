//! SPL Token-2022 re-exports with pina compatibility aliases.

pub use pinocchio_token_2022::*;

pub mod state {
	pub use pinocchio_token_2022::state::*;

	pub type TokenAccount = Account;
}
