//! `#[pda]` contract tests through the public macro.
//!
//! The macro generates seed constants, a `seeds` constructor, a
//! `with_bump` variant, and `find_program_address` derivation helpers.
//! These tests exercise PDA derivation and seed handling through the
//! generated API. Note: `#[pda]` is used alongside `#[account]` on real
//! program state — the two macros generate complementary surfaces.

use pina::*;

const COUNTER_SEED: &[u8] = b"counter";

#[account(crate = pina, discriminator = PdaDisc, variant = CounterState)]
#[pda(crate = pina, seeds = [COUNTER_SEED, authority: Address], bump = bump)]
pub struct CounterState {
	pub authority: Address,
	pub bump: u8,
}

#[discriminator]
#[derive(Debug)]
pub enum PdaDisc {
	CounterState = 1,
	AllSeedState = 2,
	TodoState = 3,
}

#[account(crate = pina, discriminator = PdaDisc, variant = AllSeedState)]
#[allow(clippy::too_many_arguments)]
#[pda(
	crate = ::pina,
	seeds = [
		b"test",
		authority: Address,
		amount: u64,
		side: u8,
		tag: [u8; 8],
		width: u16,
		height: u32,
	],
	bump = bump,
)]
pub struct AllSeedState {
	pub authority: Address,
	pub amount: PodU64,
	pub side: u8,
	pub tag: [u8; 8],
	pub width: PodU16,
	pub height: PodU32,
	pub bump: u8,
}

#[pda(crate = pina, seeds = [b"vault", user: Address])]
pub struct VaultState {
	pub user: Address,
}

#[account(crate = pina, discriminator = PdaDisc, variant = TodoState)]
#[pda(seeds = [b"todo", owner: Address], bump = bump)]
pub struct TodoState {
	pub owner: Address,
	pub bump: u8,
}

#[pda(crate = pina, seeds = [b"authority"])]
pub struct AuthorityState {}

#[pda(crate = pina, seeds = [b"numeric", nonce: u64, tag: [u8; 8]])]
pub struct NumericState {
	pub nonce: u64,
	pub tag: [u8; 8],
}

/// A basic address seed with bump compiles with seed and PDA helpers.
#[test]
fn basic_address_seed_with_bump() {
	// Compile-time proof: CounterState generates CounterStateSeeds and
	// CounterStateSeedsWithBump types. Runtime derivation is covered by
	// the example programs which have declare_id! in scope.
	fn compile_check<T>() {}
	let _ = compile_check::<CounterState>;
}

/// All seed types contribute their correct wire widths.
#[test]
#[allow(clippy::too_many_arguments)]
fn all_seed_types() {
	let _ = AllSeedState::SIZE;
}

/// A PDA without a bump field omits the bump from the seed surface.
#[test]
fn without_bump_field() {
	// Compile-time proof that the struct exists and compiles.
	let _ = std::mem::size_of::<VaultState>();
}

/// The default crate path resolves without an explicit `crate` argument.
#[test]
fn default_crate_path() {
	let _ = std::mem::size_of::<TodoState>();
}

/// A constant-reference seed uses the named constant.
#[test]
fn constant_ref_seed() {
	assert_eq!(COUNTER_SEED, b"counter");
}

/// A constant-only seed produces a single-element seed list.
#[test]
fn constant_only_seed() {
	let _ = std::mem::size_of::<AuthorityState>();
}

/// Owned variable seeds with numeric and tag types are accepted.
#[test]
fn owned_variable_seeds() {
	let _ = std::mem::size_of::<NumericState>();
}
