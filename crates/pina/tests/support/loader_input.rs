//! Loader-format program input for tests that run a declared `entrypoint`.

use core::mem::size_of;
use std::vec::Vec;

use pina::Address;
use pinocchio::account::MAX_PERMITTED_DATA_INCREASE;
use pinocchio::entrypoint::NON_DUP_MARKER;

/// One account slot in a serialized input.
#[derive(Clone, Copy)]
pub enum Slot {
	/// A writable account whose address starts with this byte, owned by
	/// `owner`.
	Account { address: u8, owner: Address },
	/// A duplicate marker naming an earlier slot.
	Duplicate(u8),
}

/// Serializes the input the SVM loader passes to `entrypoint`. The words keep
/// every record 8-byte aligned, as the loader does.
pub fn serialize(slots: &[Slot], data: &[u8], program_id: &Address) -> Vec<u64> {
	let mut bytes = (slots.len() as u64).to_le_bytes().to_vec();

	for slot in slots {
		match *slot {
			Slot::Account { address, owner } => {
				let mut header = [0; 88];
				header[0] = NON_DUP_MARKER;
				header[2] = 1;
				header[8] = address;
				header[40..72].copy_from_slice(owner.as_ref());
				bytes.extend(header);
				// Empty data, the realloc region, alignment, and the rent epoch.
				bytes.resize(
					bytes.len()
						+ MAX_PERMITTED_DATA_INCREASE.next_multiple_of(8)
						+ size_of::<u64>(),
					0,
				);
			}
			Slot::Duplicate(index) => bytes.extend([index, 0, 0, 0, 0, 0, 0, 0]),
		}
	}

	bytes.extend((data.len() as u64).to_le_bytes());
	bytes.extend(data);
	bytes.extend(program_id.as_ref());

	bytes
		.chunks(size_of::<u64>())
		.map(|chunk| {
			let mut word = [0; 8];
			word[..chunk.len()].copy_from_slice(chunk);
			u64::from_ne_bytes(word)
		})
		.collect()
}
