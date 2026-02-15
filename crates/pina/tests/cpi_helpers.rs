use pina::combine_seeds_with_bump;
use pinocchio::pubkey::MAX_SEEDS;

#[test]
fn combine_seeds_with_bump_basic() {
	let seed_a: &[u8] = b"escrow";
	let seed_b: &[u8] = &[1, 2, 3];
	let bump = [42u8; 1];

	let result = combine_seeds_with_bump(&[seed_a, seed_b], &bump);

	assert_eq!(result[0], b"escrow");
	assert_eq!(result[1], &[1, 2, 3]);
	assert_eq!(result[2], &[42]);
	// Remaining slots should be empty.
	for slot in &result[3..] {
		assert!(slot.is_empty());
	}
}

#[test]
fn combine_seeds_with_bump_single_seed() {
	let seed: &[u8] = b"hello";
	let bump = [0u8; 1];

	let result = combine_seeds_with_bump(&[seed], &bump);

	assert_eq!(result[0], b"hello");
	assert_eq!(result[1], &[0]);
	for slot in &result[2..] {
		assert!(slot.is_empty());
	}
}

#[test]
fn combine_seeds_with_bump_empty_seeds() {
	let bump = [255u8; 1];

	let result = combine_seeds_with_bump(&[], &bump);

	assert_eq!(result[0], &[255]);
	for slot in &result[1..] {
		assert!(slot.is_empty());
	}
}

#[test]
fn combine_seeds_with_bump_at_max_minus_one() {
	// MAX_SEEDS - 1 seeds leaves room for exactly one bump slot.
	let seeds: Vec<&[u8]> = (0..MAX_SEEDS - 1).map(|_| &[1u8][..]).collect();
	let bump = [7u8; 1];

	let result = combine_seeds_with_bump(&seeds, &bump);

	for (i, slot) in result.iter().enumerate().take(MAX_SEEDS - 1) {
		assert_eq!(*slot, &[1u8], "slot {i} should be the original seed");
	}
	assert_eq!(result[MAX_SEEDS - 1], &[7]);
}

// NOTE: a `#[should_panic]` test for `seeds.len() >= MAX_SEEDS` is omitted
// because the pinocchio `no_std` runtime abort cannot be caught by the standard
// test harness.
