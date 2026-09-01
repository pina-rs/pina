use pina::*;

#[error(crate = pina)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MyError {
	Invalid = 0,
	Duplicate = 1,
}

#[error(crate = pina, final)]
#[derive(Debug)]
pub enum FinalError {
	Unauthorized = 0,
}

#[error(crate = pina)]
#[derive(Debug)]
pub enum DetailedError {
	/// Not enough funds to complete the transaction.
	InsufficientFunds = 0,
	/// The account has already been initialized.
	AlreadyInitialized = 1,
	/// The provided authority does not match.
	InvalidAuthority = 2,
	/// The mint does not match.
	InvalidMint = 3,
	/// Arithmetic overflow occurred.
	Overflow = 4,
}

#[error]
#[derive(Debug)]
pub enum DefaultCrateError {
	Something = 0,
}
