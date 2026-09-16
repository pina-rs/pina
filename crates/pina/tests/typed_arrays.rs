#![allow(dead_code)]

//! End-to-end coverage for typed fixed-array schema fields.
//!
//! `[T; N]` fields store `[PodT; N]` little-endian with no length prefix, and
//! validation recurses per element. These tests pin that contract across
//! accounts, instructions, events, and compact layouts.

use pina::*;

#[discriminator(crate = ::pina, primitive = u8, final)]
enum WeightsKind {
	Table = 7,
}

#[discriminator(crate = ::pina, primitive = u8, final)]
enum WeightsEventKind {
	TableReweighted = 3,
}

#[account(crate = ::pina, discriminator = WeightsKind, variant = Table)]
struct WeightsState {
	pub outcomes: [u64; 4],
	pub flags: [bool; 2],
	pub owners: [Address; 2],
	pub nested: [[u8; 4]; 2],
	pub maybe: Option<[u64; 2]>,
}

// 1 discriminator + 4 * 8 + 2 + 2 * 32 + 2 * 4 + (1 + 2 * 8).
const OUTCOMES_OFFSET: usize = 1;
const FLAGS_OFFSET: usize = 1 + 4 * 8;

#[instruction(crate = ::pina, discriminator = WeightsKind, variant = Table)]
struct SetOutcomes {
	pub outcomes: [u64; 4],
}

#[event(crate = ::pina, discriminator = WeightsEventKind, variant = TableReweighted)]
struct TableReweighted {
	pub outcomes: [u64; 4],
}

#[test]
fn typed_array_account_roundtrips_elements() {
	let mut bytes = [0u8; WeightsState::SIZE];
	assert_eq!(WeightsState::SIZE, 124);

	{
		WeightsState::initialize(&mut bytes, |state| {
			state.outcomes[0] = PodU64::from(0x0102_0304_0506_0708);
			state.outcomes[1] = PodU64::from(1);
			state.outcomes[2] = PodU64::from(2);
			state.outcomes[3] = PodU64::from(3);
			state.flags[1] = true.into();
			state.owners[1] = Address::new_from_array([7; 32]);
			state.nested[1] = [9, 9, 9, 9];
			state.maybe.set(Some([PodU64::from(5), PodU64::from(6)]));
			Ok(())
		})
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
	}

	assert_eq!(
		bytes[OUTCOMES_OFFSET..OUTCOMES_OFFSET + 8],
		0x0102_0304_0506_0708_u64.to_le_bytes()
	);
	assert_eq!(bytes[FLAGS_OFFSET + 1], 1, "canonical true byte");

	let state = WeightsState::try_from_bytes(&bytes)
		.unwrap_or_else(|error| panic!("validation failed: {error:?}"));

	assert_eq!(state.outcomes()[0].get(), 0x0102_0304_0506_0708);
	assert_eq!(state.outcomes()[3].get(), 3);
	assert_eq!(state.flags()[1].get(), true);
	assert_eq!(state.owners()[1], Address::new_from_array([7; 32]));
	assert_eq!(state.nested()[1], [9, 9, 9, 9]);
	assert_eq!(
		state.maybe.get().map(|values| values[1].get()),
		Some(6),
		"Option<[u64; N]> resolves its payload through the array pod"
	);
}

#[test]
fn typed_array_account_rejects_invalid_bool_element() {
	let mut bytes = [0u8; WeightsState::SIZE];
	WeightsState::write_discriminator(&mut bytes);
	bytes[FLAGS_OFFSET + 1] = 2;

	// Pina surfaces schema violations as the generic invalid-data error; the
	// per-element cause stays internal to the pod validation layer.
	assert!(matches!(
		WeightsState::try_from_bytes(&bytes),
		Err(ProgramError::InvalidAccountData)
	));
}

#[test]
fn typed_array_instruction_roundtrips() {
	let mut bytes = [0u8; SetOutcomes::SIZE];

	{
		SetOutcomes::initialize(&mut bytes, |instruction| {
			instruction.outcomes[0] = PodU64::from(49_152);
			instruction.outcomes[1] = PodU64::from(1);
			instruction.outcomes[2] = PodU64::from(2);
			instruction.outcomes[3] = PodU64::from(3);
			Ok(())
		})
		.unwrap_or_else(|error| panic!("instruction initialization failed: {error:?}"));
	}

	let instruction = SetOutcomes::try_from_bytes(&bytes)
		.unwrap_or_else(|error| panic!("instruction parsing failed: {error:?}"));
	assert_eq!(instruction.outcomes()[0].get(), 49_152);
	assert_eq!(instruction.outcomes()[3].get(), 3);

	// Instruction data is the discriminator followed by the little-endian pod
	// layout, so typed arrays need no client-side packing.
	assert_eq!(bytes[1..9], 49_152_u64.to_le_bytes());
}

