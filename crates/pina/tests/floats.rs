#![cfg(feature = "floats")]
#![allow(dead_code)]

//! End-to-end coverage for float and fixed-point schema fields.
//!
//! Every fractional schema type is stored as the complete bit pattern of its
//! backing little-endian integer pod: `f32`/`f64` fields convert under the
//! hood through `PodF32`/`PodF64`, and `FixedI*<Frac>`/`FixedU*<Frac>` fields
//! map to their backing integer pods. These tests pin the runtime behavior of
//! that contract across accounts, instructions, events, and compact layouts.

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

#[account(crate = ::pina, discriminator = FixedKind, variant = State)]
struct FloatReadingState {
	pub temperature: f32,
	pub depth: f64,
	pub maybe_bias: Option<f32>,
	pub samples: Vec<f32, 2>,
}

// 1 discriminator + 4 + 8 + (1 + 4) + (2 + 2 * 4).
const TEMPERATURE_OFFSET: usize = 1;
const DEPTH_OFFSET: usize = TEMPERATURE_OFFSET + 4;
const MAYBE_BIAS_OFFSET: usize = DEPTH_OFFSET + 8;
const SAMPLES_OFFSET: usize = MAYBE_BIAS_OFFSET + 5;
#[instruction(crate = ::pina, discriminator = FixedKind, variant = State)]
struct RecordReading {
	pub temperature: f32,
	pub depth: f64,
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
	assert_eq!(error, ProgramError::InvalidAccountData);
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
fn float_account_roundtrips_values_under_the_hood() {
	let mut bytes = [0u8; FloatReadingState::SIZE];
	assert_eq!(FloatReadingState::SIZE, 28);

	{
		FloatReadingState::initialize(&mut bytes, |state| {
			// Float fields expose native float accessors; the bit conversion
			// happens inside the generated storage, exactly like `u32`
			// fields convert through `PodU32`.
			state.temperature.set(-12.5);
			state.depth.set(3.125);
			state.maybe_bias.set(Some(PodF32::from(0.5)));
			state.samples.try_push(1.0_f32).expect("samples capacity 2");
			state
				.samples
				.try_push(-2.0_f32)
				.expect("samples capacity 2");
			Ok(())
		})
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
	}

	let state = FloatReadingState::try_from_bytes(&bytes)
		.unwrap_or_else(|error| panic!("validation failed: {error:?}"));

	assert_eq!(state.temperature.get(), -12.5);
	assert_eq!(state.depth.get(), 3.125);
	assert_eq!(state.maybe_bias.get().map(|bias| bias.get()), Some(0.5));
	let samples: std::vec::Vec<f32> = state.samples.iter().map(|value| value.get()).collect();
	assert_eq!(samples, [1.0, -2.0]);
}

#[test]
fn float_storage_preserves_nan_bit_patterns() {
	// Every bit pattern is a valid stored value; NaN payloads are preserved
	// bit-exactly by the pod's bitwise accessors.
	let mut bytes = [0u8; FloatReadingState::SIZE];
	FloatReadingState::initialize(&mut bytes, |state| {
		state.temperature.set_bits(0x7fc0_0001);
		state.depth.set(f64::NAN);
		Ok(())
	})
	.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));

	let state = FloatReadingState::try_from_bytes(&bytes)
		.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
	assert_eq!(state.temperature.to_bits(), 0x7fc0_0001);
	assert!(state.depth.get().is_nan());
}

#[test]
fn float_wire_format_is_the_backing_little_endian_bits() {
	let mut bytes = [0u8; FloatReadingState::SIZE];
	FloatReadingState::initialize(&mut bytes, |state| {
		state.temperature.set(1.0);
		Ok(())
	})
	.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));

	assert_eq!(
		&bytes[TEMPERATURE_OFFSET..TEMPERATURE_OFFSET + 4],
		&1.0_f32.to_bits().to_le_bytes(),
		"float fields are stored as little-endian backing bits"
	);
	assert_eq!(
		&bytes[DEPTH_OFFSET..DEPTH_OFFSET + 8],
		&0_f64.to_bits().to_le_bytes()
	);
	// An absent optional bias zeroes tag and payload; a zero sample count
	// means empty.
	assert_eq!(&bytes[MAYBE_BIAS_OFFSET..SAMPLES_OFFSET], &[0, 0, 0, 0, 0]);
	assert_eq!(
		&bytes[SAMPLES_OFFSET..SAMPLES_OFFSET + 2],
		&0_u16.to_le_bytes()
	);
}

#[test]
fn float_instruction_roundtrips() {
	let mut bytes = [0u8; RecordReading::SIZE];

	{
		RecordReading::initialize(&mut bytes, |instruction| {
			instruction.temperature.set(20.25);
			instruction.depth.set(-1.5);
			Ok(())
		})
		.unwrap_or_else(|error| panic!("instruction initialization failed: {error:?}"));
	}

	let instruction = RecordReading::try_from_bytes(&bytes)
		.unwrap_or_else(|error| panic!("instruction parsing failed: {error:?}"));
	assert_eq!(instruction.temperature.get(), 20.25);
	assert_eq!(instruction.depth.get(), -1.5);
}

#[test]
fn float_event_and_compact_roundtrip() {
	let mut bytes = [0u8; FloatEvent::SIZE];

	{
		let event = FloatEvent::initialize(&mut bytes, |_| Ok(()))
			.unwrap_or_else(|error| panic!("event initialization failed: {error:?}"));
		event.reading.set(9.75);
	}

	let event = FloatEvent::try_from_bytes(&bytes)
		.unwrap_or_else(|error| panic!("event parsing failed: {error:?}"));
	assert_eq!(event.reading.get(), 9.75);
}

#[discriminator(crate = ::pina, primitive = u8, final)]
enum FloatEventKind {
	Reading = 4,
}

#[event(crate = ::pina, discriminator = FloatEventKind, variant = Reading)]
struct FloatEvent {
	pub reading: f32,
}

#[cfg(feature = "compact")]
mod compact {
	use super::*;

	#[account(crate = ::pina, discriminator = FixedKind, variant = State, compact)]
	struct CompactFixedState {
		pub price: FixedU64<U16>,
		pub bias: f32,
		pub maybe_gain: Option<f32>,
		pub label: String<4>,
	}

	#[test]
	fn fractional_inline_fields_keep_bit_pattern_storage() {
		assert_eq!(CompactFixedState::MAX_SIZE, 1 + 8 + 4 + (1 + 4) + (1 + 4));

		let mut bytes = [0u8; CompactFixedState::MAX_SIZE];
		let encoded_len = CompactFixedState::initialize(
			&mut bytes,
			&CompactFixedStatePatch::new()
				.price(16_384_u64)
				.bias(0.5_f32)
				.maybe_gain(Some(PodF32::from(2.0_f32)))
				.label("abcd"),
		)
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));

		let state = CompactFixedState::try_from_bytes(&bytes[..encoded_len])
			.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
		assert_eq!(state.price.get(), 16_384);
		assert_eq!(state.bias.get(), 0.5);
		assert_eq!(state.maybe_gain.get().map(|gain| gain.get()), Some(2.0));
		assert_eq!(state.label(), "abcd");
	}
}
