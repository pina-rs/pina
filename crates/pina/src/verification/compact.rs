//! Bounded Kani state-machine proofs for three independently sized compact tails.

use core::mem::size_of;

use crate::*;

#[pina_macros::discriminator(crate = crate, primitive = u8, final)]
enum CompactProofKind {
	Layout = 13,
	Aligned = 14,
}

#[pina_macros::account(crate = crate, discriminator = CompactProofKind, variant = Layout, compact)]
struct CompactProofState {
	pub marker: u8,
	pub bytes: Vec<u8, 2>,
	pub words: Vec<u16, 2>,
	pub triples: Vec<[u8; 3], 2>,
}

#[pina_macros::account(
	crate = crate,
	discriminator = CompactProofKind,
	variant = Aligned,
	compact
)]
struct AlignedCompactProofState {
	pub marker: u8,
	pub words: Vec<u16, 2>,
	pub quads: Vec<u32, 2>,
}

const BYTES_LENGTH_OFFSET: usize = 2;
const WORDS_LENGTH_OFFSET: usize = 4;
const TRIPLES_LENGTH_OFFSET: usize = 6;

fn decode_length(bytes: &[u8], offset: usize) -> usize {
	usize::from(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]))
}

#[kani::proof]
fn quick_schema_size_validation_matches_declared_bounds_and_alignment() {
	let size: usize = kani::any();
	let layout_expected = size >= CompactProofState::HEADER_SIZE
		&& size <= CompactProofState::MAX_SIZE
		&& (size - CompactProofState::HEADER_SIZE)
			.is_multiple_of(CompactProofState::TAIL_ALIGNMENT);
	let aligned_expected = size >= AlignedCompactProofState::HEADER_SIZE
		&& size <= AlignedCompactProofState::MAX_SIZE
		&& (size - AlignedCompactProofState::HEADER_SIZE)
			.is_multiple_of(AlignedCompactProofState::TAIL_ALIGNMENT);

	assert_eq!(
		CompactProofState::validate_size(size).is_ok(),
		layout_expected
	);
	assert_eq!(
		AlignedCompactProofState::validate_size(size).is_ok(),
		aligned_expected
	);
	assert_eq!(CompactProofState::TAIL_ALIGNMENT, 1);
	assert_eq!(AlignedCompactProofState::TAIL_ALIGNMENT, 2);
}

#[kani::proof]
#[kani::unwind(16)]
fn compact_initialize_accepts_every_valid_aligned_size_and_starts_empty() {
	let len: usize = kani::any();
	kani::assume(len <= AlignedCompactProofState::MAX_SIZE + 1);
	let mut data = [0xa5u8; AlignedCompactProofState::MAX_SIZE + 1];
	let expected = AlignedCompactProofState::validate_size(len).is_ok();
	let result = AlignedCompactProofState::initialize(&mut data[..len]);

	assert_eq!(result.is_ok(), expected);
	if let Ok(mut state) = result {
		assert_eq!(
			state.projected_size(),
			AlignedCompactProofState::HEADER_SIZE
		);
		assert_eq!(state.commit(), Ok(AlignedCompactProofState::HEADER_SIZE));
	}
	if expected {
		let state = AlignedCompactProofState::try_from_bytes(
			&data[..AlignedCompactProofState::HEADER_SIZE],
		)
		.unwrap();
		assert!(state.words().is_empty());
		assert!(state.quads().is_empty());
	}
}

