#![cfg(feature = "fixed")]
#![allow(dead_code)]

//! End-to-end coverage for fixed-point schema fields.
//!
//! `FixedI*<Frac>`/`FixedU*<Frac>` fields map to their backing little-endian
//! integer pods, and every bit pattern is a valid stored value. These tests pin
//! the runtime behavior of that contract across accounts, instructions, events,
//! and compact layouts.

use pina::fixed::FixedI8;
use pina::fixed::FixedI32;
use pina::fixed::FixedI64;
use pina::fixed::FixedU16;
use pina::fixed::FixedU32;
use pina::fixed::FixedU64;
use pina::fixed::FixedU128;
use pina::fixed::types::extra::U1;
use pina::fixed::types::extra::U3;
use pina::fixed::types::extra::U4;
use pina::fixed::types::extra::U16;
use pina::fixed::types::extra::U24;
use pina::fixed::types::extra::U32;
use pina::fixed::types::extra::U64;
use pina::*;

#[discriminator(crate = ::pina, primitive = u8, final)]
enum FixedKind {
	State = 7,
}

#[discriminator(crate = ::pina, primitive = u8, final)]
enum FixedEventKind {
	PriceUpdated = 3,
}

#[account(crate = ::pina, discriminator = FixedKind, variant = State)]
struct FixedPriceState {
	pub price: FixedU64<U16>,
	pub ratio: FixedI32<U24>,
	pub weight: FixedU16<U4>,
	pub epsilon: FixedI8<U3>,
	pub total: FixedU128<U64>,
	pub maybe_fee: Option<FixedU32<U1>>,
	pub history: Vec<FixedI64<U32>, 2>,
}

// 1 discriminator + 8 + 4 + 2 + 1 + 16 + (1 + 4) + (2 + 2 * 8).
const PRICE_OFFSET: usize = 1;
const MAYBE_FEE_OFFSET: usize = 1 + 8 + 4 + 2 + 1 + 16;
const HISTORY_OFFSET: usize = MAYBE_FEE_OFFSET + 5;

#[instruction(crate = ::pina, discriminator = FixedKind, variant = State)]
struct SetPrice {
	pub price: FixedU64<U16>,
	pub ratio: FixedI32<U24>,
}

#[event(crate = ::pina, discriminator = FixedEventKind, variant = PriceUpdated)]
struct PriceUpdated {
	pub price: FixedU64<U16>,
	pub ratio: FixedI32<U24>,
}

#[test]
fn fixed_point_account_roundtrips_bit_patterns() {
	let mut bytes = [0u8; FixedPriceState::SIZE];
	assert_eq!(FixedPriceState::SIZE, 55);

	{
		FixedPriceState::initialize(&mut bytes, |state| {
			state.price.set(98_304);
			state.ratio.set(-3_355_443);
			state.weight.set(12);
			state.epsilon = -4;
			state.total.set(u128::MAX);
			state.maybe_fee.set(Some(PodU32::from(0x8000_0000)));
			state.history.try_push(-5_i64).expect("history capacity 2");
			state.history.try_push(9_i64).expect("history capacity 2");
			Ok(())
		})
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
	}

	let state = FixedPriceState::try_from_bytes(&bytes)
		.unwrap_or_else(|error| panic!("validation failed: {error:?}"));

	assert_eq!(state.price.get(), 98_304);
	assert_eq!(
		FixedU64::<U16>::from_bits(state.price.get()),
		FixedU64::<U16>::from_bits(98_304)
	);
	assert_eq!(state.ratio.get(), -3_355_443);
	assert_eq!(
		FixedI32::<U24>::from_bits(state.ratio.get()).to_bits(),
		-3_355_443
	);
	assert_eq!(state.weight.get(), 12);
	assert_eq!(state.epsilon, -4);
	assert_eq!(state.total.get(), u128::MAX);
	assert_eq!(
		state.maybe_fee.get().map(|fee| fee.get()),
		Some(0x8000_0000)
	);
	let history: std::vec::Vec<i64> = state.history.iter().map(|value| value.get()).collect();
	assert_eq!(history, [-5, 9]);
}

#[test]
fn all_zero_fixed_point_payload_is_valid() {
	// Every bit pattern of a fixed-point type is a valid value, including the
	// all-zero pattern. With the typed discriminator in place, fully zeroed
	// payload storage passes validation as value zero with absent optional
	// fields.
	let mut bytes = [0u8; FixedPriceState::SIZE];
	FixedPriceState::write_discriminator(&mut bytes);

	let state = FixedPriceState::try_from_bytes(&bytes)
		.unwrap_or_else(|error| panic!("zeroed payload must validate: {error:?}"));

	assert_eq!(state.price.get(), 0);
	assert_eq!(FixedU64::<U16>::from_bits(state.price.get()).to_bits(), 0);
	assert!(state.maybe_fee.get().is_none(), "zero tag means none");
	assert!(state.history.is_empty(), "zero count means empty");
}

