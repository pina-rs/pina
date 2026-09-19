//! Named `const` capacities in account and instruction schemas.
//!
//! A capacity may name a constant instead of repeating a literal. Resolution
//! happens before the schema grammar runs, so the generated proofs, the
//! `MAX_SIZE`/`SIZE` constants, and the ABI document are identical to the
//! literal spelling. These tests pin that equivalence, because a resolved
//! capacity that reached the ABI layer as a name would silently change the
//! on-chain layout contract.

use pina::*;

#[discriminator]
#[derive(Debug)]
pub enum CapacityDisc {
	Roster = 0,
	AddMember = 1,
}

/// Declared at module scope so resolution reads it from crate source rather
/// than from the item being expanded.
pub mod limits {
	/// Maximum roster members.
	pub const MAX_MEMBERS: usize = 4;
	/// Maximum title byte length.
	pub const MAX_TITLE: usize = 12;
	/// A bound written in terms of another constant.
	pub const MAX_NOTES: usize = MAX_MEMBERS * 2;
}

/// A compact account whose every tail capacity is a named constant.
#[account(crate = pina, discriminator = CapacityDisc, variant = Roster, compact)]
pub struct Roster {
	pub bump: u8,
	pub members: Vec<Address, limits::MAX_MEMBERS>,
	pub title: String<limits::MAX_TITLE>,
	pub notes: Option<Vec<u8, limits::MAX_NOTES>>,
}

/// The same compact account with the literals spelled out.
#[account(crate = pina, discriminator = CapacityDisc, variant = Roster, compact)]
pub struct LiteralRoster {
	pub bump: u8,
	pub members: Vec<Address, 4>,
	pub title: String<12>,
	pub notes: Option<Vec<u8, 8>>,
}

/// A compact account with no optional tail, so `projected_bytes` is generated.
#[account(crate = pina, discriminator = CapacityDisc, variant = Roster, compact)]
pub struct PlainRoster {
	pub bump: u8,
	pub members: Vec<Address, limits::MAX_MEMBERS>,
	pub title: String<limits::MAX_TITLE>,
}

/// The same non-optional compact account with literals.
#[account(crate = pina, discriminator = CapacityDisc, variant = Roster, compact)]
pub struct LiteralPlainRoster {
	pub bump: u8,
	pub members: Vec<Address, 4>,
	pub title: String<12>,
}

/// A fixed account and instruction argument sized by constants.
#[account(crate = pina, discriminator = CapacityDisc, variant = Roster)]
pub struct Words {
	pub values: [u64; limits::MAX_MEMBERS],
}

#[instruction(crate = pina, discriminator = CapacityDisc, variant = AddMember)]
pub struct AddMember {
	pub slot: [u8; limits::MAX_TITLE],
	pub weights: [u16; limits::MAX_NOTES],
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn compact_tail_capacities_resolve_to_their_values() {
		assert_eq!(Roster::MEMBERS_CAPACITY, 4);
		assert_eq!(Roster::TITLE_CAPACITY, 12);
		assert_eq!(Roster::NOTES_CAPACITY, 8);
	}

	#[test]
	fn a_named_capacity_matches_its_literal_spelling() {
		// Every size the ABI layer depends on must agree between the two
		// spellings; a mismatch here is a silently wrong layout.
		assert_eq!(Roster::HEADER_SIZE, LiteralRoster::HEADER_SIZE);
		assert_eq!(Roster::MIN_SIZE, LiteralRoster::MIN_SIZE);
		assert_eq!(Roster::MAX_SIZE, LiteralRoster::MAX_SIZE);
		assert_eq!(Roster::TAIL_ALIGNMENT, LiteralRoster::TAIL_ALIGNMENT);
	}

	#[test]
	fn a_named_capacity_matches_literal_projected_bytes() {
		// Without an optional tail the generated `projected_bytes` exists, and
		// it must compute the same length for both spellings.
		assert_eq!(
			PlainRoster::projected_bytes(4, 12),
			LiteralPlainRoster::projected_bytes(4, 12),
		);
	}

	#[test]
	fn a_named_array_length_sizes_a_fixed_account() {
		assert_eq!(Words::SIZE, CapacityDisc::BYTES + 4 * 8);
	}

	#[test]
	fn a_named_array_length_sizes_an_instruction_argument() {
		assert_eq!(AddMember::SIZE, CapacityDisc::BYTES + 12 + 8 * 2);
	}

	#[test]
	fn a_compact_account_initializes_at_its_declared_capacities() {
		let mut bytes = [0u8; Roster::MAX_SIZE];
		let encoded = Roster::initialize(&mut bytes, &RosterPatch::new())
			.unwrap_or_else(|error| panic!("initialization must succeed: {error}"));

		assert_eq!(encoded, Roster::MIN_SIZE);
	}

	/// A constant used only in a schema position must stay live.
	///
	/// Resolution replaces the name with its value, so without the generated
	/// reference proof `limits::MAX_NOTES` would be dead code and `lint:all`
	/// (which denies warnings) would reject this file. This test deliberately
	/// does not read the constant: the gate is that the file compiles at all.
	#[test]
	fn a_schema_only_constant_resolves_without_becoming_dead_code() {
		assert_eq!(Roster::NOTES_CAPACITY, 8);
	}
}