#[kani::proof]
#[kani::unwind(32)]
fn compact_every_shorter_prefix_rejects_active_tail_truncation() {
	let byte_values: [u8; 2] = kani::any();
	let word_values = [
		PodU16::from(kani::any::<u16>()),
		PodU16::from(kani::any::<u16>()),
	];
	let triple_values: [[u8; 3]; 2] = kani::any();
	let byte_len: usize = kani::any();
	let word_len: usize = kani::any();
	let triple_len: usize = kani::any();
	kani::assume(byte_len <= byte_values.len());
	kani::assume(word_len <= word_values.len());
	kani::assume(triple_len <= triple_values.len());
	let mut data = [0u8; CompactProofState::MAX_SIZE];
	let committed_size = {
		let mut state = CompactProofState::initialize(&mut data).unwrap();
		state.set_bytes(&byte_values[..byte_len]).unwrap();
		state.set_words(&word_values[..word_len]).unwrap();
		state.set_triples(&triple_values[..triple_len]).unwrap();
		state.commit().unwrap()
	};
	let shorter_len: usize = kani::any();
	kani::assume(shorter_len < committed_size);

	assert!(CompactProofState::validate_account_data(&data[..shorter_len]).is_err());
}

#[kani::proof]
#[kani::unwind(32)]
fn compact_offsets_are_ordered_disjoint_and_within_the_committed_prefix() {
	let byte_values: [u8; 2] = kani::any();
	let word_values = [
		PodU16::from(kani::any::<u16>()),
		PodU16::from(kani::any::<u16>()),
	];
	let triple_values: [[u8; 3]; 2] = kani::any();
	let byte_len: usize = kani::any();
	let word_len: usize = kani::any();
	let triple_len: usize = kani::any();
	kani::assume(byte_len <= byte_values.len());
	kani::assume(word_len <= word_values.len());
	kani::assume(triple_len <= triple_values.len());
	let mut data = [0u8; CompactProofState::MAX_SIZE];
	let committed_size;

	{
		let mut state = CompactProofState::initialize(&mut data)
			.unwrap_or_else(|error| panic!("valid compact storage rejected: {error:?}"));
		state
			.set_bytes(&byte_values[..byte_len])
			.unwrap_or_else(|error| panic!("bounded byte tail rejected: {error:?}"));
		state
			.set_words(&word_values[..word_len])
			.unwrap_or_else(|error| panic!("bounded word tail rejected: {error:?}"));
		state
			.set_triples(&triple_values[..triple_len])
			.unwrap_or_else(|error| panic!("bounded triple tail rejected: {error:?}"));

		let projected_size = state.projected_size();
		committed_size = state
			.commit()
			.unwrap_or_else(|error| panic!("bounded compact state failed to commit: {error:?}"));
		assert_eq!(committed_size, projected_size);
	}

	let bytes_start = CompactProofState::HEADER_SIZE;
	let bytes_end = bytes_start + byte_len;
	let words_start = bytes_end;
	let words_end = words_start + word_len * size_of::<PodU16>();
	let triples_start = words_end;
	let triples_end = triples_start + triple_len * size_of::<[u8; 3]>();

	assert!(bytes_start <= bytes_end);
	assert_eq!(bytes_end, words_start);
	assert!(words_start <= words_end);
	assert_eq!(words_end, triples_start);
	assert!(triples_start <= triples_end);
	assert_eq!(triples_end, committed_size);
	assert!(committed_size <= data.len());
	assert_eq!(decode_length(&data, BYTES_LENGTH_OFFSET), byte_len);
	assert_eq!(decode_length(&data, WORDS_LENGTH_OFFSET), word_len);
	assert_eq!(decode_length(&data, TRIPLES_LENGTH_OFFSET), triple_len);

	let state = CompactProofState::try_from_bytes(&data[..committed_size])
		.unwrap_or_else(|error| panic!("committed compact prefix rejected: {error:?}"));
	assert_eq!(state.bytes(), &byte_values[..byte_len]);
	assert_eq!(state.words(), &word_values[..word_len]);
	assert_eq!(state.triples(), &triple_values[..triple_len]);
}

