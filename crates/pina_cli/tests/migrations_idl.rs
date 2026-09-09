use std::path::PathBuf;

fn example(name: &str) -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("../..")
		.join("examples")
		.join(name)
}

#[test]
fn current_idl_contains_only_omitted_migration_constants() {
	let program = example("migrations_program");
	let root = pina_cli::generate_idl(&program, None)
		.unwrap_or_else(|error| panic!("generate migration-aware IDL: {error}"));
	let idl = serde_json::to_value(root)
		.unwrap_or_else(|error| panic!("serialize migration-aware IDL: {error}"));

	for account_index in 0..3 {
		let base = format!("/program/accounts/{account_index}");
		assert_eq!(
			idl.pointer(&format!("{base}/data/fields/1/name")),
			Some(&serde_json::json!("migrationVersion")),
		);
		assert_eq!(
			idl.pointer(&format!("{base}/data/fields/1/defaultValueStrategy")),
			Some(&serde_json::json!("omitted")),
		);
		assert_eq!(
			idl.pointer(&format!("{base}/data/fields/1/defaultValue/number")),
			Some(&serde_json::json!(1)),
		);
		assert_eq!(
			idl.pointer(&format!("{base}/discriminators/1/offset")),
			Some(&serde_json::json!(1)),
		);
	}

	let update = "/program/instructions/0";
	assert_eq!(
		idl.pointer(&format!("{update}/arguments/1/name")),
		Some(&serde_json::json!("migrationVersion")),
	);
	assert_eq!(
		idl.pointer(&format!("{update}/arguments/1/defaultValueStrategy")),
		Some(&serde_json::json!("omitted")),
	);
	assert_eq!(
		idl.pointer(&format!("{update}/arguments/1/defaultValue/number")),
		Some(&serde_json::json!(1)),
	);
	assert_eq!(
		idl.pointer(&format!("{update}/discriminators/1/offset")),
		Some(&serde_json::json!(1)),
	);

	let serialized = idl.to_string();
	assert!(!serialized.contains("schemaSha256"));
	assert!(!serialized.contains("transition"));
	assert!(!serialized.contains("sourceProcessSha256"));
}

#[test]
fn non_migratable_idl_does_not_enter_migration_discovery() {
	pina_cli::generate_idl(&example("validation_program"), None)
		.unwrap_or_else(|error| panic!("generate ordinary IDL: {error:?}"));
}
