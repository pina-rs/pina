use syn::File;
use syn::Item;

use super::doc_comments::extract_docs;
use super::types::type_to_string;
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
pub fn extract_instruction_structs(file: &File) -> Vec<InstructionStruct> {
	let mut result = Vec::new();

	for item in &file.items {
		let Item::Struct(item_struct) = item else {
			continue;
		};

		let Some((disc_enum, variant)) =
			get_instruction_discriminator_and_variant(&item_struct.attrs)
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

	result
}

/// Parse `discriminator = EnumType, variant = Variant` from
/// `#[instruction(...)]`.
fn get_instruction_discriminator_and_variant(attrs: &[syn::Attribute]) -> Option<(String, String)> {
	for attr in attrs {
		if !attr.path().is_ident("instruction") {
			continue;
		}
		let Ok(meta_list) = attr.parse_args_with(
			syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
		) else {
			continue;
		};
		let mut disc = None;
		let mut variant = None;
		for meta in &meta_list {
			if let syn::Meta::NameValue(nv) = meta {
				if nv.path.is_ident("discriminator") {
					disc = Some(expr_to_string(&nv.value));
				} else if nv.path.is_ident("variant") {
					variant = Some(expr_to_string(&nv.value));
				}
			}
		}
		if let (Some(d), Some(v)) = (disc, variant) {
			return Some((d, v));
		}
	}
	None
}

fn expr_to_string(expr: &syn::Expr) -> String {
	match expr {
		syn::Expr::Path(p) => {
			p.path
				.segments
				.iter()
				.map(|s| s.ident.to_string())
				.collect::<Vec<_>>()
				.join("::")
		}
		_ => "unknown".to_owned(),
	}
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
		let instructions = extract_instruction_structs(&file);
		assert_eq!(instructions.len(), 1);
		assert_eq!(instructions[0].name, "InitializeInstruction");
		assert_eq!(instructions[0].discriminator_enum, "CounterInstruction");
		assert_eq!(instructions[0].variant, "Initialize");
		assert_eq!(instructions[0].fields.len(), 1);
		assert_eq!(instructions[0].fields[0].name, "bump");
	}

	#[test]
	fn extracts_empty_instruction_struct() {
		let source = r#"
			#[instruction(discriminator = CounterInstruction, variant = Increment)]
			pub struct IncrementInstruction {}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let instructions = extract_instruction_structs(&file);
		assert_eq!(instructions.len(), 1);
		assert_eq!(instructions[0].fields.len(), 0);
	}
}
