//! Program-aware names and field decoding for rehearsal reports.
//!
//! The catalog is built from the project's IR: instruction discriminators name
//! each top-level instruction, and account discriminators plus the fixed
//! `PinaPod` layout map differing account bytes to field names. Decoding is a
//! presentation aid only; classification never depends on it, so anything the
//! catalog cannot name is still reported as raw byte ranges.

use pina_abi::DataSchema;
use pina_abi::FieldSchema;
use pina_abi::LayoutKind;
use pina_abi::MigrationVersionType;
use pina_abi::PhysicalLayout;

use super::report::ByteRange;
use super::report::FieldChange;
use crate::ir::ProgramIr;

/// The most differing byte ranges listed for one account.
const MAX_BYTE_RANGES: usize = 16;

/// Instruction and account names known to the project being rehearsed.
#[derive(Debug)]
pub(crate) struct ProgramCatalog {
	instructions: Vec<NamedInstruction>,
	accounts: Vec<AccountLayout>,
}

#[derive(Debug)]
struct NamedInstruction {
	name: String,
	discriminator: Discriminator,
}

#[derive(Clone, Copy, Debug)]
struct Discriminator {
	value: u64,
	width: usize,
}

impl Discriminator {
	fn matches(self, data: &[u8]) -> bool {
		let Some(prefix) = data.get(..self.width).filter(|_| self.width <= 8) else {
			return false;
		};
		let mut bytes = [0_u8; 8];
		bytes[..self.width].copy_from_slice(prefix);

		u64::from_le_bytes(bytes) == self.value
	}
}

/// Byte layout of one account type, including its framework header.
#[derive(Debug)]
pub(crate) struct AccountLayout {
	name: String,
	discriminator: Discriminator,
	fields: Vec<FieldSlot>,
}

#[derive(Debug)]
struct FieldSlot {
	name: String,
	offset: usize,
	size: usize,
	kind: FieldKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FieldKind {
	Unsigned,
	Signed,
	Bool,
	Address,
	Float,
	Bytes,
}

/// Decoded field changes and the remaining differing byte ranges.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct DataComparison {
	pub(crate) fields: Vec<FieldChange>,
	pub(crate) ranges: Vec<ByteRange>,
	pub(crate) omitted_ranges: usize,
}

impl ProgramCatalog {
	/// Build the catalog from parsed IR.
	///
	/// `version_type` is the checked-in manifest's version-envelope width.
	/// Without it, a migration-aware account's payload offset is unknown, so
	/// only its discriminator is decoded.
	pub(crate) fn new(ir: &ProgramIr, version_type: Option<MigrationVersionType>) -> Self {
		let instructions = ir
			.instructions
			.iter()
			.map(|instruction| {
				NamedInstruction {
					name: instruction.name.clone(),
					discriminator: Discriminator {
						value: instruction.discriminator.value,
						width: instruction.discriminator.repr_size,
					},
				}
			})
			.collect();
		let accounts = ir
			.accounts
			.iter()
			.map(|account| {
				let discriminator = Discriminator {
					value: account.discriminator.value,
					width: account.discriminator.repr_size,
				};
				let mut fields = vec![FieldSlot {
					name: "(discriminator)".to_owned(),
					offset: 0,
					size: discriminator.width,
					kind: FieldKind::Unsigned,
				}];
				let version_bytes = if account.is_migratable() {
					version_type.map(MigrationVersionType::bytes)
				} else {
					Some(0)
				};

				if let Some(version_bytes) = version_bytes {
					if version_bytes > 0 {
						fields.push(FieldSlot {
							name: "(migration version)".to_owned(),
							offset: discriminator.width,
							size: version_bytes,
							kind: FieldKind::Unsigned,
						});
					}

					let header = discriminator.width + version_bytes;
					let schema = DataSchema {
						layout: if account.is_compact() {
							LayoutKind::Compact
						} else {
							LayoutKind::Fixed
						},
						fields: account
							.fields
							.iter()
							.map(|field| {
								FieldSchema {
									name: field.name.clone(),
									rust_type: field.rust_type.clone(),
								}
							})
							.collect(),
					};
					fields.extend(payload_slots(&schema, &account.fields, header));
				}

				AccountLayout {
					name: account.name.clone(),
					discriminator,
					fields,
				}
			})
			.collect();

		Self {
			instructions,
			accounts,
		}
	}

	/// Name an instruction of this program from its data.
	pub(crate) fn instruction_name(&self, data: &[u8]) -> String {
		if let Some(instruction) = self
			.instructions
			.iter()
			.find(|instruction| instruction.discriminator.matches(data))
		{
			return instruction.name.clone();
		}

		data.first().map_or_else(
			|| "unknown (no data)".to_owned(),
			|byte| format!("unknown (0x{byte:02x})"),
		)
	}

	/// Find the account type whose discriminator prefixes `data`.
	pub(crate) fn account_layout(&self, data: &[u8]) -> Option<&AccountLayout> {
		self.accounts
			.iter()
			.find(|account| account.discriminator.matches(data))
	}
}

impl AccountLayout {
	/// The account type's name.
	pub(crate) fn name(&self) -> &str {
		&self.name
	}

