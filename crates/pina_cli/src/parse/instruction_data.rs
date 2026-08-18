use syn::File;
use syn::Item;

use super::discriminator::extract_discriminator_and_variant;
use super::doc_comments::extract_docs;
use super::types::type_to_string;
use crate::error::IdlError;
use crate::ir::FieldIr;

/// A parsed `#[instruction(discriminator = ..., variant = ...)]` struct.
#[derive(Debug, Clone)]
pub struct InstructionStruct {
	pub name: String,
	pub discriminator_enum: String,
	pub variant: String,
	pub fields: Vec<FieldIr>,
	pub docs: Vec<String>,
}

/// Extract all `#[instruction(...)]` structs from a file.
///
/// # Errors
///
/// Returns an error when an instruction discriminator does not match the
/// grammar accepted by the attribute macro.
pub fn extract_instruction_structs(file: &File) -> Result<Vec<InstructionStruct>, IdlError> {
	let mut result = Vec::new();

	for item in &file.items {
		let Item::Struct(item_struct) = item else {
			continue;
		};

		let Some((disc_enum, variant)) = extract_discriminator_and_variant(
			&item_struct.attrs,
			"instruction",
			&item_struct.ident,
		)?
		else {
			continue;
		};

		let fields = extract_named_fields(&item_struct.fields);
		let docs = extract_docs(&item_struct.attrs);

		result.push(InstructionStruct {
			name: item_struct.ident.to_string(),
			discriminator_enum: disc_enum,
			variant,
			fields,
			docs,
		});
	}

	Ok(result)
}

fn extract_named_fields(fields: &syn::Fields) -> Vec<FieldIr> {
	let syn::Fields::Named(named) = fields else {
		return Vec::new();
	};

	named
		.named
		.iter()
		.map(|f| {
			let name = f
				.ident
				.as_ref()
				.map_or_else(|| "unknown".to_owned(), ToString::to_string);
			let rust_type = type_to_string(&f.ty);
			let docs = extract_docs(&f.attrs);
			FieldIr {
				name,
				rust_type,
				docs,
			}
		})
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn extracts_instruction_struct() {
		let source = r#"
			#[instruction(discriminator = CounterInstruction, variant = Initialize)]
			pub struct InitializeInstruction {
				pub bump: u8,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let instructions =
			extract_instruction_structs(&file).unwrap_or_else(|e| panic!("extract failed: {e}"));
		assert_eq!(instructions.len(), 1);
		assert_eq!(instructions[0].name, "InitializeInstruction");
		assert_eq!(instructions[0].discriminator_enum, "CounterInstruction");
		assert_eq!(instructions[0].variant, "Initialize");
		assert_eq!(instructions[0].fields.len(), 1);
		assert_eq!(instructions[0].fields[0].name, "bump");
	}

	#[test]
	fn extracts_instruction_struct_with_path_variant() {
		let source = r#"
			#[instruction(discriminator = CounterInstruction::Initialize)]
			pub struct InitializeInstruction {
				pub bump: u8,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let instructions =
			extract_instruction_structs(&file).unwrap_or_else(|e| panic!("extract failed: {e}"));
		assert_eq!(instructions.len(), 1);
		assert_eq!(instructions[0].name, "InitializeInstruction");
		assert_eq!(instructions[0].discriminator_enum, "CounterInstruction");
		assert_eq!(instructions[0].variant, "Initialize");
	}

	#[test]
	fn extracts_instruction_struct_with_shorthand_variant() {
		let source = r#"
			#[instruction(discriminator = CounterInstruction)]
			pub struct Initialize {
				pub bump: u8,
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let instructions =
			extract_instruction_structs(&file).unwrap_or_else(|e| panic!("extract failed: {e}"));
		assert_eq!(instructions.len(), 1);
		assert_eq!(instructions[0].discriminator_enum, "CounterInstruction");
		assert_eq!(instructions[0].variant, "Initialize");
	}

	#[test]
	fn extracts_empty_instruction_struct() {
		let source = r#"
			#[instruction(discriminator = CounterInstruction, variant = Increment)]
			pub struct IncrementInstruction {}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let instructions =
			extract_instruction_structs(&file).unwrap_or_else(|e| panic!("extract failed: {e}"));
		assert_eq!(instructions.len(), 1);
		assert_eq!(instructions[0].fields.len(), 0);
	}

	#[test]
	fn extracts_qualified_discriminator_with_explicit_variant() {
		let source = r#"
			#[instruction(discriminator = crate::types::CounterInstruction, variant = Initialize)]
			pub struct InitializeInstruction {}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let instructions =
			extract_instruction_structs(&file).unwrap_or_else(|e| panic!("extract failed: {e}"));

		assert_eq!(instructions[0].discriminator_enum, "CounterInstruction");
		assert_eq!(instructions[0].variant, "Initialize");
	}

	#[test]
	fn extracts_qualified_discriminator_with_path_variant() {
		let source = r#"
			#[instruction(discriminator = crate::types::CounterInstruction::Initialize)]
			pub struct InitializeInstruction {}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let instructions =
			extract_instruction_structs(&file).unwrap_or_else(|e| panic!("extract failed: {e}"));

		assert_eq!(instructions[0].discriminator_enum, "CounterInstruction");
		assert_eq!(instructions[0].variant, "Initialize");
	}

	#[test]
	fn rejects_missing_discriminator_argument() {
		let source = r#"
			#[instruction(variant = Initialize)]
			pub struct InitializeInstruction {}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let error =
			extract_instruction_structs(&file).expect_err("missing discriminator must fail");

		assert!(
			error
				.to_string()
				.contains("missing `discriminator` argument")
		);
	}

	#[test]
	fn rejects_multi_segment_variant() {
		let source = r#"
			#[instruction(discriminator = CounterInstruction, variant = types::Initialize)]
			pub struct InitializeInstruction {}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let error = extract_instruction_structs(&file).expect_err("qualified variant must fail");

		assert!(
			error
				.to_string()
				.contains("`variant` must be a single identifier")
		);
	}
}