#[kani::proof]
#[kani::unwind(32)]
fn compact_changing_an_earlier_tail_preserves_and_shifts_every_later_tail() {
	let initial_bytes: [u8; 2] = kani::any();
	let replacement_bytes: [u8; 2] = kani::any();
	let words = [
		PodU16::from(kani::any::<u16>()),
		PodU16::from(kani::any::<u16>()),
	];
	let triples: [[u8; 3]; 2] = kani::any();
	let initial_len: usize = kani::any();
	let replacement_len: usize = kani::any();
	let word_len: usize = kani::any();
	let triple_len: usize = kani::any();
	kani::assume(initial_len <= initial_bytes.len());
	kani::assume(replacement_len <= replacement_bytes.len());
	kani::assume(word_len <= words.len());
	kani::assume(triple_len <= triples.len());
	let mut data = [0u8; CompactProofState::MAX_SIZE];

	{
		let mut state = CompactProofState::initialize(&mut data)
			.unwrap_or_else(|error| panic!("valid compact storage rejected: {error:?}"));
		state.set_bytes(&initial_bytes[..initial_len]).unwrap();
		state.set_words(&words[..word_len]).unwrap();
		state.set_triples(&triples[..triple_len]).unwrap();
		state.commit().unwrap();
	}

	let committed_size = {
		let mut state = CompactProofState::try_from_bytes_mut(&mut data)
			.unwrap_or_else(|error| panic!("valid compact state rejected: {error:?}"));
		state
			.set_bytes(&replacement_bytes[..replacement_len])
			.unwrap();
		let projected_size = state.projected_size();
		let committed_size = state.commit().unwrap();
		assert_eq!(committed_size, projected_size);

		committed_size
	};

	let state = CompactProofState::try_from_bytes(&data[..committed_size])
		.unwrap_or_else(|error| panic!("changed compact prefix rejected: {error:?}"));
	assert_eq!(state.bytes(), &replacement_bytes[..replacement_len]);
	assert_eq!(state.words(), &words[..word_len]);
	assert_eq!(state.triples(), &triples[..triple_len]);

	let words_start = CompactProofState::HEADER_SIZE + replacement_len;
	let triples_start = words_start + word_len * size_of::<PodU16>();
	assert_eq!(
		triples_start + triple_len * size_of::<[u8; 3]>(),
		committed_size
	);
}

#[kani::proof]
#[kani::unwind(32)]
fn compact_every_two_step_tail_ordering_preserves_all_logical_values() {
	let byte_values = [[1u8, 2], [3, 4], [5, 6]];
	let word_values = [
		[PodU16::from(10), PodU16::from(11)],
		[PodU16::from(12), PodU16::from(13)],
		[PodU16::from(14), PodU16::from(15)],
	];
	let triple_values = [
		[[20u8, 21, 22], [23, 24, 25]],
		[[26, 27, 28], [29, 30, 31]],
		[[32, 33, 34], [35, 36, 37]],
	];
	let initial_lengths: [usize; 3] = kani::any();
	let replacement_lengths: [usize; 2] = kani::any();
	let operations: [u8; 2] = kani::any();
	for length in initial_lengths {
		kani::assume(length <= 1);
	}
	for length in replacement_lengths {
		kani::assume(length <= 1);
	}
	for operation in operations {
		kani::assume(operation < 3);
	}

	let mut data = [0u8; CompactProofState::MAX_SIZE];
	{
		let mut state = CompactProofState::initialize(&mut data).unwrap();
		state
			.set_bytes(&byte_values[0][..initial_lengths[0]])
			.unwrap();
		state
			.set_words(&word_values[0][..initial_lengths[1]])
			.unwrap();
		state
			.set_triples(&triple_values[0][..initial_lengths[2]])
			.unwrap();
		state.commit().unwrap();
	}

	let mut generations = [0usize; 3];
	let mut lengths = initial_lengths;
	for step in 0..2 {
		let generation = step + 1;
		let operation = usize::from(operations[step]);
		let replacement_length = replacement_lengths[step];
		let committed_size = {
			let mut state = CompactProofState::try_from_bytes_mut(&mut data).unwrap();
			match operation {
				0 => {
					state
						.set_bytes(&byte_values[generation][..replacement_length])
						.unwrap()
				}
				1 => {
					state
						.set_words(&word_values[generation][..replacement_length])
						.unwrap()
				}
				2 => {
					state
						.set_triples(&triple_values[generation][..replacement_length])
						.unwrap()
				}
				_ => unreachable!(),
			}
			let projected_size = state.projected_size();
			let committed_size = state.commit().unwrap();
			assert_eq!(committed_size, projected_size);
			committed_size
		};

		generations[operation] = generation;
		lengths[operation] = replacement_length;
		let state = CompactProofState::try_from_bytes(&data[..committed_size]).unwrap();
		assert_eq!(state.bytes(), &byte_values[generations[0]][..lengths[0]]);
		assert_eq!(state.words(), &word_values[generations[1]][..lengths[1]]);
		assert_eq!(state.triples().len(), lengths[2]);
		for index in 0..2 {
			if index < lengths[2] {
				let actual = state.triples()[index];
				let expected = triple_values[generations[2]][index];
				assert!(
					actual[0] == expected[0]
						&& actual[1] == expected[1]
						&& actual[2] == expected[2]
				);
			}
		}
	}
}

