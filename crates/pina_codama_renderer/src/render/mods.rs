use codama_nodes::ProgramNode;

use super::helpers::canonical_pubkey;
use super::helpers::program_id_const_name;
use crate::error::Result;

pub(crate) fn render_root_mod(
	program: &ProgramNode,
	has_public_types: bool,
	has_compute_budget: bool,
) -> String {
	let mut lines = Vec::new();

	if !program.accounts.is_empty() {
		lines.push("pub mod accounts;".to_string());
	}
	if has_compute_budget {
		lines.push("pub mod compute_budget;".to_string());
	}
	if !program.errors.is_empty() {
		lines.push("pub mod errors;".to_string());
	}
	if !program.events.is_empty() {
		lines.push("pub mod events;".to_string());
	}
	if !program.instructions.is_empty() {
		lines.push("pub mod instructions;".to_string());
	}
	lines.push("pub mod programs;".to_string());
	if has_public_types {
		lines.push("pub mod types;".to_string());
	}
	lines.push(String::new());
	if has_compute_budget {
		lines.push("pub use compute_budget::set_compute_unit_limit_instruction;".to_string());
	}
	lines.push("#[allow(unused_imports)]".to_string());
	lines.push("pub(crate) use programs::*;".to_string());

	lines.join("\n")
}

/// The compute budget helper for clients whose instructions carry recorded
/// compute unit limits.
///
/// The program id is spelled out rather than imported so generated clients
/// need no dependency beyond the ones every client already has.
pub(crate) fn render_compute_budget_mod() -> String {
	r#"use solana_pubkey::Pubkey;
use solana_pubkey::pubkey;

/// The Compute Budget program, which sets a transaction's compute unit limit.
pub const COMPUTE_BUDGET_PROGRAM_ID: Pubkey =
	pubkey!("ComputeBudget111111111111111111111111111111");

/// `SetComputeUnitLimit`'s instruction tag in the Compute Budget program.
const SET_COMPUTE_UNIT_LIMIT_TAG: u8 = 2;

/// Build the Compute Budget program's `SetComputeUnitLimit(units)` instruction.
///
/// Add it to a transaction carrying this program's instructions with the sum
/// of their `*_COMPUTE_UNIT_LIMIT` constants. The transaction then requests,
/// and pays priority fees for, the compute units its recorded measurements
/// justify instead of the runtime's default allowance. The sum is
/// conservative: every constant carries its own margin and the compute budget
/// instructions' cost. Instructions for other programs need budgets of their
/// own on top.
#[must_use]
pub fn set_compute_unit_limit_instruction(units: u32) -> solana_instruction::Instruction {
	let mut data = Vec::with_capacity(5);
	data.push(SET_COMPUTE_UNIT_LIMIT_TAG);
	data.extend_from_slice(&units.to_le_bytes());

	solana_instruction::Instruction {
		program_id: COMPUTE_BUDGET_PROGRAM_ID,
		accounts: Vec::new(),
		data,
	}
}"#
	.to_string()
}

pub(crate) fn render_programs_mod(programs: &[&ProgramNode]) -> Result<String> {
	let mut lines = Vec::new();
	lines.push("use solana_pubkey::{pubkey, Pubkey};".to_string());
	lines.push(String::new());

	for program in programs {
		let program_name = program.name.as_ref();
		let public_key = canonical_pubkey(
			&program.public_key,
			&format!("program `{program_name}` public key"),
		)?;
		lines.push(format!(
			"pub const {}: Pubkey = pubkey!(\"{}\");",
			program_id_const_name(program_name),
			public_key
		));
	}

	Ok(lines.join("\n"))
}
