//! CU cost model and profile data structures.
//!
//! Solana's SBF runtime assigns deterministic CU costs to each instruction
//! class. This module defines the cost model and the profile output structs.
//!
//! ## Cost model
//!
//! The static profiler estimates compute unit (CU) costs by decoding each
//! 8-byte SBF instruction's opcode and assigning a cost based on the
//! instruction class:
//!
//! | Class | Opcodes | Cost |
//! |-------|---------|------|
//! | ALU (add, sub, mul, div, mod, xor, or, and, lsh, rsh, arsh, neg, le, be) | 0x04–0xDC | 1 CU |
//! | Memory load/store (ldxb, ldxh, ldxw, ldxdw, stb, sth, stw, stdw, stxb, stxh, stxw, stxdw) | 0x61–0x7B | 1 CU |
//! | Branch (ja, jeq, jgt, jge, jlt, jle, jne, jset, jsgt, jsge, jslt, jsle, call, exit) | 0x05–0x9D | 1 CU |
//! | Syscall (call imm with known syscall hash) | 0x85 | 100 CU |
//! | Load immediate (lddw — 16-byte wide load) | 0x18 | 1 CU |
//!
//! Syscalls are identified by opcode 0x85 (`BPF_CALL` with immediate operand).
//! The actual on-chain syscall cost varies (e.g. `sol_log` ~100 CU,
//! `sol_invoke_signed` ~thousands), but we use a flat 100 CU estimate since
//! static analysis cannot determine the exact syscall target without symbol
//! resolution of the immediate.
//!
//! ## Limitations
//!
//! - **Static analysis only** — does not account for runtime branching. The
//!   reported CU is the sum of all instructions, not the worst-case or
//!   average-case path.
//! - **Flat syscall cost** — all syscalls are estimated at 100 CU regardless
//!   of their actual on-chain cost.
//! - **No loop analysis** — loops are counted once; actual CU depends on
//!   iteration count at runtime.

use serde::Serialize;

/// CU cost per regular instruction (ALU, memory, branch).
pub const CU_PER_INSTRUCTION: u64 = 1;

/// Estimated CU cost per syscall invocation.
///
/// The actual cost varies by syscall (`sol_log` ~100, `sol_invoke_signed`
/// ~thousands), but 100 CU is a reasonable baseline for static estimation.
pub const CU_PER_SYSCALL: u64 = 100;

/// SBF opcode for `call imm` (`BPF_JMP` | `BPF_CALL`).
///
/// When the source register is 0, this is a syscall (call to an external
/// function resolved by the runtime). When the source register is non-zero,
/// it's an internal function call.
pub const BPF_CALL_IMM: u8 = 0x85;

/// Estimate the CU cost of a single SBF instruction from its 8-byte encoding.
///
/// Returns [`CU_PER_SYSCALL`] for syscall instructions (opcode `0x85` with
/// `src_reg == 0`), and [`CU_PER_INSTRUCTION`] for all other instructions.
#[must_use]
pub fn estimate_instruction_cu(instruction_bytes: &[u8; 8]) -> u64 {
	let opcode = instruction_bytes[0];
	// src_reg is the high nibble of byte 1
	let src_reg = instruction_bytes[1] >> 4;

	if opcode == BPF_CALL_IMM && src_reg == 0 {
		CU_PER_SYSCALL
	} else {
		CU_PER_INSTRUCTION
	}
}

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
	/// Number of syscall instructions detected.
	pub syscall_count: u64,
	/// Estimated CU cost (sum of per-instruction costs).
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
	/// Total syscall count across all functions.
	pub total_syscalls: u64,
	/// Total estimated CU across all functions.
	pub total_cu: u64,
	/// Per-function profiles, sorted by estimated CU (descending).
	pub functions: Vec<FunctionProfile>,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn regular_instruction_costs_one_cu() {
		// ADD64 imm: opcode 0x07
		let add = [0x07, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
		assert_eq!(estimate_instruction_cu(&add), CU_PER_INSTRUCTION);
	}

	#[test]
	fn syscall_costs_syscall_cu() {
		// CALL imm with src_reg=0: opcode 0x85, byte1 high nibble = 0
		let syscall = [0x85, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
		assert_eq!(estimate_instruction_cu(&syscall), CU_PER_SYSCALL);
	}

	#[test]
	fn internal_call_costs_one_cu() {
		// CALL imm with src_reg=1 (internal function call, not syscall)
		let internal_call = [0x85, 0x10, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
		assert_eq!(estimate_instruction_cu(&internal_call), CU_PER_INSTRUCTION);
	}

	#[test]
	fn exit_instruction_costs_one_cu() {
		// EXIT: opcode 0x95
		let exit = [0x95, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
		assert_eq!(estimate_instruction_cu(&exit), CU_PER_INSTRUCTION);
	}

	#[test]
	fn memory_load_costs_one_cu() {
		// LDXDW: opcode 0x79
		let load = [0x79, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
		assert_eq!(estimate_instruction_cu(&load), CU_PER_INSTRUCTION);
	}

	#[test]
	fn branch_costs_one_cu() {
		// JEQ imm: opcode 0x15
		let branch = [0x15, 0x00, 0x02, 0x00, 0x05, 0x00, 0x00, 0x00];
		assert_eq!(estimate_instruction_cu(&branch), CU_PER_INSTRUCTION);
	}
}
