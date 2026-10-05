//! Minimal decoder for the Solana transaction wire format.
//!
//! `pina rehearse` replays each transaction byte for byte, so it only needs the
//! parts of a message that name instructions and accounts: the static account
//! keys, each instruction's program and data, and the address lookup tables a
//! v0 message loads further accounts from. Legacy and v0 messages are
//! supported; any other version is reported as undecodable.

/// The parts of a signed transaction a rehearsal reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WireTransaction {
	/// Static account keys in message order.
	pub(crate) account_keys: Vec<[u8; 32]>,
	/// Address lookup tables referenced by a v0 message.
	pub(crate) lookup_tables: Vec<[u8; 32]>,
	/// Top-level instructions in execution order.
	pub(crate) instructions: Vec<WireInstruction>,
}

/// One top-level instruction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WireInstruction {
	/// Index of the invoked program in the static account keys.
	pub(crate) program_id_index: u8,
	/// Raw instruction data.
	pub(crate) data: Vec<u8>,
}

impl WireTransaction {
	/// Decode a serialized `VersionedTransaction`.
	pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
		let mut reader = Reader { bytes, offset: 0 };
		let signatures = reader.compact_len()?;
		reader.take(signatures * 64)?;

		let versioned = reader.peek()? & 0x80 != 0;

		if versioned {
			let version = reader.byte()? & 0x7f;

			if version != 0 {
				return Err(format!("message version {version} is not supported"));
			}
		}

		reader.take(3)?;
		let key_count = reader.compact_len()?;
		let account_keys = (0..key_count)
			.map(|_| reader.key())
			.collect::<Result<Vec<_>, _>>()?;
		reader.take(32)?;

		let instruction_count = reader.compact_len()?;
		let instructions = (0..instruction_count)
			.map(|_| reader.instruction())
			.collect::<Result<Vec<_>, _>>()?;

		let mut lookup_tables = Vec::new();

		if versioned {
			for _ in 0..reader.compact_len()? {
				lookup_tables.push(reader.key()?);
				let writable = reader.compact_len()?;
				reader.take(writable)?;
				let readonly = reader.compact_len()?;
				reader.take(readonly)?;
			}
		}

		Ok(Self {
			account_keys,
			lookup_tables,
			instructions,
		})
	}

	/// Return the program an instruction invokes.
	pub(crate) fn program_id(&self, instruction: &WireInstruction) -> Option<&[u8; 32]> {
		self.account_keys
			.get(usize::from(instruction.program_id_index))
	}
}

struct Reader<'a> {
	bytes: &'a [u8],
	offset: usize,
}

impl<'a> Reader<'a> {
	fn take(&mut self, length: usize) -> Result<&'a [u8], String> {
		let end = self
			.offset
			.checked_add(length)
			.filter(|end| *end <= self.bytes.len())
			.ok_or_else(|| {
				format!(
					"transaction ends before byte {}",
					self.offset.saturating_add(length)
				)
			})?;
		let slice = &self.bytes[self.offset..end];
		self.offset = end;

		Ok(slice)
	}

	fn peek(&self) -> Result<u8, String> {
		self.bytes
			.get(self.offset)
			.copied()
			.ok_or_else(|| "transaction has no message".to_owned())
	}

	fn byte(&mut self) -> Result<u8, String> {
		Ok(self.take(1)?[0])
	}

	fn key(&mut self) -> Result<[u8; 32], String> {
		let mut key = [0_u8; 32];
		key.copy_from_slice(self.take(32)?);

		Ok(key)
	}

	/// Read Solana's `compact-u16` length prefix (at most three bytes).
	fn compact_len(&mut self) -> Result<usize, String> {
		let mut value = 0_usize;

		for position in 0..3 {
			let byte = self.byte()?;
			value |= usize::from(byte & 0x7f) << (position * 7);

			if byte & 0x80 == 0 {
				return u16::try_from(value)
					.map(usize::from)
					.map_err(|_| format!("compact length {value} exceeds u16"));
			}
		}

		Err("compact length is longer than three bytes".to_owned())
	}

	fn instruction(&mut self) -> Result<WireInstruction, String> {
		let program_id_index = self.byte()?;
		let accounts = self.compact_len()?;
		self.take(accounts)?;
		let data_length = self.compact_len()?;
		let data = self.take(data_length)?.to_vec();

		Ok(WireInstruction {
			program_id_index,
			data,
		})
	}
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
