//! Extraction of migration-aware event data schemas.

use quote::ToTokens as _;
use syn::File;
use syn::Item;

use super::MigrationOptIn;
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
///
/// An event is migration-aware when its declaration carries the `migrations`
/// token or `auto` covers events. `migrations = false` always wins.
pub fn extract_migratable_events(file: &File, auto: bool) -> Result<Vec<EventStruct>, IdlError> {
	let mut events = Vec::new();
	for item in &file.items {
		let Item::Struct(item_struct) = item else {
			continue;
		};
		// Non-migratable events may use event-macro arguments that the ABI
		// snapshot parser does not own (for example `validate(...)`). Ignore
		// those declarations before parsing the narrower migration syntax.
		let Some(opt_in) = migrations_opt_in(&item_struct.attrs, "event")? else {
			continue;
		};
		if !opt_in.is_enabled(auto) {
			continue;
		}
		let (discriminator_enum, variant) =
			extract_discriminator_and_variant(&item_struct.attrs, "event", &item_struct.ident)?
				.expect("an event attribute with `migrations` always supplies the parsed event");
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

/// Parse the `migrations`, `migrations = true`, or `migrations = false`
/// argument from one schema attribute.
///
/// Returns `None` when the declaration does not carry `attribute` at all, so a
/// bare struct in an auto-policy program is not mistaken for a declaration. A
/// known `migrations` argument with a non-boolean value is an error: silently
/// reading it as unspecified would let the auto policy envelop a declaration
/// that never validly opted in (or out).
pub(crate) fn migrations_opt_in(
	attrs: &[syn::Attribute],
	attribute: &str,
) -> Result<Option<MigrationOptIn>, IdlError> {
	let mut opt_in = MigrationOptIn::Unspecified;
	let mut found = false;
	for attr in attrs {
		if !attr.path().is_ident(attribute) {
			continue;
		}
		found = true;
		let Ok(items) = attr.parse_args_with(
			syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
		) else {
			continue;
		};
		for item in items {
			match &item {
				syn::Meta::Path(path) if path.is_ident("migrations") => {
					opt_in = MigrationOptIn::Explicit;
				}
				syn::Meta::NameValue(value) if value.path.is_ident("migrations") => {
					match &value.value {
						syn::Expr::Lit(syn::ExprLit {
							lit: syn::Lit::Bool(literal),
							..
						}) => {
							opt_in = if literal.value {
								MigrationOptIn::Explicit
							} else {
								MigrationOptIn::Disabled
							};
						}
						other => {
							return Err(IdlError::Other(format!(
								"`migrations = {}` on a `{attribute}` schema is not a boolean; \
								 use `migrations`, `migrations = true`, or `migrations = false`",
								other.to_token_stream()
							)));
						}
					}
				}
				_ => {}
			}
		}
	}
	Ok(found.then_some(opt_in))
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
		let events = extract_migratable_events(&file, false)
			.unwrap_or_else(|error| panic!("extract: {error}"));

		assert_eq!(events.len(), 1);
		assert_eq!(events[0].name, "Current");
		assert_eq!(events[0].schema.fields[0].rust_type, "u64");
	}

	#[test]
	fn auto_policy_includes_unannotated_events_and_disabled_ones_stay_out() {
		let file = syn::parse_file(
			r#"
				#[event(discriminator = Events::Current)]
				struct Current { value: u64 }

				#[event(discriminator = Events::Disabled, migrations = false)]
				struct Disabled { value: u64 }
			"#,
		)
		.unwrap_or_else(|error| panic!("parse: {error}"));
		let events = extract_migratable_events(&file, true)
			.unwrap_or_else(|error| panic!("extract: {error}"));

		assert_eq!(events.len(), 1);
		assert_eq!(events[0].name, "Current");
		assert!(
			extract_migratable_events(&file, false)
				.unwrap_or_else(|error| panic!("extract: {error}"))
				.is_empty()
		);
	}

	#[test]
	fn parses_every_migrations_argument_spelling() {
		let file = syn::parse_file(
			r#"
				#[event(discriminator = Events::Bare, migrations)]
				struct Bare { value: u64 }

				#[event(discriminator = Events::True, migrations = true)]
				struct True { value: u64 }

				#[event(discriminator = Events::False, migrations = false)]
				struct False { value: u64 }

				#[event(discriminator = Events::None)]
				struct None { value: u64 }
			"#,
		)
		.unwrap_or_else(|error| panic!("parse: {error}"));
		let states = file
			.items
			.iter()
			.map(|item| {
				match item {
					Item::Struct(item_struct) => {
						migrations_opt_in(&item_struct.attrs, "event").unwrap_or_default()
					}
					_ => None,
				}
			})
			.collect::<Vec<_>>();

		assert_eq!(
			states,
			[
				Some(MigrationOptIn::Explicit),
				Some(MigrationOptIn::Explicit),
				Some(MigrationOptIn::Disabled),
				// The attribute exists but has no `migrations` argument, so the
				// auto policy decides rather than the declaration.
				Some(MigrationOptIn::Unspecified),
			]
		);
	}

	#[test]
	fn absent_schema_attributes_have_no_opt_in() {
		let file = syn::parse_file(
			r#"
				struct Plain { value: u64 }

				#[account(discriminator = Kind::State)]
				struct State { value: u64 }
			"#,
		)
		.unwrap_or_else(|error| panic!("parse: {error}"));

		let states = file
			.items
			.iter()
			.filter_map(|item| {
				match item {
					Item::Struct(item_struct) => {
						migrations_opt_in(&item_struct.attrs, "event")
							.ok()
							.flatten()
					}
					_ => None,
				}
			})
			.collect::<Vec<_>>();

		assert!(states.is_empty());
	}

	#[test]
	fn non_boolean_migrations_values_are_rejected() {
		let file = syn::parse_file(
			r#"
				#[event(discriminator = Events::Stale, migrations = "false")]
				struct Stale { value: u64 }
			"#,
		)
		.unwrap_or_else(|error| panic!("parse: {error}"));

		// The value is invalid rather than merely disabled, so the error fires
		// whatever the auto policy would decide.
		let error = extract_migratable_events(&file, false).expect_err("string must fail");
		let message = error.to_string();
		assert!(message.contains(r#""false""#), "message: {message}");
		assert!(message.contains("`event` schema"), "message: {message}");
		assert!(message.contains("not a boolean"), "message: {message}");
		assert!(extract_migratable_events(&file, true).is_err());

		let item: syn::ItemStruct = syn::parse_str(
			"#[event(discriminator = Events::Pinned, migrations = 1)] struct Pinned { value: u64 }",
		)
		.unwrap_or_else(|error| panic!("parse: {error}"));
		let pinned = migrations_opt_in(&item.attrs, "event").expect_err("integer must fail");
		assert!(
			pinned.to_string().contains("not a boolean"),
			"message: {pinned}"
		);
	}

	#[test]
	fn ignores_non_migratable_event_arguments_owned_by_other_features() {
		let file = syn::parse_file(
			r#"
				#[event(
					discriminator = Events::Validated,
					validate(with = validate_event)
				)]
				struct Validated { value: u64 }
			"#,
		)
		.unwrap_or_else(|error| panic!("parse: {error}"));

		let events = extract_migratable_events(&file, false)
			.unwrap_or_else(|error| panic!("extract: {error}"));
		assert!(events.is_empty());
	}

	#[test]
	fn declarations_record_which_events_opt_into_migration() {
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
			extract_event_declarations(&file).unwrap_or_else(|error| panic!("extract: {error}"));

		assert_eq!(events.len(), 2);
		assert_eq!(events[0].name, "Current");
		assert_eq!(events[0].migrations, MigrationOptIn::Explicit);
		assert_eq!(events[1].name, "Ephemeral");
		assert_eq!(events[1].migrations, MigrationOptIn::Unspecified);
	}
}

/// One `#[event]` source declaration of any flavor.
#[derive(Debug, Clone)]
pub struct EventDeclaration {
	pub name: String,
	pub discriminator_enum: String,
	pub variant: String,
	pub fields: Vec<crate::ir::FieldIr>,
	pub docs: Vec<String>,
	/// The declaration's migration opt-in, before policy resolution.
	pub migrations: MigrationOptIn,
}

/// Extract every `#[event]` struct, regardless of its event-macro arguments.
///
/// Unlike [`extract_migratable_events`] this does not require the `migrations`
/// flag and does not parse migration-specific arguments, so events decorated
/// with other event-macro options (for example `validate(...)`) still
/// contribute their identity and field schema to the IDL.
pub fn extract_event_declarations(file: &File) -> Result<Vec<EventDeclaration>, IdlError> {
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
		events.push(EventDeclaration {
			name: item_struct.ident.to_string(),
			discriminator_enum,
			variant,
			fields: super::account_state::extract_named_fields(&item_struct.fields),
			docs: super::doc_comments::extract_docs(&item_struct.attrs),
			migrations: migrations_opt_in(&item_struct.attrs, "event")?.unwrap_or_default(),
		});
	}
	Ok(events)
}
