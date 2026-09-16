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

#[test]
fn try_from_bytes_mut_reports_the_same_split_errors() {
	let mut short_bytes = initialized_balance(5);
	short_bytes.pop();
	let short_error = Balance::try_from_bytes_mut(&mut short_bytes).err();
	assert_eq!(
		short_error,
		Some(PinaProgramError::InvalidAccountSize.into())
	);

	let mut wrong_bytes = initialized_balance(5);
	wrong_bytes[0] = 99;
	let wrong_error = Balance::try_from_bytes_mut(&mut wrong_bytes).err();
	assert_eq!(
		wrong_error,
		Some(PinaProgramError::InvalidDiscriminator.into())
	);
}

#[test]
fn validate_account_data_reports_the_same_split_errors() {
	let mut short_bytes = initialized_balance(5);
	short_bytes.pop();
	let short_error = Balance::validate_account_data(&short_bytes).err();
	assert_eq!(
		short_error,
		Some(PinaProgramError::InvalidAccountSize.into())
	);

	let mut wrong_bytes = initialized_balance(5);
	wrong_bytes[0] = 99;
	let wrong_error = Balance::validate_account_data(&wrong_bytes).err();
	assert_eq!(
		wrong_error,
		Some(PinaProgramError::InvalidDiscriminator.into())
	);
}

#[test]
fn a_correct_representation_still_decodes() {
	let bytes = initialized_balance(7);

	let decoded = Balance::try_from_bytes(&bytes).expect("valid account must decode");

	assert_eq!(decoded.amount.get(), 7);
}
