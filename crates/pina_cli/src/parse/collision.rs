//! Static validation for discriminator collisions and duplicate input names.
//!
//! These checks run after the IR has been fully assembled and surface errors
//! early — before serialization — so users get actionable diagnostics.

use std::collections::HashMap;

use crate::ir::ProgramIr;

/// A collision between two named entities sharing the same discriminator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscriminatorCollision {
	/// The kind of entity (e.g. "account", "instruction").
	pub kind: &'static str,
	/// First entity name.
	pub name_a: String,
	/// Second entity name.
	pub name_b: String,
	/// The colliding discriminator value.
	pub discriminator_value: u64,
	/// The discriminator repr size in bytes.
	pub repr_size: usize,
}

/// A duplicate input field name within a single instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateInputField {
	/// Instruction name.
	pub instruction: String,
	/// The duplicate field name.
	pub field_name: String,
	/// Sources of the field (e.g. `["account", "argument"]`).
	pub sources: Vec<&'static str>,
}

/// Check for discriminator collisions across all accounts and instructions.
///
/// Returns a list of collision descriptions. An empty list means no
/// collisions.
pub fn find_discriminator_collisions(ir: &ProgramIr) -> Vec<DiscriminatorCollision> {
	let mut collisions = Vec::new();

	// Check within accounts.
	check_collisions_within(
		"account",
		&ir.accounts,
		|a| (&a.name, a.discriminator.value, a.discriminator.repr_size),
		&mut collisions,
	);

	// Check within instructions.
	check_collisions_within(
		"instruction",
		&ir.instructions,
		|i| (&i.name, i.discriminator.value, i.discriminator.repr_size),
		&mut collisions,
	);

	collisions
}

/// Generic O(n²) collision check within a single kind.
fn check_collisions_within<T>(
	kind: &'static str,
	items: &[T],
	extract: impl Fn(&T) -> (&str, u64, usize),
	collisions: &mut Vec<DiscriminatorCollision>,
) {
	for (i, item_a) in items.iter().enumerate() {
		let (name_a, val_a, repr_a) = extract(item_a);
		for item_b in &items[(i + 1)..] {
			let (name_b, val_b, _repr_b) = extract(item_b);
			if val_a == val_b {
				collisions.push(DiscriminatorCollision {
					kind,
					name_a: name_a.to_owned(),
					name_b: name_b.to_owned(),
					discriminator_value: val_a,
					repr_size: repr_a,
				});
			}
		}
	}
}

/// Check for duplicate input field names within instructions.
///
/// An instruction's "input" consists of both its account names and its
/// argument names. Having duplicates would cause ambiguity in client
/// generation.
pub fn find_duplicate_input_fields(ir: &ProgramIr) -> Vec<DuplicateInputField> {
	let mut duplicates = Vec::new();

	for instruction in &ir.instructions {
		let mut field_sources: HashMap<&str, Vec<&'static str>> = HashMap::new();

		for acct in &instruction.accounts {
			field_sources.entry(&acct.name).or_default().push("account");
		}

		for arg in &instruction.arguments {
			field_sources
				.entry(&arg.name)
				.or_default()
				.push("argument");
		}

		for (name, sources) in &field_sources {
			if sources.len() > 1 {
				duplicates.push(DuplicateInputField {
					instruction: instruction.name.clone(),
					field_name: (*name).to_owned(),
					sources: sources.clone(),
				});
			}
		}
	}

	duplicates
}

/// Format collision errors into human-readable messages.
pub fn format_collision_errors(collisions: &[DiscriminatorCollision]) -> Vec<String> {
	collisions
		.iter()
		.map(|c| {
			format!(
				"{} '{}' and {} '{}' share discriminator value {} (repr: {} byte{})",
				c.kind,
				c.name_a,
				c.kind,
				c.name_b,
				c.discriminator_value,
				c.repr_size,
				if c.repr_size == 1 { "" } else { "s" },
			)
		})
		.collect()
}

/// Format duplicate input field errors into human-readable messages.
pub fn format_duplicate_field_errors(duplicates: &[DuplicateInputField]) -> Vec<String> {
	duplicates
		.iter()
		.map(|d| {
			format!(
				"instruction '{}' has duplicate input field '{}' from {}",
				d.instruction,
				d.field_name,
				d.sources.join(" + "),
			)
		})
		.collect()
}

#[cfg(test)]
mod tests {
	use crate::ir::AccountIr;
	use crate::ir::DiscriminatorIr;
	use crate::ir::FieldIr;
	use crate::ir::InstructionAccountIr;
	use crate::ir::InstructionIr;

	use super::*;

	fn make_account(name: &str, disc_value: u64) -> AccountIr {
		AccountIr {
			name: name.to_owned(),
			fields: vec![],
			discriminator: DiscriminatorIr {
				value: disc_value,
				repr_size: 1,
			},
			docs: vec![],
		}
	}

	fn make_instruction(
		name: &str,
		disc_value: u64,
		account_names: &[&str],
		arg_names: &[&str],
	) -> InstructionIr {
		InstructionIr {
			name: name.to_owned(),
			accounts: account_names
				.iter()
				.map(|n| InstructionAccountIr {
					name: (*n).to_owned(),
					is_writable: false,
					is_signer: false,
					is_optional: false,
					default_value: None,
					is_pda: false,
					pda_name: None,
					docs: vec![],
				})
				.collect(),
			arguments: arg_names
				.iter()
				.map(|n| FieldIr {
					name: (*n).to_owned(),
					rust_type: "u64".to_owned(),
					docs: vec![],
				})
				.collect(),
			discriminator: DiscriminatorIr {
				value: disc_value,
				repr_size: 1,
			},
			docs: vec![],
		}
	}

