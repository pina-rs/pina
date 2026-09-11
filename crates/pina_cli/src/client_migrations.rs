//! Migration-envelope facts extracted from a Codama IDL.
//!
//! The generated-client hardeners turn these facts into per-account
//! `needsMigration` checks and the program-level reserved `Migrate`
//! instruction, so a released client can bring stale accounts current
//! without hand-composing account metas.

use codama_nodes::NestedTypeNodeTrait;
use codama_nodes::Number;
use codama_nodes::NumberFormat;
use codama_nodes::ProgramNode;
use codama_nodes::TypeNode;
use codama_nodes::ValueNode;

/// One declared migratable account, in program declaration order.
///
/// The order is the reserved instruction's wire order: slot `2 + position`.
#[derive(Clone, Debug)]
pub(crate) struct MigratableAccount {
	/// camelCase name used by the TypeScript account module.
	pub(crate) camel: String,
	/// `SHOUTING_SNAKE` name used for generated constants.
	pub(crate) shouting: String,
	/// Discriminator bytes, little-endian, as stored at offset 0.
	pub(crate) discriminator: Vec<u8>,
	/// Current schema version this IDL was generated from.
	pub(crate) version: u64,
	/// Width of the version envelope field.
	pub(crate) version_bytes: usize,
}

impl MigratableAccount {
	/// Offset of the version envelope: directly after the discriminator.
	pub(crate) fn version_offset(&self) -> usize {
		self.discriminator.len()
	}
}

/// Everything the hardeners need to emit migration helpers for one program.
#[derive(Clone, Debug)]
pub(crate) struct MigrationPlan {
	/// Declared migratable accounts in reserved-instruction slot order.
	pub(crate) accounts: Vec<MigratableAccount>,
	/// The reserved all-ones discriminator at the program's instruction width.
	pub(crate) reserved_discriminator: Vec<u8>,
}

impl MigrationPlan {
	/// Read the migration envelope from a parsed IDL program.
	///
	/// Returns `None` for programs that declare no migratable accounts: they
	/// get no migration helpers.
	pub(crate) fn read(program: &ProgramNode) -> Result<Option<Self>, String> {
		let mut accounts = Vec::new();
		for account in &program.accounts {
			let fields = &account.data.get_nested_type_node().fields;
			// The envelope is the account's first two fields: the
			// discriminator, then the migration version carrying a numeric
			// default. Anything else is not a migratable account.
			let mut envelope = fields.iter().take(2).filter_map(|field| {
				let default_value = field.default_value.as_ref().as_ref()?;
				let kind = match field.name.as_ref() {
					"discriminator" => "discriminator",
					"migrationVersion" => "migrationVersion",
					_ => return None,
				};
				Some((kind, field.r#type.as_ref(), default_value))
			});
			let facts = |field_type: &TypeNode,
			             default_value: &ValueNode|
			 -> Result<NumberFacts, String> {
				let ValueNode::Number(number_value) = default_value else {
					return Err("migration envelope fields must carry numeric defaults".to_owned());
				};
				let TypeNode::Number(number_type) = field_type else {
					return Err("migration envelope fields must be numeric".to_owned());
				};
				let width = number_width(number_type.format)?;
				let value = match number_value.number {
					Number::UnsignedInteger(value) => Ok(value),
					Number::SignedInteger(value) if value >= 0 => {
						u64::try_from(value)
							.map_err(|_| "migration envelope values must fit u64".to_owned())
					}
					_ => Err("migration envelope values must be non-negative integers".to_owned()),
				}?;
				let mut bytes = value.to_le_bytes().to_vec();
				bytes.truncate(width);

				Ok(NumberFacts {
					bytes,
					value,
					width,
				})
			};

			let discriminator = match envelope.next() {
				Some(("discriminator", field_type, default_value)) => {
					facts(field_type, default_value)?
				}
				_ => continue,
			};
			let version = match envelope.next() {
				Some(("migrationVersion", field_type, default_value)) => {
					facts(field_type, default_value)?
				}
				_ => continue,
			};

			accounts.push(MigratableAccount {
				camel: account.name.as_ref().to_owned(),
				shouting: shouting_snake(account.name.as_ref()),
				discriminator: discriminator.bytes,
				version: version.value,
				version_bytes: version.width,
			});
		}

		if accounts.is_empty() {
			return Ok(None);
		}

		let reserved_discriminator = reserved_discriminator(program);

		Ok(Some(Self {
			accounts,
			reserved_discriminator,
		}))
	}
}

/// Numeric facts about one envelope field: LE bytes, value, and width.
struct NumberFacts {
	bytes: Vec<u8>,
	value: u64,
	width: usize,
}

fn number_width(format: NumberFormat) -> Result<usize, String> {
	match format {
		NumberFormat::U8 => Ok(1),
		NumberFormat::U16 => Ok(2),
		NumberFormat::U32 => Ok(4),
		NumberFormat::U64 => Ok(8),
		other => {
			Err(format!(
				"migration envelope fields must be unsigned integers, found `{other:?}`"
			))
		}
	}
}

/// Build the reserved all-ones discriminator at the program's instruction
/// discriminator width, defaulting to one byte when no instruction declares a
/// constant discriminator.
fn reserved_discriminator(program: &ProgramNode) -> Vec<u8> {
	let width = program
		.instructions
		.iter()
		.find_map(|instruction| {
			instruction.discriminators.iter().find_map(|discriminator| {
				let codama_nodes::DiscriminatorNode::Constant(constant) = discriminator else {
					return None;
				};
				let TypeNode::Number(number_type) = constant.constant.r#type.as_ref() else {
					return None;
				};
				number_width(number_type.format).ok()
			})
		})
		.unwrap_or(1);

	vec![u8::MAX; width]
}

/// `manualState` becomes `MANUAL_STATE`.
pub(crate) fn shouting_snake(camel: &str) -> String {
	let mut out = String::with_capacity(camel.len() + 4);
	for (index, character) in camel.char_indices() {
		if character.is_ascii_uppercase() && index > 0 {
			out.push('_');
		}
		out.extend(character.to_uppercase());
	}
	out
}

/// `manualState` becomes `ManualState`.
pub(crate) fn pascal_case(camel: &str) -> String {
	let mut characters = camel.chars();
	match characters.next() {
		Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
		None => String::new(),
	}
}
