//! Decode failures must name what actually failed.
//!
//! A length mismatch and a discriminator mismatch are different bugs with
//! different remedies, so they must be distinguishable from the wire:
//!
//! - `InvalidAccountSize` (`0xFFFF_FFFB`) means the account is the wrong
//!   length for the type — usually a stale or un-migrated representation.
//! - `InvalidDiscriminator` (`0xFFFF_FFFF`) means the bytes are not this
//!   account type at all.
//!
//! Collapsing both into `InvalidAccountData` is the failure mode that makes
//! a migration-envelope change undiagnosable from a client.

#![allow(dead_code)]

use std::vec;
use std::vec::Vec;

use pina::*;

const TEST_PROGRAM_ID: Address = address!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator(crate = ::pina)]
pub enum DecodeKind {
	Balance = 1,
}

#[account(crate = ::pina, discriminator = DecodeKind)]
pub struct Balance {
	pub amount: u64,
}

fn initialized_balance(amount: u64) -> Vec<u8> {
	let mut bytes = vec![0u8; Balance::SIZE];
	Balance::initialize(&mut bytes, |state| {
		state.amount.set(amount);
		Ok(())
	})
	.expect("valid account storage");
	bytes
}

#[test]
fn try_from_bytes_reports_a_short_buffer_as_invalid_account_size() {
	let mut bytes = initialized_balance(5);
	bytes.pop();

	let error = Balance::try_from_bytes(&bytes).err();

	assert_eq!(error, Some(PinaProgramError::InvalidAccountSize.into()));
}

#[test]
fn try_from_bytes_reports_a_long_buffer_as_invalid_account_size() {
	let mut bytes = initialized_balance(5);
	bytes.push(0);

	let error = Balance::try_from_bytes(&bytes).err();

	assert_eq!(error, Some(PinaProgramError::InvalidAccountSize.into()));
}

#[test]
fn try_from_bytes_reports_an_unknown_discriminator_as_invalid_discriminator() {
	let mut bytes = initialized_balance(5);
	bytes[0] = 99;

	let error = Balance::try_from_bytes(&bytes).err();

	assert_eq!(error, Some(PinaProgramError::InvalidDiscriminator.into()));
}

/// The generated immutable reader reports both failures distinctly.
///
/// This is the reader a program calls for a fixed account, so it is the one
/// whose errors reach a client. The generic trait defaults
/// (`try_from_bytes_mut`, `validate_account_data`) keep the framework's
/// previous combined error: they sit in the account-loading hot path, and
/// giving them the same split measured compute-unit regressions across example
/// suites that load accounts. Account loading already validates size
/// separately through `as_account`, so those callers keep a precise error
/// either way.
#[test]
fn generated_try_from_bytes_reports_both_failures_distinctly() {
	let mut short_bytes = initialized_balance(5);
	short_bytes.pop();
	let short_error = Balance::try_from_bytes(&short_bytes).err();
	assert_eq!(
		short_error,
		Some(PinaProgramError::InvalidAccountSize.into())
	);

	let mut wrong_bytes = initialized_balance(5);
	wrong_bytes[0] = 99;
	let wrong_error = Balance::try_from_bytes(&wrong_bytes).err();
	assert_eq!(
		wrong_error,
		Some(PinaProgramError::InvalidDiscriminator.into())
	);
}

/// The generic read paths keep the combined error, deliberately.
///
/// A caller reaching them through `as_account` gets `InvalidAccountSize` from
/// the size check that runs first; a discriminator mismatch arrives as
/// `InvalidAccountData`, exactly as before this change.
#[test]
fn generic_read_paths_keep_the_combined_error() {
	let mut short_bytes = initialized_balance(5);
	short_bytes.pop();
	assert_eq!(
		Balance::validate_account_data(&short_bytes).err(),
		Some(ProgramError::InvalidAccountData),
		"`as_account` reports size before this check runs"
	);

	let mut mutable_bytes = initialized_balance(5);
	mutable_bytes.pop();
	assert_eq!(
		Balance::try_from_bytes_mut(&mut mutable_bytes).err(),
		Some(ProgramError::InvalidAccountData)
	);

	let mut wrong_bytes = initialized_balance(5);
	wrong_bytes[0] = 99;
	assert_eq!(
		Balance::validate_account_data(&wrong_bytes).err(),
		Some(ProgramError::InvalidAccountData)
	);
}
