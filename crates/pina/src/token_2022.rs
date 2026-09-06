//! SPL Token-2022 re-exports with pina compatibility aliases.

pub use pinocchio_token_2022::*;

pub mod state {
	pub use pinocchio_token_2022::state::*;

	/// Validated Token-2022 account view alias produced by the token validators.
	pub type TokenAccount = Account;
}
