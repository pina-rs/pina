//! CU cost model and profile data structures.
//!
//! Solana's SBF runtime assigns deterministic CU costs to each instruction
//! class. This module defines the cost model and the profile output structs.

use serde::Serialize;

/// CU cost per regular instruction (ALU, memory, branch).
///
/// The SBF VM charges 1 CU per instruction by default.
/// Some syscalls have higher costs, but for static analysis of the
/// instruction stream this is the baseline.
pub const CU_PER_INSTRUCTION: u64 = 1;

/// Per-function CU profile.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FunctionProfile {
	/// Function name (symbol name or `<unknown+offset>`).
	pub name: String,
	/// Byte offset of the function within the `.text` section.
	pub offset: u64,
	/// Size of the function in bytes.
	pub size: u64,
	/// Number of SBF instructions in the function.
	pub instruction_count: u64,
	/// Estimated CU cost (`instruction_count * CU_PER_INSTRUCTION`).
	pub estimated_cu: u64,
}

/// Complete profile for a program binary.
#[derive(Debug, Clone, Serialize)]
pub struct ProgramProfile {
	/// Program name (derived from the ELF filename).
	pub program_name: String,
	/// Total binary size in bytes.
	pub binary_size: u64,
	/// Size of the `.text` section(s) in bytes.
	pub text_size: u64,
	/// Total SBF instruction count across all functions.
	pub total_instructions: u64,
	/// Total estimated CU across all functions.
	pub total_cu: u64,
	/// Per-function profiles, sorted by estimated CU (descending).
	pub functions: Vec<FunctionProfile>,
}