#[test]
fn typed_array_event_roundtrips() {
	let mut bytes = [0u8; TableReweighted::SIZE];

	{
		let event = TableReweighted::initialize(&mut bytes, |_| Ok(()))
			.unwrap_or_else(|error| panic!("event initialization failed: {error:?}"));
		event.outcomes[0] = PodU64::from(65_536);
		event.outcomes[1] = PodU64::from(1);
		event.outcomes[2] = PodU64::from(2);
		event.outcomes[3] = PodU64::from(3);
	}

	let event = TableReweighted::try_from_bytes(&bytes)
		.unwrap_or_else(|error| panic!("event parsing failed: {error:?}"));
	assert_eq!(event.outcomes()[0].get(), 65_536);
}

#[cfg(feature = "compact")]
mod compact {
	use super::*;

	#[discriminator(crate = ::pina, primitive = u8, final)]
	enum CompactWeightsKind {
		Table = 7,
	}

	#[account(
		crate = ::pina,
		discriminator = CompactWeightsKind,
		variant = Table,
		compact
	)]
	struct CompactTable {
		pub weights: [u64; 2],
		pub label: String<8>,
	}

	#[test]
	fn typed_array_inline_fields_accept_native_patch_values() {
		assert_eq!(CompactTable::MAX_SIZE, 1 + 2 * 8 + 1 + 8);

		let mut bytes = [0u8; CompactTable::MAX_SIZE];
		let encoded_len = CompactTable::initialize(
			&mut bytes,
			&CompactTablePatch::new().weights([5_u64, 6]).label("hi"),
		)
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));

		let table = CompactTable::try_from_bytes(&bytes[..encoded_len])
			.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
		assert_eq!(table.weights[0].get(), 5);
		assert_eq!(table.weights[1].get(), 6);
		assert_eq!(table.label(), "hi");
	}
}

/// Typed arrays compose with the rest of the closed grammar.
///
/// Pins which surrounding shapes accept a `[T; N]` element: `Vec` elements,
/// `Option` payloads at both levels, and fixed-point elements. The layout
/// assertions make the storage contract explicit rather than implied.
#[cfg(feature = "fixed")]
mod composition {
	use pina::fixed::FixedU64;
	use pina::fixed::types::extra::U16;
	use pina::*;

	#[discriminator(crate = ::pina, primitive = u8, final)]
	enum ComposeKind {
		Wide = 7,
	}

	#[account(crate = ::pina, discriminator = ComposeKind, variant = Wide)]
	struct Wide {
		pub matrix: Vec<[u64; 2], 3>,
		pub maybe_row: Option<[u64; 2]>,
		pub maybe_prices: Option<[FixedU64<U16>; 2]>,
		pub deep: [[[u8; 2]; 2]; 2],
	}

	#[test]
	fn array_compositions_share_the_element_layout() {
		// 1 discriminator
		// + (2 prefix + 3 * 2 * 8) matrix
		// + (1 + 2 * 8) maybe_row
		// + (1 + 2 * 8) maybe_prices
		// + 2 * 2 * 2 deep
		assert_eq!(Wide::SIZE, 1 + 50 + 17 + 17 + 8);
	}

	#[test]
	fn array_compositions_roundtrip() {
		let mut bytes = [0u8; Wide::SIZE];
		{
			Wide::initialize(&mut bytes, |state| {
				// Pod containers store pod values, exactly like scalar fields:
				// `[u64; 2]` fields store `[PodU64; 2]`.
				state.matrix.try_push([PodU64::from(1), PodU64::from(2)])?;
				state.matrix.try_push([PodU64::from(3), PodU64::from(4)])?;
				state
					.maybe_row
					.set(Some([PodU64::from(5), PodU64::from(6)]));
				state.maybe_prices.set(Some([
					PodU64::from(FixedU64::<U16>::from_num(7).to_bits()),
					PodU64::from(FixedU64::<U16>::from_num(8).to_bits()),
				]));
				state.deep[1][0] = [9, 10];
				Ok(())
			})
			.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
		}

		let state = Wide::try_from_bytes(&bytes)
			.unwrap_or_else(|error| panic!("validation failed: {error:?}"));

		assert_eq!(state.matrix.len(), 2);
		assert_eq!(state.matrix[1][0].get(), 3);
		assert_eq!(state.matrix[1][1].get(), 4);
		assert_eq!(
			state.maybe_row.get().map(|row| row[1].get()),
			Some(6),
			"Option<[T; N]> resolves through the array pod"
		);
		assert_eq!(
			state
				.maybe_prices
				.get()
				.map(|prices| FixedU64::<U16>::from_bits(prices[1].get()).to_num::<u64>()),
			Some(8),
			"fixed-point elements keep their bit-pattern storage"
		);
		assert_eq!(state.deep[1][0], [9, 10]);
	}
}
