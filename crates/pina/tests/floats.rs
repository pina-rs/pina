#![cfg(feature = "floats")]
#![allow(dead_code)]

//! End-to-end coverage for IEEE-754 schema fields.
//!
//! `f32`/`f64` fields are stored as the complete little-endian bit pattern of
//! their backing integer through `pinapod`'s `PodF32`/`PodF64`, and every bit
//! pattern is a valid stored value. These tests pin the runtime behavior of
//! that contract across accounts, instructions, events, and compact layouts.

use pina::*;

#[discriminator(crate = ::pina, primitive = u8, final)]
enum FloatKind {
	State = 7,
}

#[discriminator(crate = ::pina, primitive = u8, final)]
enum FloatEventKind {
	Reading = 4,
}

#[account(crate = ::pina, discriminator = FloatKind, variant = State)]
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

#[instruction(crate = ::pina, discriminator = FloatKind, variant = State)]
struct RecordReading {
	pub temperature: f32,
	pub depth: f64,
}

#[event(crate = ::pina, discriminator = FloatEventKind, variant = Reading)]
struct FloatEvent {
	pub reading: f32,
}

#[test]
fn float_account_roundtrips_values_under_the_hood() {
	let mut bytes = [0u8; FloatReadingState::SIZE];
	assert_eq!(FloatReadingState::SIZE, 28);

	{
		FloatReadingState::initialize(&mut bytes, |state| {
			// Float fields expose native float accessors; the bit conversion
			// happens inside the generated storage, exactly like `u32` fields
			// convert through `PodU32`.
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
fn float_event_roundtrips() {
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

#[test]
fn float_pods_are_the_pinapod_pods() {
	// The pods and their `ZcField` mappings come from pinapod; Pina only
	// re-exports them, so a Pina schema and a direct pinapod schema agree.
	fn assert_mapping<T: ZcField<Pod = P>, P>() {}
	assert_mapping::<f32, PodF32>();
	assert_mapping::<f64, PodF64>();

	assert_eq!(size_of::<PodF32>(), 4);
	assert_eq!(size_of::<PodF64>(), 8);
	assert_eq!(PodF32::from(1.5).to_bits(), 1.5_f32.to_bits());
	assert_eq!(PodF64::from(-2.25).to_bits(), (-2.25_f64).to_bits());
}

#[test]
fn the_fixed_crate_is_not_required_for_float_fields() {
	// `floats` does not pull in the `fixed` crate: only `pina::fixed` and the
	// fixed-point field grammar need it. This test compiles in the
	// `floats`-only lane, where `fixed` is disabled.
	let mut bytes = [0u8; FloatReadingState::SIZE];
	FloatReadingState::initialize(&mut bytes, |state| {
		state.temperature.set(1.0);
		Ok(())
	})
	.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));
}

#[cfg(feature = "compact")]
mod compact {
	use super::*;

	#[account(crate = ::pina, discriminator = FloatKind, variant = State, compact)]
	struct CompactFloatState {
		pub bias: f32,
		pub maybe_gain: Option<f32>,
		pub label: String<4>,
	}

	#[test]
	fn float_inline_fields_keep_bit_pattern_storage() {
		assert_eq!(CompactFloatState::MAX_SIZE, 1 + 4 + (1 + 4) + (1 + 4));

		let mut bytes = [0u8; CompactFloatState::MAX_SIZE];
		let encoded_len = CompactFloatState::initialize(
			&mut bytes,
			&CompactFloatStatePatch::new()
				.bias(0.5_f32)
				// The patch setter takes the native float: `pinapod` maps
				// `f32` through `ZcField`, so no pod spelling is needed.
				.maybe_gain(Some(2.0_f32))
				.label("abcd"),
		)
		.unwrap_or_else(|error| panic!("initialization failed: {error:?}"));

		let state = CompactFloatState::try_from_bytes(&bytes[..encoded_len])
			.unwrap_or_else(|error| panic!("validation failed: {error:?}"));
		assert_eq!(state.bias.get(), 0.5);
		assert_eq!(state.maybe_gain.get().map(|gain| gain.get()), Some(2.0));
		assert_eq!(state.label(), "abcd");
	}
}
