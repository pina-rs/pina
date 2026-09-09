//! Extraction of migration-aware event data schemas.

use syn::File;
use syn::Item;

use super::discriminator::extract_discriminator_and_variant;
use crate::error::IdlError;

/// One `#[event(..., migrations)]` source declaration.
#[derive(Debug, Clone)]
pub struct EventStruct {
	pub name: String,
	pub discriminator_enum: String,
	pub variant: String,
	pub schema: pina_abi::DataSchema,
}

/// Extract only events opted into ABI history.
pub fn extract_migratable_events(file: &File) -> Result<Vec<EventStruct>, IdlError> {
	let mut events = Vec::new();
	for item in &file.items {
		let Item::Struct(item_struct) = item else {
			continue;
		};
		let Some((discriminator_enum, variant)) =
			extract_discriminator_and_variant(&item_struct.attrs, "event", &item_struct.ident)?
		else {
			continue;
		};
		if !has_migrations_flag(&item_struct.attrs, "event") {
			continue;
		}
		let schema = pina_abi::data_schema(item_struct, pina_abi::LayoutKind::Fixed)
			.map_err(IdlError::Other)?;
		events.push(EventStruct {
			name: item_struct.ident.to_string(),
			discriminator_enum,
			variant,
			schema,
		});
	}
	Ok(events)
}

/// Return whether one schema attribute contains the bare `migrations` flag.
pub(crate) fn has_migrations_flag(attrs: &[syn::Attribute], attribute: &str) -> bool {
	attrs.iter().any(|attr| {
		if !attr.path().is_ident(attribute) {
			return false;
		}
		attr.parse_args_with(
			syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
		)
		.is_ok_and(|items| items.iter().any(|item| item.path().is_ident("migrations")))
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn extracts_only_migration_aware_events() {
		let file = syn::parse_file(
			r#"
				#[event(discriminator = Events::Current, migrations)]
				struct Current { value: u64 }

				#[event(discriminator = Events::Ephemeral)]
				struct Ephemeral { value: u64 }
			"#,
		)
		.unwrap_or_else(|error| panic!("parse: {error}"));
		let events =
			extract_migratable_events(&file).unwrap_or_else(|error| panic!("extract: {error}"));

		assert_eq!(events.len(), 1);
		assert_eq!(events[0].name, "Current");
		assert_eq!(events[0].schema.fields[0].rust_type, "u64");
	}
}
