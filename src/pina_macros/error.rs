//! `#[error]` contract tests through the public macro.
//!
//! The macro generates `From<Enum> for ProgramError` with custom error codes
//! derived from the enum discriminants, plus the standard derives the enum
//! needs for downstream matching.

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
	InsufficientFunds = 0,
	AlreadyInitialized = 1,
	InvalidAuthority = 2,
	InvalidMint = 3,
	Overflow = 4,
}

#[error]
#[derive(Debug)]
pub enum DefaultCrateError {
	Something = 0,
}

/// Basic error maps variants to `ProgramError::Custom` codes.
#[test]
fn basic() {
	let err: ProgramError = MyError::Invalid.into();
	match err {
		ProgramError::Custom(code) => {
			assert_eq!(code, MyError::Invalid as u32);
		}
		other => panic!("expected Custom, got {other:?}"),
	}
	assert_eq!(
		ProgramError::from(MyError::Duplicate),
		ProgramError::Custom(MyError::Duplicate as u32),
	);
}

/// `final` omits `#[non_exhaustive]` so downstream matches are exhaustive.
#[test]
fn final_attribute() {
	let err: ProgramError = FinalError::Unauthorized.into();
	assert!(matches!(err, ProgramError::Custom(_)));
}

/// Many variants keep distinct custom codes.
#[test]
fn many_variants() {
	let codes: std::vec::Vec<u32> = [MyError::Invalid, MyError::Duplicate]
		.iter()
		.map(|e| {
			match ProgramError::from(*e) {
				ProgramError::Custom(code) => code,
				other => panic!("expected Custom, got {other:?}"),
			}
		})
		.collect();
	for (i, code) in codes.iter().enumerate() {
		assert_eq!(*code, i as u32);
	}
}

/// The default crate path resolves without an explicit `crate` argument.
#[test]
fn default_crate_path() {
	let err: ProgramError = DefaultCrateError::Something.into();
	assert!(matches!(err, ProgramError::Custom(_)));
}