	/// Split the differing bytes of two images into decoded fields and the
	/// byte ranges no field covers.
	pub(crate) fn compare(&self, baseline: &[u8], candidate: &[u8]) -> DataComparison {
		let mut differs = differing_bytes(baseline, candidate);
		let mut fields = Vec::new();

		for slot in &self.fields {
			let range = slot.offset..slot.offset + slot.size;
			let (Some(before), Some(after)) =
				(baseline.get(range.clone()), candidate.get(range.clone()))
			else {
				continue;
			};

			if before == after {
				continue;
			}

			fields.push(FieldChange {
				name: slot.name.clone(),
				baseline: decode(slot.kind, before),
				candidate: decode(slot.kind, after),
			});
			differs[range].fill(false);
		}

		let (ranges, omitted_ranges) = byte_ranges(&differs);

		DataComparison {
			fields,
			ranges,
			omitted_ranges,
		}
	}
}

/// Compare two images of an account no catalog entry describes.
pub(crate) fn compare_raw(baseline: &[u8], candidate: &[u8]) -> DataComparison {
	let (ranges, omitted_ranges) = byte_ranges(&differing_bytes(baseline, candidate));

	DataComparison {
		fields: Vec::new(),
		ranges,
		omitted_ranges,
	}
}

/// Payload slots for a fixed layout. Compact layouts and types outside the
/// closed grammar have no fixed offsets, so they decode as byte ranges.
fn payload_slots(
	schema: &DataSchema,
	fields: &[crate::ir::FieldIr],
	header: usize,
) -> Vec<FieldSlot> {
	let Ok(PhysicalLayout::Fixed {
		fields: physical, ..
	}) = schema.physical()
	else {
		return Vec::new();
	};

	physical
		.iter()
		.zip(fields)
		.map(|(layout, field)| {
			FieldSlot {
				name: layout.name.clone(),
				offset: header + layout.offset as usize,
				size: layout.size as usize,
				kind: field_kind(&field.rust_type),
			}
		})
		.collect()
}

fn field_kind(rust_type: &str) -> FieldKind {
	match rust_type.trim() {
		"u8" | "u16" | "u32" | "u64" | "u128" | "PodU16" | "PodU32" | "PodU64" | "PodU128" => {
			FieldKind::Unsigned
		}
		"i8" | "i16" | "i32" | "i64" | "i128" | "PodI16" | "PodI32" | "PodI64" | "PodI128" => {
			FieldKind::Signed
		}
		"bool" | "PodBool" => FieldKind::Bool,
		"Address" => FieldKind::Address,
		"f32" | "f64" => FieldKind::Float,
		_ => FieldKind::Bytes,
	}
}

/// Render one field value. Integers are little-endian, matching `PinaPod`.
fn decode(kind: FieldKind, bytes: &[u8]) -> String {
	match (kind, bytes.len()) {
		(FieldKind::Unsigned, 1..=16) => {
			let mut buffer = [0_u8; 16];
			buffer[..bytes.len()].copy_from_slice(bytes);
			u128::from_le_bytes(buffer).to_string()
		}
		(FieldKind::Signed, 1..=16) => {
			let fill = if bytes[bytes.len() - 1] & 0x80 == 0 {
				0
			} else {
				0xff
			};
			let mut buffer = [fill; 16];
			buffer[..bytes.len()].copy_from_slice(bytes);
			i128::from_le_bytes(buffer).to_string()
		}
		(FieldKind::Bool, 1) if bytes[0] <= 1 => (bytes[0] == 1).to_string(),
		(FieldKind::Address, 32) => bs58::encode(bytes).into_string(),
		(FieldKind::Float, 4) => {
			let mut buffer = [0_u8; 4];
			buffer.copy_from_slice(bytes);
			f32::from_le_bytes(buffer).to_string()
		}
		(FieldKind::Float, 8) => {
			let mut buffer = [0_u8; 8];
			buffer.copy_from_slice(bytes);
			f64::from_le_bytes(buffer).to_string()
		}
		_ => format!("0x{}", super::hex(bytes)),
	}
}

fn differing_bytes(baseline: &[u8], candidate: &[u8]) -> Vec<bool> {
	(0..baseline.len().max(candidate.len()))
		.map(|index| baseline.get(index) != candidate.get(index))
		.collect()
}

/// Coalesce differing bytes into half-open ranges, keeping the first
/// [`MAX_BYTE_RANGES`] and counting the rest.
fn byte_ranges(differs: &[bool]) -> (Vec<ByteRange>, usize) {
	let mut ranges = Vec::new();
	let mut start = None;

	for (index, differ) in differs.iter().copied().chain([false]).enumerate() {
		match (start, differ) {
			(None, true) => start = Some(index),
			(Some(first), false) => {
				ranges.push(ByteRange {
					start: first,
					end: index,
				});
				start = None;
			}
			_ => {}
		}
	}

	let omitted = ranges.len().saturating_sub(MAX_BYTE_RANGES);
	ranges.truncate(MAX_BYTE_RANGES);

	(ranges, omitted)
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
