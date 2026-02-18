//! Intermediate representation for a parsed Pina program.
//!
//! The IR is constructed from syn-parsed source files and later lowered into
//! `codama-nodes` types for JSON output.

/// Top-level IR for a single program crate.
#[derive(Debug, Clone)]
pub struct ProgramIr {
	pub name: String,
	pub public_key: String,
	pub accounts: Vec<AccountIr>,
	pub instructions: Vec<InstructionIr>,
	pub errors: Vec<ErrorIr>,
	pub pdas: Vec<PdaIr>,
}

/// An on-chain account type decorated with `#[account]`.
#[derive(Debug, Clone)]
pub struct AccountIr {
	pub name: String,
	pub fields: Vec<FieldIr>,
	pub discriminator: DiscriminatorIr,
	pub docs: Vec<String>,
}

/// An instruction assembled from `#[instruction]`, `#[derive(Accounts)]`, the
/// entrypoint dispatch map, and validation chain analysis.
#[derive(Debug, Clone)]
pub struct InstructionIr {
	pub name: String,
	pub accounts: Vec<InstructionAccountIr>,
	pub arguments: Vec<FieldIr>,
	pub discriminator: DiscriminatorIr,
	pub docs: Vec<String>,
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
	pub is_pda: bool,
	pub pda_name: Option<String>,
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