	fn make_program(accounts: Vec<AccountIr>, instructions: Vec<InstructionIr>) -> ProgramIr {
		ProgramIr {
			name: "test".to_owned(),
			public_key: "1111111111111111111111111111111111".to_owned(),
			accounts,
			instructions,
			errors: vec![],
			pdas: vec![],
		}
	}

	// ---- discriminator collision tests ----

	#[test]
	fn no_collisions_when_discriminators_differ() {
		let ir = make_program(
			vec![make_account("A", 0), make_account("B", 1)],
			vec![],
		);
		assert!(find_discriminator_collisions(&ir).is_empty());
	}

	#[test]
	fn detects_account_discriminator_collision() {
		let ir = make_program(
			vec![make_account("A", 0), make_account("B", 0)],
			vec![],
		);
		let collisions = find_discriminator_collisions(&ir);
		assert_eq!(collisions.len(), 1);
		assert_eq!(collisions[0].kind, "account");
		assert_eq!(collisions[0].name_a, "A");
		assert_eq!(collisions[0].name_b, "B");
		assert_eq!(collisions[0].discriminator_value, 0);
	}

	#[test]
	fn detects_instruction_discriminator_collision() {
		let ir = make_program(
			vec![],
			vec![
				make_instruction("init", 0, &["authority"], &[]),
				make_instruction("update", 0, &["authority"], &[]),
			],
		);
		let collisions = find_discriminator_collisions(&ir);
		assert_eq!(collisions.len(), 1);
		assert_eq!(collisions[0].kind, "instruction");
	}

	#[test]
	fn no_cross_kind_collisions() {
		// Account disc=0 and instruction disc=0 should NOT collide.
		let ir = make_program(
			vec![make_account("AccountA", 0)],
			vec![make_instruction("init", 0, &["authority"], &[])],
		);
		assert!(find_discriminator_collisions(&ir).is_empty());
	}

	#[test]
	fn multiple_collisions() {
		let ir = make_program(
			vec![
				make_account("A", 0),
				make_account("B", 0),
				make_account("C", 1),
				make_account("D", 1),
			],
			vec![],
		);
		let collisions = find_discriminator_collisions(&ir);
		assert_eq!(collisions.len(), 2);
	}

	#[test]
	fn three_way_collision_produces_three_pairs() {
		let ir = make_program(
			vec![
				make_account("A", 0),
				make_account("B", 0),
				make_account("C", 0),
			],
			vec![],
		);
		let collisions = find_discriminator_collisions(&ir);
		// (A,B), (A,C), (B,C)
		assert_eq!(collisions.len(), 3);
	}

	// ---- duplicate input field tests ----

	#[test]
	fn no_duplicates_when_names_differ() {
		let ir = make_program(
			vec![],
			vec![make_instruction("init", 0, &["authority"], &["amount"])],
		);
		assert!(find_duplicate_input_fields(&ir).is_empty());
	}

	#[test]
	fn detects_duplicate_account_arg_name() {
		let ir = make_program(
			vec![],
			vec![make_instruction("init", 0, &["amount"], &["amount"])],
		);
		let dupes = find_duplicate_input_fields(&ir);
		assert_eq!(dupes.len(), 1);
		assert_eq!(dupes[0].instruction, "init");
		assert_eq!(dupes[0].field_name, "amount");
	}

	#[test]
	fn no_false_positives_across_instructions() {
		let ir = make_program(
			vec![],
			vec![
				make_instruction("init", 0, &["authority"], &[]),
				make_instruction("update", 1, &["authority"], &[]),
			],
		);
		assert!(find_duplicate_input_fields(&ir).is_empty());
	}

	// ---- formatting tests ----

	#[test]
	fn format_collision_message() {
		let collision = DiscriminatorCollision {
			kind: "account",
			name_a: "Counter".to_owned(),
			name_b: "Vault".to_owned(),
			discriminator_value: 0,
			repr_size: 1,
		};
		let msgs = format_collision_errors(&[collision]);
		assert_eq!(msgs.len(), 1);
		assert!(msgs[0].contains("Counter"));
		assert!(msgs[0].contains("Vault"));
		assert!(msgs[0].contains("discriminator value 0"));
	}

	#[test]
	fn format_duplicate_field_message() {
		let dup = DuplicateInputField {
			instruction: "init".to_owned(),
			field_name: "amount".to_owned(),
			sources: vec!["account", "argument"],
		};
		let msgs = format_duplicate_field_errors(&[dup]);
		assert_eq!(msgs.len(), 1);
		assert!(msgs[0].contains("init"));
		assert!(msgs[0].contains("amount"));
		assert!(msgs[0].contains("account + argument"));
	}

	// ---- error variants ----

	#[test]
	fn collision_errors_for_empty_programs() {
		let ir = make_program(vec![], vec![]);
		assert!(find_discriminator_collisions(&ir).is_empty());
		assert!(find_duplicate_input_fields(&ir).is_empty());
	}

	#[test]
	fn single_account_no_collision() {
		let ir = make_program(vec![make_account("Only", 42)], vec![]);
		assert!(find_discriminator_collisions(&ir).is_empty());
	}

	#[test]
	fn single_instruction_no_collision() {
		let ir = make_program(
			vec![],
			vec![make_instruction("only", 42, &["auth"], &["val"])],
		);
		assert!(find_discriminator_collisions(&ir).is_empty());
	}
}