#[kani::proof]
#[kani::unwind(32)]
fn compact_repeated_grow_and_shrink_operations_preserve_logical_values() {
	let first: [u8; 2] = kani::any();
	let second: [u8; 2] = kani::any();
	let third: [u8; 2] = kani::any();
	let first_len: usize = kani::any();
	let second_len: usize = kani::any();
	let third_len: usize = kani::any();
	kani::assume(first_len <= first.len());
	kani::assume(second_len <= second.len());
	kani::assume(third_len <= third.len());
	let mut data = [0u8; CompactProofState::MAX_SIZE];

	{
		let mut state = CompactProofState::initialize(&mut data).unwrap();
		state.set_bytes(&first[..first_len]).unwrap();
		state.commit().unwrap();
	}

	for (values, len) in [(&second, second_len), (&third, third_len)] {
		let committed_size = {
			let mut state = CompactProofState::try_from_bytes_mut(&mut data).unwrap();
			state.set_bytes(&values[..len]).unwrap();
			state.commit().unwrap()
		};
		let state = CompactProofState::try_from_bytes(&data[..committed_size]).unwrap();
		assert_eq!(state.bytes(), &values[..len]);
		assert!(state.words().is_empty());
		assert!(state.triples().is_empty());
	}
}

#[kani::proof]
#[kani::unwind(8)]
fn compact_arbitrary_bytes_are_rejected_or_form_a_bounded_layout() {
	let data: [u8; CompactProofState::MAX_SIZE + 1] = kani::any();
	let len: usize = kani::any();
	kani::assume(len <= data.len());
	let result = CompactProofState::try_from_bytes(&data[..len]);
	let expected = if len < CompactProofState::HEADER_SIZE || len > CompactProofState::MAX_SIZE {
		false
	} else {
		let byte_len = decode_length(&data, BYTES_LENGTH_OFFSET);
		let word_len = decode_length(&data, WORDS_LENGTH_OFFSET);
		let triple_len = decode_length(&data, TRIPLES_LENGTH_OFFSET);
		let active_size = CompactProofState::HEADER_SIZE
			+ byte_len
			+ word_len * size_of::<PodU16>()
			+ triple_len * size_of::<[u8; 3]>();

		data[0] == CompactProofKind::Layout as u8
			&& byte_len <= 2
			&& word_len <= 2
			&& triple_len <= 2
			&& active_size <= len
	};

	assert_eq!(result.is_ok(), expected);

	if let Ok(state) = result {
		assert!(state.bytes().len() <= 2);
		assert!(state.words().len() <= 2);
		assert!(state.triples().len() <= 2);
		let active_size = CompactProofState::HEADER_SIZE
			+ state.bytes().len()
			+ state.words().len() * size_of::<PodU16>()
			+ state.triples().len() * size_of::<[u8; 3]>();
		assert!(active_size <= len);
	}
}