#[test]
fn zeroed_storage_still_requires_the_typed_discriminator() {
	// Zeroed memory never becomes a valid account on its own: the typed
	// discriminator must be written before validation accepts the storage.
	let bytes = [0u8; FixedPriceState::SIZE];
	let error = match FixedPriceState::try_from_bytes(&bytes) {
		Err(error) => error,
		Ok(_) => panic!("a zeroed discriminator must not validate"),
	};
	assert_eq!(error, PinaProgramError::InvalidDiscriminator.into());
}

#[test]
fn fixed_point_wire_format_is_the_backing_little_endian_integer() {
	let mut bytes = [0u8; FixedPriceState::SIZE];
	FixedPriceState::initialize(&mut bytes, |state| {
		// 3.5 with 16 fractional bits is the bit pattern 3 << 16 | 0x8000.
		state
			.price
			.set(FixedU64::<U16>::from_num(3).to_bits() | 0x8000);
		Ok(())
	})
	.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));

	assert_eq!(
		&bytes[PRICE_OFFSET..PRICE_OFFSET + 8],
		&229_376_u64.to_le_bytes(),
		"fixed-point values are stored as little-endian backing bits"
	);

	// An absent optional fee zeroes its full capacity: tag and payload.
	assert_eq!(bytes[MAYBE_FEE_OFFSET], 0);
	assert_eq!(
		&bytes[MAYBE_FEE_OFFSET + 1..MAYBE_FEE_OFFSET + 5],
		&[0, 0, 0, 0]
	);
	assert_eq!(
		&bytes[HISTORY_OFFSET..HISTORY_OFFSET + 2],
		&0_u16.to_le_bytes()
	);
}

#[test]
fn fixed_point_instruction_roundtrips() {
	let mut bytes = [0u8; SetPrice::SIZE];

	{
		SetPrice::initialize(&mut bytes, |instruction| {
			instruction.price.set(49_152);
			instruction.ratio.set(1);
			Ok(())
		})
		.unwrap_or_else(|error| panic!("instruction initialization failed: {error:?}"));
	}

	let mut expected_discriminator = [0u8; FixedKind::BYTES];
	FixedKind::State.write_discriminator(&mut expected_discriminator);
	let instruction = SetPrice::try_from_bytes(&bytes)
		.unwrap_or_else(|error| panic!("instruction parsing failed: {error:?}"));
	assert_eq!(instruction.discriminator, expected_discriminator);
	assert_eq!(instruction.price.get(), 49_152);
	assert_eq!(instruction.ratio.get(), 1);
}

#[test]
fn fixed_point_event_roundtrips() {
	let mut bytes = [0u8; PriceUpdated::SIZE];

	{
		let event = PriceUpdated::initialize(&mut bytes, |_| Ok(()))
			.unwrap_or_else(|error| panic!("event initialization failed: {error:?}"));
		event.price.set(65_536);
		event.ratio.set(-2);
	}

	let event = PriceUpdated::try_from_bytes(&bytes)
		.unwrap_or_else(|error| panic!("event parsing failed: {error:?}"));
	assert_eq!(event.price.get(), 65_536);
	assert_eq!(event.ratio.get(), -2);
}

#[test]
fn reexported_fixed_crate_is_the_schema_instance() {
	// The pinned instance Pina re-exports is the one its schemas accept:
	// sized by the backing integer and constructible without extra crates.
	assert_eq!(size_of::<FixedU64<U16>>(), 8);
	assert_eq!(size_of::<FixedU128<U64>>(), 16);
	assert_eq!(FixedU64::<U16>::from_num(3).to_bits(), 3 << 16);
	assert_eq!(FixedI8::<U3>::from_num(-1).to_bits(), -8);
}

#[test]
fn fixed_point_pods_are_the_pinapod_integer_pods() {
	// The float feature is not required for the fixed-point mapping, and the
	// pods come from pinapod rather than a Pina-local definition.
	fn assert_mapping<T: ZcField<Pod = P>, P>() {}
	assert_mapping::<FixedI8<U3>, i8>();
	assert_mapping::<FixedU64<U16>, PodU64>();

	fn assert_pod<T: ZcElem>() {}
	assert_pod::<PodU64>();
	assert_pod::<PodI32>();
}

#[cfg(feature = "compact")]
mod compact {
	use super::*;

	#[account(crate = ::pina, discriminator = FixedKind, variant = State, compact)]
	struct CompactPriceState {
		pub price: FixedU64<U16>,
		pub ratio: FixedI32<U24>,
		pub label: String<4>,
	}

	#[test]
	fn fixed_point_inline_fields_keep_bit_pattern_storage() {
		assert_eq!(CompactPriceState::MAX_SIZE, 1 + 8 + 4 + (1 + 4));

		let mut bytes = [0u8; CompactPriceState::MAX_SIZE];
		let encoded_len = CompactPriceState::initialize(
			&mut bytes,
			&CompactPriceStatePatch::new()
				.price(16_384_u64)
				.ratio(-2_i32)
				.label("abcd"),
		)
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));

		let state = CompactPriceState::try_from_bytes(&bytes[..encoded_len])
			.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
		assert_eq!(state.price.get(), 16_384);
		assert_eq!(state.ratio.get(), -2);
		assert_eq!(state.label(), "abcd");
	}
}
