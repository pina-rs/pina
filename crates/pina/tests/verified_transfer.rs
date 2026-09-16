//! A verified token transfer reconciles the balance change the CPI caused.
//!
//! Reading a token balance before a CPI and again after it is the pattern
//! every program with a vault writes by hand: snapshot the source, invoke,
//! re-read, and reconcile the debit against the amount that was supposed to
//! move. Two real consumers each hand-rolled it across many sites — one around
//! fifteen, another in two ~110-line wrappers — because the framework offered
//! builders but no reconciliation.
//!
//! The arithmetic is the part worth testing on the host: which amounts are
//! accepted, and which discrepancies are rejected. A helper that silently
//! allows a shortfall is worse than no helper at all.

#![cfg(feature = "token")]

use pina::PinaProgramError;
use pina::ProgramError;
use pina::token::verified;

/// The exact debit passed.
#[test]
fn accepts_an_exact_debit() {
	let outcome = verified::reconcile_debit(1_000, 750, 250).expect("exact debit");

	assert_eq!(outcome, ());
}

/// A debit larger than requested is a surplus: the transfer moved more than
/// the caller accounted for, which is a bug in the caller's arithmetic.
#[test]
fn rejects_a_debit_larger_than_requested() {
	let error = verified::reconcile_debit(1_000, 700, 250).expect_err("surplus must fail");

	assert_eq!(
		error,
		ProgramError::from(PinaProgramError::UnverifiedTransfer)
	);
}

/// A debit smaller than requested means the transfer was short: a fee, a hook,
/// or a partial transfer the caller did not expect. Silently accepting it
/// credits the recipient less than the ledger says.
#[test]
fn rejects_a_debit_smaller_than_requested() {
	let error = verified::reconcile_debit(1_000, 800, 250).expect_err("shortfall must fail");

	assert_eq!(
		error,
		ProgramError::from(PinaProgramError::UnverifiedTransfer)
	);
}

/// An unchanged balance means nothing moved at all, which is the failure a
/// snapshot check exists to catch.
#[test]
fn rejects_a_transfer_that_moved_nothing() {
	let error = verified::reconcile_debit(1_000, 1_000, 250).expect_err("no movement must fail");

	assert_eq!(
		error,
		ProgramError::from(PinaProgramError::UnverifiedTransfer)
	);
}

/// A zero-amount transfer has nothing to reconcile, so the helper refuses it
/// rather than reporting a vacuous success.
#[test]
fn rejects_a_zero_amount() {
	let error = verified::reconcile_debit(1_000, 1_000, 0).expect_err("zero amount must fail");

	assert_eq!(
		error,
		ProgramError::from(PinaProgramError::UnverifiedTransfer)
	);
}

/// A balance that increased during the CPI is never a valid debit.
#[test]
fn rejects_a_credit() {
	let error = verified::reconcile_debit(1_000, 1_100, 250).expect_err("credit must fail");

	assert_eq!(
		error,
		ProgramError::from(PinaProgramError::UnverifiedTransfer)
	);
}

/// A destination credit equal to the amount is the fee-free case.
#[test]
fn accepts_a_credit_equal_to_the_amount() {
	verified::reconcile_credit_at_most(0, 250, 250).expect("exact credit");
}

/// A Token-2022 transfer fee or hook can make the credit smaller than the
/// debit. That is expected and must not fail.
#[test]
fn accepts_a_credit_smaller_than_the_amount() {
	verified::reconcile_credit_at_most(0, 240, 250)
		.expect("a fee-reduced credit is still the caller's transfer");
}

/// A credit larger than the amount means the destination received tokens from
/// somewhere else in the same instruction, so the attribution is wrong.
#[test]
fn rejects_a_credit_larger_than_the_amount() {
	let error =
		verified::reconcile_credit_at_most(0, 300, 250).expect_err("excess credit must fail");

	assert_eq!(
		error,
		ProgramError::from(PinaProgramError::UnverifiedTransfer)
	);
}

/// An unchanged destination means the transfer never landed.
#[test]
fn rejects_a_credit_that_never_landed() {
	let error = verified::reconcile_credit_at_most(100, 100, 250).expect_err("no credit must fail");

	assert_eq!(
		error,
		ProgramError::from(PinaProgramError::UnverifiedTransfer)
	);
}

/// A destination that lost tokens during the CPI is not a credit at all.
#[test]
fn rejects_a_destination_debit() {
	let error =
		verified::reconcile_credit_at_most(500, 400, 250).expect_err("destination debit must fail");

	assert_eq!(
		error,
		ProgramError::from(PinaProgramError::UnverifiedTransfer)
	);
}

/// A zero amount is refused on the credit side too.
#[test]
fn rejects_a_zero_credit_amount() {
	let error = verified::reconcile_credit_at_most(0, 0, 0).expect_err("zero amount must fail");

	assert_eq!(
		error,
		ProgramError::from(PinaProgramError::UnverifiedTransfer)
	);
}
