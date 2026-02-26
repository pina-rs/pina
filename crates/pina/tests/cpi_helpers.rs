use pina::MAX_PERMITTED_DATA_INCREASE;
use pina::combine_seeds_with_bump;
use pina::realloc_account;
use pina::realloc_account_zero;
use pinocchio::address::MAX_SEEDS;

#[test]
fn combine_seeds_with_bump_basic() {
	let seed_a: &[u8] = b"escrow";
	let seed_b: &[u8] = &[1, 2, 3];
	let bump = [42u8; 1];

	let result = combine_seeds_with_bump(&[seed_a, seed_b], &bump)
		.unwrap_or_else(|e| panic!("failed: {e:?}"));

	assert_eq!(&*result[0], b"escrow");
	assert_eq!(&*result[1], &[1, 2, 3]);
	assert_eq!(&*result[2], &[42]);
	// Remaining slots should be empty.
	for slot in &result[3..] {
		assert!(slot.is_empty());
	}
}

#[test]
fn combine_seeds_with_bump_single_seed() {
	let seed: &[u8] = b"hello";
	let bump = [0u8; 1];

	let result =
		combine_seeds_with_bump(&[seed], &bump).unwrap_or_else(|e| panic!("failed: {e:?}"));

	assert_eq!(&*result[0], b"hello");
	assert_eq!(&*result[1], &[0]);
	for slot in &result[2..] {
		assert!(slot.is_empty());
	}
}

#[test]
fn combine_seeds_with_bump_empty_seeds() {
	let bump = [255u8; 1];

	let result = combine_seeds_with_bump(&[], &bump).unwrap_or_else(|e| panic!("failed: {e:?}"));

	assert_eq!(&*result[0], &[255]);
	for slot in &result[1..] {
		assert!(slot.is_empty());
	}
}

#[test]
fn combine_seeds_with_bump_at_max_minus_one() {
	// MAX_SEEDS - 1 seeds leaves room for exactly one bump slot.
	let seeds: Vec<&[u8]> = (0..MAX_SEEDS - 1).map(|_| &[1u8][..]).collect();
	let bump = [7u8; 1];

	let result = combine_seeds_with_bump(&seeds, &bump).unwrap_or_else(|e| panic!("failed: {e:?}"));

	for (i, slot) in result.iter().enumerate().take(MAX_SEEDS - 1) {
		assert_eq!(&**slot, &[1u8], "slot {i} should be the original seed");
	}
	assert_eq!(&*result[MAX_SEEDS - 1], &[7]);
}

#[test]
fn combine_seeds_with_bump_too_many_seeds_fails() {
	// MAX_SEEDS seeds leaves no room for the bump — should return Err.
	let seeds: Vec<&[u8]> = (0..MAX_SEEDS).map(|_| &[1u8][..]).collect();
	let bump = [7u8; 1];

	let result = combine_seeds_with_bump(&seeds, &bump);
	assert!(result.is_err());
}

#[test]
fn max_permitted_data_increase_is_10_kib() {
	assert_eq!(MAX_PERMITTED_DATA_INCREASE, 10_240);
}

/// Verify that `realloc_account` and `realloc_account_zero` are exported and
/// have the expected function signatures. This is a compilation-level check;
/// the actual runtime behavior requires a Solana VM (e.g. mollusk-svm).
#[test]
fn realloc_functions_are_exported() {
	// Confirm that both symbols resolve to function pointers with compatible
	// signatures. We only inspect the type — calling them requires a live
	// AccountView which cannot be safely constructed outside the runtime.
	let _grow: fn(
		&pinocchio::AccountView,
		usize,
		&pinocchio::AccountView,
		&pinocchio::Address,
	) -> pinocchio::ProgramResult = realloc_account;
	let _grow_zero: fn(
		&pinocchio::AccountView,
		usize,
		&pinocchio::AccountView,
		&pinocchio::Address,
	) -> pinocchio::ProgramResult = realloc_account_zero;
}
