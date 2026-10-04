//! Intermediate representation for a parsed Pina program.
//!
//! The IR is constructed from syn-parsed source files and later lowered into
//! `codama-nodes` types for JSON output.

/// Top-level IR for a single program crate.
#[derive(Debug, Clone)]
pub struct ProgramIr {
	pub name: String,
	pub public_key: String,
	pub pinapod_enums: Vec<PinaPodEnumIr>,
	pub accounts: Vec<AccountIr>,
	pub instructions: Vec<InstructionIr>,
	pub events: Vec<EventIr>,
	pub errors: Vec<ErrorIr>,
	pub pdas: Vec<PdaIr>,
}

/// An `#[event]` struct with its resolved discriminator.
#[derive(Debug, Clone)]
pub struct EventIr {
	pub name: String,
	pub discriminator: DiscriminatorIr,
	pub fields: Vec<FieldIr>,
	pub docs: Vec<String>,
}

impl EventIr {
	pub(crate) fn visible_docs(&self) -> Vec<String> {
		self.docs
			.iter()
			.filter(|doc| doc.as_str() != MIGRATABLE_DOC_MARKER)
			.cloned()
			.collect()
	}

	pub(crate) fn is_migratable(&self) -> bool {
		self.docs.iter().any(|doc| doc == MIGRATABLE_DOC_MARKER)
	}
}

/// A local unit enum derived with `PinaPod` and its generated companion.
#[derive(Debug, Clone)]
pub struct PinaPodEnumIr {
	pub name: String,
	pub repr_size: usize,
	pub variants: Vec<PinaPodEnumVariantIr>,
	pub docs: Vec<String>,
}

/// A unit variant and its explicit wire discriminant.
#[derive(Debug, Clone)]
pub struct PinaPodEnumVariantIr {
	pub name: String,
	pub value: u32,
}

/// An on-chain account type decorated with `#[account]`.
#[derive(Debug, Clone)]
pub struct AccountIr {
	pub name: String,
	pub fields: Vec<FieldIr>,
	pub discriminator: DiscriminatorIr,
	pub docs: Vec<String>,
	/// The name of the PDA declared for this account via `#[pda(...)]`.
	pub pda_name: Option<String>,
}

// Keep compactness in the pre-existing docs field so this minor release does
// not add a field to the public IR structs. Codegen always strips the sentinel.
pub(crate) const COMPACT_ACCOUNT_DOC_MARKER: &str = "\0pina:compact";
// Migration opt-in follows the same compatibility strategy. The checked-in
// manifest supplies the current version and global width during IDL lowering.
pub(crate) const MIGRATABLE_DOC_MARKER: &str = "\0pina:migratable";
// An instruction an auto policy records without a version envelope: the
// manifest snapshots it to gate wire-breaking changes, but its bytes carry no
// version.
pub(crate) const RECORDED_DOC_MARKER: &str = "\0pina:recorded";

impl AccountIr {
	pub(crate) fn is_compact(&self) -> bool {
		self.docs
			.iter()
			.any(|doc| doc == COMPACT_ACCOUNT_DOC_MARKER)
	}

	pub(crate) fn visible_docs(&self) -> Vec<String> {
		self.docs
			.iter()
			.filter(|doc| {
				!matches!(
					doc.as_str(),
					COMPACT_ACCOUNT_DOC_MARKER | MIGRATABLE_DOC_MARKER
				)
			})
			.cloned()
			.collect()
	}

	pub(crate) fn is_migratable(&self) -> bool {
		self.docs.iter().any(|doc| doc == MIGRATABLE_DOC_MARKER)
	}
}

/// An instruction assembled from `#[instruction]`, `#[derive(Accounts)]`, the
/// entrypoint dispatch map, and validation chain analysis.
#[derive(Debug, Clone)]
pub struct InstructionIr {
	pub name: String,
	/// Rust struct ident behind the instruction. The IDL `name` is the
	/// snake-cased discriminator variant, but the migration manifest keys
	/// source lookups by the struct ident the macro expands.
	pub rust_name: String,
	pub accounts: Vec<InstructionAccountIr>,
	pub arguments: Vec<FieldIr>,
	pub discriminator: DiscriminatorIr,
	pub docs: Vec<String>,
}

impl InstructionIr {
	pub(crate) fn visible_docs(&self) -> Vec<String> {
		self.docs
			.iter()
			.filter(|doc| !matches!(doc.as_str(), MIGRATABLE_DOC_MARKER | RECORDED_DOC_MARKER))
			.cloned()
			.collect()
	}

	/// Whether the instruction carries the version envelope: only an explicit
	/// `migrations` token opts an instruction into migrations.
	pub(crate) fn is_migratable(&self) -> bool {
		self.docs.iter().any(|doc| doc == MIGRATABLE_DOC_MARKER)
	}

	/// Whether the migration manifest records the instruction, with or without
	/// an envelope.
	pub(crate) fn is_recorded(&self) -> bool {
		self.docs
			.iter()
			.any(|doc| matches!(doc.as_str(), MIGRATABLE_DOC_MARKER | RECORDED_DOC_MARKER))
	}
}

/// A single account slot inside an instruction.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct InstructionAccountIr {
	pub name: String,
	pub is_writable: bool,
	pub is_signer: bool,
	pub is_optional: bool,
	pub default_value: Option<DefaultValueIr>,
	/// The processor pins this slot's address to its PDA: it derives or checks
	/// the address from the PDA's seeds, or loads a PDA account type whose
	/// seeds are all constants. Generated clients derive default addresses only
	/// for pinned slots.
	pub is_pda: bool,
	/// The PDA this slot's account belongs to. Always set when `is_pda` is;
	/// also set without `is_pda` when the processor loads a variable-seed PDA
	/// account type without checking which seeds derived the address.
	pub pda_name: Option<String>,
	/// Canonical declarative account constraints used by compatibility checks.
	pub constraints: Vec<String>,
	pub docs: Vec<String>,
}

/// A typed field (used for both account state fields and instruction
/// arguments).
#[derive(Debug, Clone)]
pub struct FieldIr {
	pub name: String,
	pub rust_type: String,
	pub docs: Vec<String>,
}

/// A discriminator value and its byte width.
#[derive(Debug, Clone)]
pub struct DiscriminatorIr {
	pub value: u64,
	pub repr_size: usize,
}

/// A program error variant from `#[error]`.
#[derive(Debug, Clone)]
pub struct ErrorIr {
	pub name: String,
	pub code: u32,
	pub docs: Vec<String>,
}

/// A PDA derivation.
#[derive(Debug, Clone)]
pub struct PdaIr {
	pub name: String,
	pub seeds: Vec<PdaSeedIr>,
}

/// A single PDA seed — either a compile-time constant or a runtime variable.
#[derive(Debug, Clone)]
pub enum PdaSeedIr {
	Constant { value: Vec<u8> },
	Variable { name: String, rust_type: String },
}

/// A default value for an instruction account (e.g. a well-known program
/// address).
#[derive(Debug, Clone)]
pub enum DefaultValueIr {
	ProgramId(String),
	PublicKey(String),
}
