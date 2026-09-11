//! Tests for the migration CLI, grouped by concern.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;

use pina_abi::ContractHistory;
use pina_abi::ContractIdentity;
use pina_abi::ContractKind;
use pina_abi::DataSchema;
use pina_abi::FieldSchema;
use pina_abi::LayoutKind;
use pina_abi::MANIFEST_PATH;
use pina_abi::MigrationManifest;
use pina_abi::MigrationVersionType;
use pina_abi::PUBLICATIONS_PATH;
use pina_abi::ProcessAccount;
use pina_abi::ProcessContract;
use pina_abi::PublicationLedger;
use pina_abi::PublishedContract;
use pina_abi::SchemaVersion;
use pina_abi::Transition;
use pina_abi::TransitionMode;
use serde::Serialize;
use sha2::Digest as _;
use sha2::Sha256;
use tempfile::TempDir;

use super::diff::*;
use super::ledger::*;
use super::prompt::*;
use super::scan::*;
use super::storage::*;
use super::transition::*;
use super::*;
use crate::ir::DefaultValueIr;
use crate::ir::DiscriminatorIr;
use crate::ir::InstructionIr;
use crate::project::Project;

fn schema(layout: LayoutKind, fields: &[(&str, &str)]) -> DataSchema {
	DataSchema::try_new(
		layout,
		fields
			.iter()
			.map(|(name, rust_type)| {
				FieldSchema {
					name: (*name).to_owned(),
					rust_type: (*rust_type).to_owned(),
				}
			})
			.collect(),
	)
	.unwrap_or_else(|error| panic!("valid test schema: {error}"))
}

#[test]
fn automatic_fixed_migration_allows_direction_safe_add_and_remove() {
	let old = schema(
		LayoutKind::Fixed,
		&[("authority", "Address"), ("count", "u64")],
	);
	let new = schema(LayoutKind::Fixed, &[("count", "u64"), ("enabled", "bool")]);

	assert_eq!(transition_mode(&old, &new), TransitionMode::Automatic);
}

#[test]
fn automatic_fixed_migration_rejects_true_field_reordering() {
	let old = schema(
		LayoutKind::Fixed,
		&[("authority", "Address"), ("count", "u64")],
	);
	let reordered = schema(
		LayoutKind::Fixed,
		&[("count", "u64"), ("authority", "Address")],
	);

	assert_eq!(transition_mode(&old, &reordered), TransitionMode::Manual);
}

#[test]
fn configured_version_width_rejects_exhaustion_before_incrementing() {
	assert_eq!(
		next_migration_version("account:1:01", 254, MigrationVersionType::U8)
			.expect("u8 has one version remaining"),
		255
	);
	assert!(matches!(
		next_migration_version("account:1:01", 255, MigrationVersionType::U8),
		Err(MigrationError::VersionExhausted { identity, .. })
			if identity == "account:1:01"
	));
	assert_eq!(
		next_migration_version(
			"account:1:01",
			u32::from(u16::MAX),
			MigrationVersionType::U32,
		)
		.expect("u32 has remaining versions"),
		u32::from(u16::MAX) + 1
	);
}

#[test]
fn manual_account_transition_is_total_after_schema_preflight() {
	let account = ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap();
	let instruction = ContractIdentity::try_new(ContractKind::Instruction, 1, 2).unwrap();
	let source_schema = schema(LayoutKind::Fixed, &[("amount", "u8")]);
	let source = SchemaVersion {
		version: 0,
		schema_sha256: source_schema.sha256(),
		schema: source_schema,
		process: None,
		process_sha256: None,
		transition: None,
	};
	let destination = schema(LayoutKind::Fixed, &[("amount", "u16")]);

	let account_source =
		manual_transition_source(&account, MigrationVersionType::U8, &source, 1, &destination)
			.unwrap_or_else(|error| panic!("manual account transition: {error:?}"));
	assert!(account_source.contains("fn migrate(data: &mut [u8]) {"));
	assert!(!account_source.contains("fn migrate(data: &mut [u8]) -> bool"));
	assert!(account_source.contains("conversion must be total"));

	let instruction_source = manual_transition_source(
		&instruction,
		MigrationVersionType::U8,
		&source,
		1,
		&destination,
	)
	.unwrap_or_else(|error| panic!("manual transition: {error:?}"));

	assert!(instruction_source.contains("fn migrate(data: &mut [u8]) -> bool"));
}

#[test]
fn compact_account_transition_uses_checked_runtime_sizing() {
	let account = ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap();
	let source_schema = schema(LayoutKind::Compact, &[("name", "String<4>")]);
	let source = SchemaVersion {
		version: 0,
		schema_sha256: source_schema.sha256(),
		schema: source_schema,
		process: None,
		process_sha256: None,
		transition: None,
	};
	let destination = schema(
		LayoutKind::Compact,
		&[("name", "String<4>"), ("tags", "Vec<u16, 2>")],
	);

	let generated =
		manual_transition_source(&account, MigrationVersionType::U8, &source, 1, &destination)
			.unwrap_or_else(|error| panic!("manual transition: {error:?}"));

	assert!(generated.contains("Source version: 0 (variable bytes)"));
	assert!(generated.contains("fn target_size(data: &[u8]) -> Option<usize>"));
	assert!(generated.contains("fn working_size("));
	assert!(!generated.contains("const SOURCE_SIZE: usize = dynamic"));
}

#[test]
fn process_transition_accepts_optional_suffix_and_rejects_privilege_changes() {
	let identity = ContractIdentity::try_new(ContractKind::Instruction, 1, 7).unwrap();
	let authority = ProcessAccount {
		name: "authority".to_owned(),
		writable: false,
		signer: true,
		optional: false,
		default_value: None,
		pda: None,
		constraints: vec![],
	};
	let source = ProcessContract {
		accounts: vec![authority.clone()],
	};
	let destination = ProcessContract {
		accounts: vec![
			authority.clone(),
			ProcessAccount {
				name: "referrer".to_owned(),
				writable: false,
				signer: false,
				optional: true,
				default_value: None,
				pda: None,
				constraints: vec![],
			},
		],
	};
	assert!(process_transition(&identity, "Transfer", Some(&source), Some(&destination),).is_ok());

	let mut escalated = source.clone();
	escalated.accounts[0].writable = true;
	assert!(matches!(
		process_transition(&identity, "Transfer", Some(&source), Some(&escalated),),
		Err(MigrationError::ProcessChanged { .. })
	));
	assert!(matches!(
		process_transition(&identity, "Transfer", Some(&source), None),
		Err(MigrationError::InvalidHistory(_))
	));

	let account = ContractIdentity::try_new(ContractKind::Account, 1, 7).unwrap();
	assert!(matches!(
		process_transition(&account, "State", Some(&source), None),
		Err(MigrationError::InvalidHistory(_))
	));
}

#[test]
fn transition_creation_propagates_process_and_directory_failures() {
	let fixture = migration_fixture();
	let project = Project::discover(&fixture.root)
		.unwrap_or_else(|error| panic!("discover transition fixture: {error}"));
	let identity =
		ContractIdentity::try_new(ContractKind::Instruction, 1, 7).expect("valid identity");
	let source_schema = schema(LayoutKind::Fixed, &[("value", "u64")]);
	let source_process = ProcessContract {
		accounts: vec![ProcessAccount {
			name: "authority".to_owned(),
			writable: false,
			signer: true,
			optional: false,
			default_value: None,
			pda: None,
			constraints: vec![],
		}],
	};
	let source = SchemaVersion {
		version: 0,
		schema_sha256: source_schema.sha256(),
		schema: source_schema.clone(),
		process_sha256: Some(source_process.sha256()),
		process: Some(source_process.clone()),
		transition: None,
	};
	let mut escalated = source_process;
	escalated.accounts[0].writable = true;
	assert!(matches!(
		create_transition(
			&project,
			TransitionRequest {
				identity: &identity,
				rust_name: "Update",
				source: &source,
				renames: Vec::new(),
				destination_version: 1,
				destination: &source_schema,
				destination_process: Some(&escalated),
				preserve_manual: false,
			},
			&mut MakeMigrationsOutput::default(),
		),
		Err(MigrationError::ProcessChanged { .. })
	));

	let blocked = migration_fixture();
	std::fs::write(blocked.root.join("migrations/transitions"), b"blocked")
		.unwrap_or_else(|error| panic!("block transition directory: {error}"));
	let project = Project::discover(&blocked.root)
		.unwrap_or_else(|error| panic!("discover blocked fixture: {error}"));
	let account = ContractIdentity::try_new(ContractKind::Account, 1, 1).expect("valid identity");
	let account_source = SchemaVersion {
		version: 0,
		schema_sha256: source_schema.sha256(),
		schema: source_schema,
		process_sha256: None,
		process: None,
		transition: None,
	};
	let destination = schema(LayoutKind::Fixed, &[("value", "u64"), ("enabled", "bool")]);
	assert!(matches!(
		create_transition(
			&project,
			TransitionRequest {
				identity: &account,
				rust_name: "State",
				source: &account_source,
				renames: Vec::new(),
				destination_version: 1,
				destination: &destination,
				destination_process: None,
				preserve_manual: false,
			},
			&mut MakeMigrationsOutput::default(),
		),
		Err(MigrationError::CreateDirectory { .. })
	));
}

#[test]
fn migration_lifecycle_propagates_transition_failures_for_frozen_and_draft_versions() {
	let frozen = publication_fixture();
	publish_current(&frozen);
	std::fs::write(frozen.root.join("migrations/transitions"), b"blocked")
		.unwrap_or_else(|error| panic!("block frozen transition directory: {error}"));
	write_state_source(&frozen, "value: u64, enabled: bool");
	assert!(matches!(
		make_migrations(&frozen.root),
		Err(MigrationError::CreateDirectory { .. })
	));

	let draft = publication_fixture();
	publish_current(&draft);
	write_state_source(&draft, "value: u64, enabled: bool");
	make_migrations(&draft.root)
		.unwrap_or_else(|error| panic!("create version-one draft: {error}"));
	let transitions = draft.root.join("migrations/transitions");
	std::fs::remove_dir_all(&transitions)
		.unwrap_or_else(|error| panic!("remove generated transitions: {error}"));
	std::fs::write(&transitions, b"blocked")
		.unwrap_or_else(|error| panic!("block draft transition directory: {error}"));
	write_state_source(&draft, "value: u64, enabled: bool, counter: u16");
	assert!(matches!(
		make_migrations(&draft.root),
		Err(MigrationError::CreateDirectory { .. })
	));
}

#[test]
fn direction_and_manual_sizing_cover_every_layout_shape() {
	let mixed_source = schema(
		LayoutKind::Fixed,
		&[("removed", "u64"), ("first", "u8"), ("second", "u8")],
	);
	let mixed_destination = schema(
		LayoutKind::Fixed,
		&[("first", "u8"), ("inserted", "u128"), ("second", "u8")],
	);
	assert!(automatic_direction(&mixed_source, &mixed_destination).is_none());

	let source_schema = schema(LayoutKind::Fixed, &[("value", "u64")]);
	let destination = schema(LayoutKind::Fixed, &[("prefix", "u8"), ("value", "u64")]);
	assert!(matches!(
		automatic_direction(&source_schema, &destination),
		Some(MoveDirection::Backward)
	));
	let source = SchemaVersion {
		version: 0,
		schema_sha256: source_schema.sha256(),
		schema: source_schema,
		process: None,
		process_sha256: None,
		transition: None,
	};
	let identity = ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap();
	let generated = automatic_transition_source(
		&identity,
		MigrationVersionType::U8,
		&source,
		1,
		&destination,
	);
	assert!(generated.contains("copy_within(2..10, 3)"));

	let compact = schema(LayoutKind::Compact, &[("name", "String<4>")]);
	let compact_source = SchemaVersion {
		version: 0,
		schema_sha256: compact.sha256(),
		schema: compact,
		process: None,
		process_sha256: None,
		transition: None,
	};
	let generated = manual_transition_source(
		&identity,
		MigrationVersionType::U8,
		&compact_source,
		1,
		&destination,
	)
	.unwrap_or_else(|error| panic!("manual transition: {error:?}"));

	assert!(generated.contains("Some(11)"));
}

#[test]
fn transition_hash_matches_rusts_cross_platform_line_ending_normalization() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
	let source = temp.path().join("transition.rs");
	std::fs::write(&source, "fn migrate() {\n\tlet value = 1;\n}\n")
		.unwrap_or_else(|error| panic!("write LF source: {error}"));
	let lf =
		hash_transition_file(&source).unwrap_or_else(|error| panic!("hash LF source: {error}"));

	std::fs::write(&source, "fn migrate() {\r\n\tlet value = 1;\r\n}\r\n")
		.unwrap_or_else(|error| panic!("write CRLF source: {error}"));
	let crlf =
		hash_transition_file(&source).unwrap_or_else(|error| panic!("hash CRLF source: {error}"));

	assert_eq!(lf, crlf);
}

#[test]
fn filesystem_boundaries_reject_missing_special_and_linked_paths() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
	let root = std::fs::canonicalize(temp.path())
		.unwrap_or_else(|error| panic!("canonicalize temp dir: {error}"));
	let missing = root.join("missing");
	assert!(matches!(
		read_bytes(&missing),
		Err(MigrationError::Read { .. })
	));
	assert!(matches!(
		hash_regular_file(&missing),
		Err(MigrationError::Read { .. })
	));
	assert!(matches!(
		hash_regular_file(&root),
		Err(MigrationError::InvalidHistory(_))
	));
	assert!(matches!(
		write_json_atomic(Path::new("/"), &serde_json::json!({})),
		Err(MigrationError::InvalidHistory(_))
	));

	let blocked = root.join("blocked");
	std::fs::write(&blocked, b"not a directory")
		.unwrap_or_else(|error| panic!("write blocked path: {error}"));
	assert!(matches!(
		acquire_migration_lock(&blocked),
		Err(MigrationError::Read { .. })
	));
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt as _;

		let readonly = root.join("readonly");
		std::fs::create_dir(&readonly)
			.unwrap_or_else(|error| panic!("create readonly directory: {error}"));
		std::fs::set_permissions(&readonly, std::fs::Permissions::from_mode(0o555))
			.unwrap_or_else(|error| panic!("protect readonly directory: {error}"));
		assert!(matches!(
			acquire_migration_lock(&readonly),
			Err(MigrationError::CreateDirectory { .. })
		));
		assert!(matches!(
			write_json_atomic(&readonly.join("nested/value.json"), &serde_json::json!({}),),
			Err(MigrationError::CreateDirectory { .. })
		));
		std::fs::set_permissions(&readonly, std::fs::Permissions::from_mode(0o755))
			.unwrap_or_else(|error| panic!("restore readonly directory: {error}"));
	}
	let lock_root = root.join("lock-root");
	std::fs::create_dir_all(lock_root.join("migrations/.lock"))
		.unwrap_or_else(|error| panic!("create directory lock: {error}"));
	assert!(matches!(
		acquire_migration_lock(&lock_root),
		Err(MigrationError::Lock { .. })
	));

	struct FailingSerialize;
	impl Serialize for FailingSerialize {
		fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
		where
			S: serde::Serializer,
		{
			Err(<S::Error as serde::ser::Error>::custom(
				"intentional failure",
			))
		}
	}
	assert!(matches!(
		write_json_atomic(&root.join("failing.json"), &FailingSerialize),
		Err(MigrationError::SerializeJson { .. })
	));
	assert!(matches!(
		write_atomic(&root.join("absent/target"), b"value"),
		Err(MigrationError::Write { .. })
	));
	let directory_target = root.join("directory-target");
	std::fs::create_dir(&directory_target)
		.unwrap_or_else(|error| panic!("create directory target: {error}"));
	assert!(matches!(
		write_atomic(&directory_target, b"value"),
		Err(MigrationError::Write { .. })
	));

	struct FailingIo;
	impl std::io::Write for FailingIo {
		fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
			Err(std::io::Error::other("intentional write failure"))
		}

		fn flush(&mut self) -> std::io::Result<()> {
			Ok(())
		}
	}
	impl std::io::Read for FailingIo {
		fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
			Err(std::io::Error::other("intentional read failure"))
		}
	}
	std::io::Write::flush(&mut FailingIo)
		.unwrap_or_else(|error| panic!("flush inert failing writer: {error}"));
	assert!(matches!(
		write_all(FailingIo, b"value", &root.join("logical")),
		Err(MigrationError::Write { .. })
	));
	assert!(matches!(
		hash_reader(&root.join("logical"), FailingIo),
		Err(MigrationError::Read { .. })
	));

	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt as _;
		use std::os::unix::fs::symlink;

		let target = root.join("target");
		let linked = root.join("linked");
		std::fs::write(&target, b"target")
			.unwrap_or_else(|error| panic!("write link target: {error}"));
		symlink(&target, &linked).unwrap_or_else(|error| panic!("create link: {error}"));
		assert!(matches!(
			ensure_safe_path(&linked),
			Err(MigrationError::UnsafePath { .. })
		));

		let unreadable = root.join("unreadable");
		std::fs::write(&unreadable, b"secret")
			.unwrap_or_else(|error| panic!("write unreadable file: {error}"));
		std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000))
			.unwrap_or_else(|error| panic!("protect unreadable file: {error}"));
		let result = hash_regular_file(&unreadable);
		std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o600))
			.unwrap_or_else(|error| panic!("restore unreadable file: {error}"));
		assert!(matches!(result, Err(MigrationError::Read { .. })));
	}
}

#[test]
fn draft_lifecycle_requires_creates_refreshes_and_removes_snapshots() {
	let fixture = migration_fixture();
	assert!(matches!(
		check_migrations(&fixture.root),
		Err(MigrationError::MissingSnapshot { .. })
	));

	let created = make_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("create migration history: {error}"));
	assert_eq!(created.created_contracts, ["account:1:01"]);
	let statuses = migration_status(&fixture.root)
		.unwrap_or_else(|error| panic!("read draft status: {error}"));
	assert_eq!(statuses.len(), 1);
	assert_eq!(statuses[0].current_version, 0);
	assert!(!statuses[0].published);
	assert!(!statuses[0].publication_pending);
	let metadata = idl_migration_metadata(&fixture.root)
		.unwrap_or_else(|error| panic!("read IDL metadata: {error}"))
		.expect("migration-aware project has IDL metadata");
	assert_eq!(metadata.version_type, MigrationVersionType::U8);
	assert_eq!(metadata.current_versions.get("account:1:01"), Some(&0));

	let unchanged = make_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("refresh unchanged draft: {error}"));
	assert_eq!(unchanged.unchanged_contracts, ["account:1:01"]);
	write_state_source(&fixture, "value: u64, enabled: bool");
	let updated = make_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("replace draft version zero: {error}"));
	assert_eq!(updated.updated_drafts, ["account:1:01@0"]);
	check_migrations(&fixture.root).unwrap_or_else(|error| panic!("check replaced draft: {error}"));

	std::fs::write(
		fixture.root.join("src/lib.rs"),
		format!("use pina::*;\ndeclare_id!(\"{}\");\n", fixture.program_id),
	)
	.unwrap_or_else(|error| panic!("remove migratable account: {error}"));
	assert!(matches!(
		check_migrations(&fixture.root),
		Err(MigrationError::ContractRemoved { .. })
	));
	assert!(matches!(
		make_migrations(&fixture.root),
		Err(MigrationError::ContractRemoved { .. })
	));
}

#[test]
fn discovery_snapshots_accounts_instructions_events_and_processes() {
	let fixture = migration_fixture();
	std::fs::write(
		fixture.root.join("src/lib.rs"),
		include_str!("../../../../examples/migrations_program/src/lib.rs"),
	)
	.unwrap_or_else(|error| panic!("write complete migration source: {error}"));
	let project = Project::discover(&fixture.root)
		.unwrap_or_else(|error| panic!("discover complete fixture: {error}"));
	let current = scan_current_contracts(&project)
		.unwrap_or_else(|error| panic!("scan complete fixture: {error}"));
	assert_eq!(current.contracts.len(), 5);
	assert_eq!(
		current
			.contracts
			.iter()
			.filter(|contract| contract.identity.kind == ContractKind::Instruction)
			.count(),
		1
	);
	let instruction = current
		.contracts
		.iter()
		.find(|contract| contract.identity.kind == ContractKind::Instruction)
		.expect("instruction contract");
	assert_eq!(
		instruction
			.process
			.as_ref()
			.expect("process")
			.accounts
			.len(),
		7
	);

	let output = make_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("snapshot complete fixture: {error}"));
	assert_eq!(output.created_contracts.len(), 5);
}

#[test]
fn event_discovery_rejects_invalid_unresolved_and_duplicate_contracts() {
	let fixture = migration_fixture();
	let scan = |body: &str| {
		std::fs::write(
			fixture.root.join("src/lib.rs"),
			format!(
				"use pina::*;\ndeclare_id!(\"{}\");\n#[discriminator]\nenum EventKind {{ Value = \
				 1 }}\n{body}\n",
				fixture.program_id
			),
		)
		.unwrap_or_else(|error| panic!("write event source: {error}"));
		let project = Project::discover(&fixture.root)
			.unwrap_or_else(|error| panic!("discover event fixture: {error}"));
		scan_current_contracts(&project)
	};

	assert!(matches!(
		scan("#[event(migrations)] struct ValueEvent { value: u64 }"),
		Err(MigrationError::Parse(_))
	));
	assert!(matches!(
		scan(
			"#[event(discriminator = Missing::Value, migrations)] struct ValueEvent { value: u64 }"
		),
		Err(MigrationError::InvalidHistory(_))
	));
	assert!(matches!(
		scan(
			"#[event(discriminator = EventKind::Value, migrations)] struct First { value: u64 \
			 }\n#[event(discriminator = EventKind::Value, migrations)] struct Second { value: u64 \
			 }"
		),
		Err(MigrationError::DuplicateIdentity { .. })
	));
}

#[test]
fn lifecycle_rejects_drift_configuration_changes_and_invalid_documents() {
	let fixture = migration_fixture();
	make_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("create baseline history: {error}"));
	write_state_source(&fixture, "value: u16");
	assert!(matches!(
		check_migrations(&fixture.root),
		Err(MigrationError::SchemaDrift { .. })
	));

	let manifest_path = fixture.root.join(MANIFEST_PATH);
	let mut manifest = load_manifest(&manifest_path)
		.unwrap_or_else(|error| panic!("read baseline manifest: {error}"))
		.expect("baseline manifest");
	manifest.program_id = "11111111111111111111111111111111".to_owned();
	write_json_atomic(&manifest_path, &manifest)
		.unwrap_or_else(|error| panic!("write mismatched program: {error}"));
	assert!(matches!(
		check_migrations(&fixture.root),
		Err(MigrationError::ProgramIdentityChanged { .. })
	));

	manifest.program_id = fixture.program_id.to_owned();
	write_json_atomic(&manifest_path, &manifest)
		.unwrap_or_else(|error| panic!("restore program identity: {error}"));
	std::fs::write(
		fixture.root.join("pina.toml"),
		"[project]\nprogram = \".\"\n[migrations]\nversion-type = \"u16\"\n",
	)
	.unwrap_or_else(|error| panic!("write changed version type: {error}"));
	assert!(matches!(
		check_migrations(&fixture.root),
		Err(MigrationError::VersionTypeChanged { .. })
	));

	std::fs::write(&manifest_path, b"not json")
		.unwrap_or_else(|error| panic!("corrupt manifest: {error}"));
	assert!(matches!(
		load_manifest(&manifest_path),
		Err(MigrationError::InvalidDocument { .. })
	));
	let publications = fixture.root.join(PUBLICATIONS_PATH);
	std::fs::write(&publications, b"not json")
		.unwrap_or_else(|error| panic!("corrupt ledger: {error}"));
	assert!(matches!(
		load_publication_ledger(&publications),
		Err(MigrationError::InvalidDocument { .. })
	));
}

#[test]
fn lifecycle_rejects_a_new_contract_without_a_snapshot() {
	let fixture = migration_fixture();
	make_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("create baseline history: {error}"));
	std::fs::write(
		fixture.root.join("src/lib.rs"),
		format!(
			"use pina::*;\ndeclare_id!(\"{}\");\n#[discriminator]\nenum Kind {{ State = 1, Other \
			 = 2 }}\n#[account(discriminator = Kind::State, migrations)]\nstruct State {{ value: \
			 u64 }}\n#[account(discriminator = Kind::Other, migrations)]\nstruct Other {{ value: \
			 u8 }}\n",
			fixture.program_id
		),
	)
	.unwrap_or_else(|error| panic!("add second contract: {error}"));
	assert!(matches!(
		check_migrations(&fixture.root),
		Err(MigrationError::MissingSnapshot { name, .. }) if name == "Other"
	));
}

#[test]
fn internal_contract_helpers_fail_closed_on_collisions_and_missing_values() {
	let identity = ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap();
	let contract = CurrentContract {
		identity,
		rust_name: "State".to_owned(),
		schema: schema(LayoutKind::Fixed, &[("value", "u64")]),
		process: None,
	};
	let mut contracts = BTreeMap::new();
	insert_current(&mut contracts, contract.clone())
		.unwrap_or_else(|error| panic!("insert first contract: {error}"));
	assert!(matches!(
		insert_current(&mut contracts, contract),
		Err(MigrationError::DuplicateIdentity { .. })
	));
	assert!(
		resolve_discriminator(
			&std::collections::HashMap::new(),
			"Kind",
			"State",
			"account"
		)
		.is_err()
	);

	let instruction = InstructionIr {
		name: "update".to_owned(),
		rust_name: "UpdateInstruction".to_owned(),
		accounts: vec![
			crate::ir::InstructionAccountIr {
				name: "program".to_owned(),
				is_writable: false,
				is_signer: false,
				is_optional: false,
				default_value: Some(DefaultValueIr::ProgramId("program".to_owned())),
				is_pda: false,
				pda_name: None,
				constraints: vec![],
				docs: vec![],
			},
			crate::ir::InstructionAccountIr {
				name: "authority".to_owned(),
				is_writable: false,
				is_signer: false,
				is_optional: false,
				default_value: Some(DefaultValueIr::PublicKey("address".to_owned())),
				is_pda: false,
				pda_name: None,
				constraints: vec![],
				docs: vec![],
			},
		],
		arguments: vec![],
		discriminator: DiscriminatorIr {
			value: 2,
			repr_size: 1,
		},
		docs: vec![],
	};
	let process = process_contract(&instruction);
	assert_eq!(
		process.accounts[0].default_value.as_deref(),
		Some("program:program")
	);
	assert_eq!(
		process.accounts[1].default_value.as_deref(),
		Some("publicKey:address")
	);
}

#[test]
fn non_migratable_projects_have_no_idl_migration_metadata() {
	let fixture = migration_fixture();
	std::fs::write(
		fixture.root.join("src/lib.rs"),
		format!("use pina::*;\ndeclare_id!(\"{}\");\n", fixture.program_id),
	)
	.unwrap_or_else(|error| panic!("write ordinary source: {error}"));
	std::fs::remove_file(fixture.root.join(PUBLICATIONS_PATH))
		.unwrap_or_else(|error| panic!("remove empty ledger: {error}"));
	assert!(
		idl_migration_metadata(&fixture.root)
			.unwrap_or_else(|error| panic!("read ordinary metadata: {error}"))
			.is_none()
	);
}

#[test]
fn publication_records_exact_artifact_and_freezes_current_versions() {
	let fixture = publication_fixture();
	let digest: [u8; 32] = Sha256::digest(b"artifact").into();
	let pending = begin_publication(
		&fixture.root,
		"devnet",
		"https://api.devnet.solana.com",
		fixture.program_id,
		&fixture.artifact,
		digest,
	)
	.unwrap_or_else(|error| panic!("begin publication: {error}"))
	.expect("migration-aware fixture has a pending publication");
	assert_eq!(
		pending.versions.get("account:1:01").map(|c| c.version),
		Some(0)
	);
	let repeated = begin_publication(
		&fixture.root,
		"devnet",
		"https://api.devnet.solana.com",
		fixture.program_id,
		&fixture.artifact,
		digest,
	)
	.unwrap_or_else(|error| panic!("resume publication: {error}"));
	assert_eq!(repeated, Some(pending.clone()));
	let conflicting = begin_publication(
		&fixture.root,
		"testnet",
		"https://api.testnet.solana.com",
		fixture.program_id,
		&fixture.artifact,
		digest,
	);
	assert!(matches!(
		conflicting,
		Err(MigrationError::PublicationPending { .. })
	));

	let rejected = record_publication(
		&fixture.root,
		"devnet",
		"https://api.devnet.solana.com",
		fixture.program_id,
		&fixture.artifact,
		[9; 32],
	);
	assert!(matches!(
		rejected,
		Err(MigrationError::PublicationArtifactChanged { .. })
	));
	let ledger = load_publication_ledger(&fixture.root.join(PUBLICATIONS_PATH))
		.unwrap_or_else(|error| panic!("reload pending publication: {error}"));
	assert!(ledger.pending.is_some());
	assert!(ledger.version_is_frozen("account:1:01", 0));
	assert!(!ledger.ever_published("account:1:01", 0));
	let statuses = migration_status(&fixture.root)
		.unwrap_or_else(|error| panic!("read pending status: {error}"));
	assert!(statuses[0].publication_pending);
	std::fs::write(
		fixture.root.join("src/lib.rs"),
		format!(
			"use pina::*;\ndeclare_id!(\"{}\");\n#[discriminator]\nenum Kind {{ State = 1 \
			 }}\n#[account(discriminator = Kind::State, migrations)]\nstruct State {{ value: u64, \
			 enabled: bool }}\n",
			fixture.program_id
		),
	)
	.unwrap_or_else(|error| panic!("change source during pending deployment: {error}"));
	let advanced = make_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("advance frozen pending version: {error}"));
	assert_eq!(advanced.advanced_versions, ["account:1:01@1"]);
	let resumed = begin_publication(
		&fixture.root,
		"devnet",
		"https://api.devnet.solana.com",
		fixture.program_id,
		&fixture.artifact,
		digest,
	)
	.unwrap_or_else(|error| panic!("resume exact pending publication: {error}"));
	assert_eq!(resumed, Some(pending));

	let receipt = record_publication(
		&fixture.root,
		"devnet",
		"https://api.devnet.solana.com",
		fixture.program_id,
		&fixture.artifact,
		digest,
	)
	.unwrap_or_else(|error| panic!("record publication: {error}"))
	.expect("migration-aware fixture has a receipt");

	assert_eq!(receipt.sequence, 0);
	assert_eq!(receipt.executable_sha256, hex_digest(digest));
	assert_eq!(
		receipt.versions.get("account:1:01").map(|c| c.version),
		Some(0)
	);
	let ledger = load_publication_ledger(&fixture.root.join(PUBLICATIONS_PATH))
		.unwrap_or_else(|error| panic!("reload publication: {error}"));
	assert!(ledger.pending.is_none());
	assert!(ledger.ever_published("account:1:01", 0));
	assert_eq!(ledger.receipts.len(), 1);

	write_state_source(&fixture, "value: u64, enabled: bool, count: u16");
	let refreshed = make_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("replace unpublished version one: {error}"));
	assert_eq!(refreshed.updated_drafts, ["account:1:01@1"]);
	check_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("check refreshed version one: {error}"));
}

#[test]
fn publication_rejects_mismatched_programs_artifacts_and_attempts() {
	let digest: [u8; 32] = Sha256::digest(b"artifact").into();

	let fixture = publication_fixture();
	assert!(matches!(
		begin_publication(
			&fixture.root,
			"devnet",
			"https://api.devnet.solana.com",
			"11111111111111111111111111111111",
			&fixture.artifact,
			digest,
		),
		Err(MigrationError::PublicationProgramMismatch { .. })
	));
	assert!(matches!(
		begin_publication(
			&fixture.root,
			"devnet",
			"https://api.devnet.solana.com",
			fixture.program_id,
			&fixture.artifact,
			[7; 32],
		),
		Err(MigrationError::PublicationArtifactChanged { .. })
	));

	begin_publication(
		&fixture.root,
		"devnet",
		"https://api.devnet.solana.com",
		fixture.program_id,
		&fixture.artifact,
		digest,
	)
	.unwrap_or_else(|error| panic!("begin exact publication: {error}"));
	std::fs::write(&fixture.artifact, b"swapped artifact")
		.unwrap_or_else(|error| panic!("swap pending artifact: {error}"));
	assert!(matches!(
		begin_publication(
			&fixture.root,
			"devnet",
			"https://api.devnet.solana.com",
			fixture.program_id,
			&fixture.artifact,
			digest,
		),
		Err(MigrationError::PublicationArtifactChanged { .. })
	));
	std::fs::write(&fixture.artifact, b"artifact")
		.unwrap_or_else(|error| panic!("restore pending artifact: {error}"));
	assert!(matches!(
		record_publication(
			&fixture.root,
			"testnet",
			"https://api.testnet.solana.com",
			fixture.program_id,
			&fixture.artifact,
			digest,
		),
		Err(MigrationError::MissingPendingPublication)
	));

	let missing_manifest = publication_fixture();
	begin_publication(
		&missing_manifest.root,
		"devnet",
		"https://api.devnet.solana.com",
		missing_manifest.program_id,
		&missing_manifest.artifact,
		digest,
	)
	.unwrap_or_else(|error| panic!("begin publication before removal: {error}"));
	std::fs::remove_file(missing_manifest.root.join(MANIFEST_PATH))
		.unwrap_or_else(|error| panic!("remove pending manifest: {error}"));
	assert!(matches!(
		begin_publication(
			&missing_manifest.root,
			"devnet",
			"https://api.devnet.solana.com",
			missing_manifest.program_id,
			&missing_manifest.artifact,
			digest,
		),
		Err(MigrationError::InvalidHistory(_))
	));

	let record_without_manifest = publication_fixture();
	std::fs::remove_file(record_without_manifest.root.join(MANIFEST_PATH))
		.unwrap_or_else(|error| panic!("remove record manifest: {error}"));
	assert!(matches!(
		record_publication(
			&record_without_manifest.root,
			"devnet",
			"https://api.devnet.solana.com",
			record_without_manifest.program_id,
			&record_without_manifest.artifact,
			digest,
		),
		Err(MigrationError::InvalidHistory(_))
	));

	let wrong_record_program = publication_fixture();
	assert!(matches!(
		record_publication(
			&wrong_record_program.root,
			"devnet",
			"https://api.devnet.solana.com",
			"11111111111111111111111111111111",
			&wrong_record_program.artifact,
			digest,
		),
		Err(MigrationError::PublicationProgramMismatch { .. })
	));
}

#[test]
fn missing_ledger_with_advanced_versions_fails_closed() {
	let fixture = publication_fixture();
	publish_current(&fixture);

	// Advance to a draft v1 on top of the published v0.
	let advanced_schema = schema(LayoutKind::Fixed, &[("value", "u64"), ("enabled", "bool")]);
	let identity = ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap();
	let mut advanced =
		MigrationManifest::new(fixture.program_id.to_owned(), MigrationVersionType::U8);
	let base = schema(LayoutKind::Fixed, &[("value", "u64")]);
	let transition = Transition {
		from: 0,
		to: 1,
		mode: TransitionMode::Automatic,
		renames: Vec::new(),
		source_schema_sha256: base.sha256(),
		destination_schema_sha256: advanced_schema.sha256(),
		source_process_sha256: None,
		destination_process_sha256: None,
		process: None,
		implementation_sha256: Some("e".repeat(64)),
	};
	advanced.contracts.insert(
		identity.key(),
		ContractHistory {
			identity,
			rust_name: "State".to_owned(),
			versions: vec![
				SchemaVersion {
					version: 0,
					schema_sha256: base.sha256(),
					schema: base,
					process: None,
					process_sha256: None,
					transition: None,
				},
				SchemaVersion {
					version: 1,
					schema_sha256: advanced_schema.sha256(),
					schema: advanced_schema,
					process: None,
					process_sha256: None,
					transition: Some(transition),
				},
			],
		},
	);
	advanced
		.validate()
		.unwrap_or_else(|error| panic!("advanced manifest must be internally valid: {error}"));
	std::fs::write(
		fixture.root.join(MANIFEST_PATH),
		serde_json::to_vec_pretty(&advanced)
			.unwrap_or_else(|error| panic!("serialize manifest: {error}")),
	)
	.unwrap_or_else(|error| panic!("write manifest: {error}"));

	// Losing the ledger must fail closed instead of unfreezing history:
	// without this check `make` would rewrite published v1 in place.
	std::fs::remove_file(fixture.root.join(PUBLICATIONS_PATH))
		.unwrap_or_else(|error| panic!("remove ledger: {error}"));
	let rejection = check_migrations(&fixture.root)
		.expect_err("a missing ledger with advanced versions must fail closed");
	assert!(
		format!("{rejection:?}").contains("publication ledger is missing"),
		"unexpected rejection: {rejection:?}"
	);
}

#[test]
fn reconcile_reports_and_abandons_pending_deployments() {
	let fixture = publication_fixture();
	let digest: [u8; 32] = Sha256::digest(
		std::fs::read(&fixture.artifact)
			.unwrap_or_else(|error| panic!("read fixture artifact: {error}")),
	)
	.into();
	begin_publication(
		&fixture.root,
		"devnet",
		"https://api.devnet.solana.com",
		fixture.program_id,
		&fixture.artifact,
		digest,
	)
	.unwrap_or_else(|error| panic!("begin publication: {error}"));

	// Inspection reports the exact deployment that must be resumed and
	// keeps the pending record.
	let report = reconcile_publication(&fixture.root, false)
		.unwrap_or_else(|error| panic!("inspect pending: {error:?}"));
	assert!(!report.no_pending && !report.abandoned);
	assert_eq!(report.cluster.as_deref(), Some("devnet"));
	assert_eq!(
		report.rpc_url.as_deref(),
		Some("https://api.devnet.solana.com")
	);
	assert_eq!(report.program_id.as_deref(), Some(fixture.program_id));
	assert!(report.executable_sha256.is_some());
	let ledger = load_publication_ledger(&fixture.root.join(PUBLICATIONS_PATH))
		.unwrap_or_else(|error| panic!("load ledger: {error:?}"));
	assert!(ledger.pending.is_some());

	// Abandonment converts the pending record into a receipt that still
	// freezes the pinned versions.
	let abandoned = reconcile_publication(&fixture.root, true)
		.unwrap_or_else(|error| panic!("abandon pending: {error:?}"));
	assert!(abandoned.abandoned);
	let ledger = load_publication_ledger(&fixture.root.join(PUBLICATIONS_PATH))
		.unwrap_or_else(|error| panic!("load ledger: {error:?}"));
	assert!(ledger.pending.is_none());
	assert_eq!(ledger.receipts.len(), 1);
	assert!(ledger.receipts[0].abandoned);
	assert!(ledger.version_is_frozen("account:1:01", 0));

	// A different deployment is no longer blocked by the pending record.
	let pending = begin_publication(
		&fixture.root,
		"testnet",
		"https://api.testnet.solana.com",
		fixture.program_id,
		&fixture.artifact,
		digest,
	)
	.unwrap_or_else(|error| panic!("new publication after abandon: {error:?}"))
	.expect("abandonment unblocks new deployments");
	assert_eq!(pending.cluster, "testnet");

	let settled = reconcile_publication(&fixture.root, false)
		.unwrap_or_else(|error| panic!("reconcile again: {error:?}"));
	assert!(!settled.no_pending);

	reconcile_publication(&fixture.root, true)
		.unwrap_or_else(|error| panic!("cleanup abandon: {error:?}"));
	let empty = reconcile_publication(&fixture.root, false)
		.unwrap_or_else(|error| panic!("final reconcile: {error:?}"));
	assert!(empty.no_pending);
}

#[test]
fn receipts_pin_published_schema_hashes() {
	let fixture = publication_fixture();
	publish_current(&fixture);
	let ledger = load_publication_ledger(&fixture.root.join(PUBLICATIONS_PATH))
		.unwrap_or_else(|error| panic!("load ledger: {error:?}"));

	// Rewrite the published version zero with a different schema and
	// recompute every internal hash, so contract validation alone accepts
	// the document. Only the receipt's pinned history can detect it.
	let tampered_schema = schema(LayoutKind::Fixed, &[("value", "u32")]);
	let mut tampered =
		MigrationManifest::new(fixture.program_id.to_owned(), MigrationVersionType::U8);
	let identity = ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap();
	tampered.contracts.insert(
		identity.key(),
		ContractHistory {
			identity,
			rust_name: "State".to_owned(),
			versions: vec![SchemaVersion {
				version: 0,
				schema_sha256: tampered_schema.sha256(),
				schema: tampered_schema,
				process: None,
				process_sha256: None,
				transition: None,
			}],
		},
	);
	tampered
		.validate()
		.unwrap_or_else(|error| panic!("coherent tamper must pass manifest validation: {error}"));

	// A legacy receipt upgraded from an older ledger format carries no
	// pins and still accepts the rewrite.
	let mut legacy = ledger.clone();
	for receipt in &mut legacy.receipts {
		for published in receipt.versions.values_mut() {
			published.history.clear();
		}
	}
	assert!(
		validate_ledger_for_manifest(&legacy, &tampered).is_ok(),
		"legacy receipts cannot verify rewritten published schemas"
	);

	// The pinned receipt records the schema that actually shipped and
	// rejects the coherent rewrite.
	let rejection = validate_ledger_for_manifest(&ledger, &tampered)
		.expect_err("pinned receipts must reject rewritten published schemas");
	assert!(
		format!("{rejection:?}").contains("pinned schema"),
		"unexpected rejection: {rejection:?}"
	);

	// The untouched manifest still validates against its own receipt.
	let manifest = load_manifest(&fixture.root.join(MANIFEST_PATH))
		.unwrap_or_else(|error| panic!("load manifest: {error:?}"))
		.expect("fixture manifest");
	assert!(validate_ledger_for_manifest(&ledger, &manifest).is_ok());
}

#[test]
fn ledger_binding_rejects_wrong_unknown_and_future_contracts() {
	let fixture = publication_fixture();
	let digest: [u8; 32] = Sha256::digest(b"artifact").into();
	begin_publication(
		&fixture.root,
		"devnet",
		"https://api.devnet.solana.com",
		fixture.program_id,
		&fixture.artifact,
		digest,
	)
	.unwrap_or_else(|error| panic!("begin publication: {error}"));
	let pending_ledger = load_publication_ledger(&fixture.root.join(PUBLICATIONS_PATH))
		.unwrap_or_else(|error| panic!("read pending ledger: {error}"));
	let manifest = load_manifest(&fixture.root.join(MANIFEST_PATH))
		.unwrap_or_else(|error| panic!("read manifest: {error}"))
		.expect("fixture manifest");

	let mut wrong_program = pending_ledger.clone();
	wrong_program.pending.as_mut().expect("pending").program_id =
		"11111111111111111111111111111111".to_owned();
	assert!(validate_ledger_for_manifest(&wrong_program, &manifest).is_err());

	let mut unknown = pending_ledger.clone();
	unknown.pending.as_mut().expect("pending").versions =
		BTreeMap::from([("account:1:ff".to_owned(), PublishedContract::legacy(0))]);
	assert!(validate_ledger_for_manifest(&unknown, &manifest).is_err());

	let mut future = pending_ledger.clone();
	future
		.pending
		.as_mut()
		.expect("pending")
		.versions
		.insert("account:1:01".to_owned(), PublishedContract::legacy(1));
	assert!(validate_ledger_for_manifest(&future, &manifest).is_err());

	record_publication(
		&fixture.root,
		"devnet",
		"https://api.devnet.solana.com",
		fixture.program_id,
		&fixture.artifact,
		digest,
	)
	.unwrap_or_else(|error| panic!("record publication: {error}"));
	let receipt_ledger = load_publication_ledger(&fixture.root.join(PUBLICATIONS_PATH))
		.unwrap_or_else(|error| panic!("read receipt ledger: {error}"));

	let mut wrong_program = receipt_ledger.clone();
	wrong_program.receipts[0].program_id = "11111111111111111111111111111111".to_owned();
	assert!(validate_ledger_for_manifest(&wrong_program, &manifest).is_err());

	let mut unknown = receipt_ledger.clone();
	unknown.receipts[0].versions =
		BTreeMap::from([("account:1:ff".to_owned(), PublishedContract::legacy(0))]);
	assert!(validate_ledger_for_manifest(&unknown, &manifest).is_err());

	let mut future = receipt_ledger;
	future.receipts[0]
		.versions
		.insert("account:1:01".to_owned(), PublishedContract::legacy(1));
	assert!(validate_ledger_for_manifest(&future, &manifest).is_err());
}

#[test]
fn transition_files_fail_closed_before_and_after_publication() {
	let fixture = publication_fixture();
	publish_current(&fixture);
	write_state_source(&fixture, "value: u32");
	let generated = make_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("generate manual transition: {error}"));
	let path = generated.manual_transitions[0].clone();
	assert!(matches!(
		check_migrations(&fixture.root),
		Err(MigrationError::ManualTransitionIncomplete { .. })
	));

	std::fs::write(&path, "pub(crate) fn migrate(_: &mut [u8]) {}\n")
		.unwrap_or_else(|error| panic!("complete manual transition: {error}"));
	assert!(matches!(
		check_migrations(&fixture.root),
		Err(MigrationError::TransitionDrift { .. })
	));
	let refreshed = make_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("refresh manual hash: {error}"));
	assert_eq!(refreshed.updated_drafts, ["account:1:01@1"]);
	check_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("check completed transition: {error}"));
	let unchanged = make_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("keep matching draft hash: {error}"));
	assert_eq!(unchanged.unchanged_contracts, ["account:1:01"]);

	publish_current(&fixture);
	std::fs::write(&path, "pub(crate) fn migrate(_: &mut [u8]) { panic!() }\n")
		.unwrap_or_else(|error| panic!("tamper frozen transition: {error}"));
	assert!(matches!(
		check_migrations(&fixture.root),
		Err(MigrationError::FrozenImplementationChanged { .. })
	));
	assert!(matches!(
		make_migrations(&fixture.root),
		Err(MigrationError::FrozenImplementationChanged { .. })
	));

	let missing = publication_fixture();
	publish_current(&missing);
	write_state_source(&missing, "value: u64, enabled: bool");
	let generated = make_migrations(&missing.root)
		.unwrap_or_else(|error| panic!("generate automatic transition: {error}"));
	let manifest = load_manifest(&generated.manifest)
		.unwrap_or_else(|error| panic!("read generated manifest: {error}"))
		.expect("generated manifest");
	let transition = manifest.contracts["account:1:01"].versions[1]
		.transition
		.as_ref()
		.expect("generated transition");
	let path = transition_path(
		&Project::discover(&missing.root).expect("discover fixture"),
		&manifest.contracts["account:1:01"].identity,
		transition.from,
		transition.to,
	);
	std::fs::write(&path, [0xff])
		.unwrap_or_else(|error| panic!("write invalid transition text: {error}"));
	assert!(matches!(
		check_migrations(&missing.root),
		Err(MigrationError::Read { .. })
	));
	std::fs::remove_file(&path).unwrap_or_else(|error| panic!("remove transition: {error}"));
	assert!(matches!(
		check_migrations(&missing.root),
		Err(MigrationError::MissingTransition { .. })
	));
	assert!(matches!(
		make_migrations(&missing.root),
		Err(MigrationError::MissingTransition { .. })
	));
	assert!(matches!(
		hash_transition_file(&path),
		Err(MigrationError::Read { .. })
	));

	let mut incomplete = manifest.contracts["account:1:01"].clone();
	incomplete.versions[1].transition = None;
	assert!(matches!(
		verify_transition_files(
			&Project::discover(&missing.root).expect("discover fixture"),
			&PublicationLedger::default(),
			"account:1:01",
			&incomplete,
		),
		Err(MigrationError::InvalidHistory(_))
	));
}

struct PublicationFixture {
	_temp: TempDir,
	root: PathBuf,
	artifact: PathBuf,
	program_id: &'static str,
}

fn migration_fixture() -> PublicationFixture {
	const PROGRAM_ID: &str = "GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS";
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp fixture: {error}"));
	let root = std::fs::canonicalize(temp.path())
		.unwrap_or_else(|error| panic!("canonical fixture: {error}"));
	std::fs::create_dir_all(root.join("src"))
		.unwrap_or_else(|error| panic!("create source: {error}"));
	std::fs::create_dir_all(root.join("migrations"))
		.unwrap_or_else(|error| panic!("create migrations: {error}"));
	std::fs::write(
		root.join("Cargo.toml"),
		"[package]\nname = \"publication_fixture\"\nversion = \"0.0.0\"\nedition = \
		 \"2024\"\n[lib]\npath = \"src/lib.rs\"\n",
	)
	.unwrap_or_else(|error| panic!("write cargo manifest: {error}"));
	std::fs::write(
		root.join("src/lib.rs"),
		format!(
			"use pina::*;\ndeclare_id!(\"{PROGRAM_ID}\");\n#[discriminator]\nenum Kind {{ State = \
			 1 }}\n#[account(discriminator = Kind::State, migrations)]\nstruct State {{ value: \
			 u64 }}\n"
		),
	)
	.unwrap_or_else(|error| panic!("write source: {error}"));
	std::fs::write(
		root.join(PUBLICATIONS_PATH),
		serde_json::to_vec_pretty(&PublicationLedger::default())
			.unwrap_or_else(|error| panic!("serialize publications: {error}")),
	)
	.unwrap_or_else(|error| panic!("write publications: {error}"));
	let artifact = root.join("program.so");
	std::fs::write(&artifact, b"artifact")
		.unwrap_or_else(|error| panic!("write artifact: {error}"));

	PublicationFixture {
		_temp: temp,
		root,
		artifact,
		program_id: PROGRAM_ID,
	}
}

fn publication_fixture() -> PublicationFixture {
	published_fixture_with(&[("value", "u64")])
}

fn published_fixture_with(fields: &[(&str, &str)]) -> PublicationFixture {
	let fixture = migration_fixture();
	let identity = ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap();
	let schema = schema(LayoutKind::Fixed, fields);
	let mut manifest =
		MigrationManifest::new(fixture.program_id.to_owned(), MigrationVersionType::U8);
	manifest.contracts.insert(
		identity.key(),
		ContractHistory {
			identity,
			rust_name: "State".to_owned(),
			versions: vec![SchemaVersion {
				version: 0,
				schema_sha256: schema.sha256(),
				schema,
				process: None,
				process_sha256: None,
				transition: None,
			}],
		},
	);
	std::fs::write(
		fixture.root.join(MANIFEST_PATH),
		serde_json::to_vec_pretty(&manifest)
			.unwrap_or_else(|error| panic!("serialize manifest: {error:?}")),
	)
	.unwrap_or_else(|error| panic!("write manifest: {error:?}"));
	let source_fields = fields
		.iter()
		.map(|(name, rust_type)| format!("{name}: {rust_type}"))
		.collect::<Vec<_>>()
		.join(", ");
	write_state_source(&fixture, &source_fields);
	fixture
}

fn publish_current(fixture: &PublicationFixture) {
	let digest: [u8; 32] = Sha256::digest(
		std::fs::read(&fixture.artifact)
			.unwrap_or_else(|error| panic!("read fixture artifact: {error}")),
	)
	.into();
	begin_publication(
		&fixture.root,
		"devnet",
		"https://api.devnet.solana.com",
		fixture.program_id,
		&fixture.artifact,
		digest,
	)
	.unwrap_or_else(|error| panic!("begin fixture publication: {error}"));
	record_publication(
		&fixture.root,
		"devnet",
		"https://api.devnet.solana.com",
		fixture.program_id,
		&fixture.artifact,
		digest,
	)
	.unwrap_or_else(|error| panic!("record fixture publication: {error}"));
}

fn write_state_source(fixture: &PublicationFixture, fields: &str) {
	std::fs::write(
		fixture.root.join("src/lib.rs"),
		format!(
			"use pina::*;\ndeclare_id!(\"{}\");\n#[discriminator]\nenum Kind {{ State = 1 \
			 }}\n#[account(discriminator = Kind::State, migrations)]\nstruct State {{ {fields} \
			 }}\n",
			fixture.program_id
		),
	)
	.unwrap_or_else(|error| panic!("write State source: {error}"));
}

#[test]
fn ambiguous_renames_require_answers_and_generate_copies() {
	let fixture = publication_fixture();
	publish_current(&fixture);
	write_state_source(&fixture, "points: u64");

	// Non-interactive runs fail with the exact command that answers the
	// question instead of guessing between rename and remove+add.
	let rejection = make_migrations_with_answers(
		&fixture.root,
		&MigrationAnswers {
			no_interactive: true,
			..MigrationAnswers::default()
		},
	)
	.expect_err("ambiguous renames must require an answer");
	let rendered = format!("{rejection}");
	assert!(
		rendered.contains("was `value` renamed to `points`"),
		"{rendered}"
	);
	assert!(rendered.contains("--rename value:points"), "{rendered}");
	assert!(rendered.contains("--assume-removed value"), "{rendered}");

	// The answered rename preserves the field's bytes and records the
	// disambiguation in the manifest.
	let answers = MigrationAnswers::from_flags(&["value:points".to_owned()], &[], true)
		.unwrap_or_else(|error| panic!("answers: {error}"));
	let output = make_migrations_with_answers(&fixture.root, &answers)
		.unwrap_or_else(|error| panic!("make with rename: {error:?}"));
	assert_eq!(output.advanced_versions, ["account:1:01@1".to_owned()]);
	assert!(output.data_warnings.is_empty());
	let manifest = load_manifest(&fixture.root.join(MANIFEST_PATH))
		.unwrap_or_else(|error| panic!("load manifest: {error:?}"))
		.expect("fixture manifest");
	let transition = manifest.contracts["account:1:01"].versions[1]
		.transition
		.as_ref()
		.expect("advanced version carries a transition");
	assert_eq!(transition.mode, TransitionMode::Automatic);
	assert_eq!(
		transition.renames,
		vec![pina_abi::RenameMapping {
			from: "value".to_owned(),
			to: "points".to_owned()
		}]
	);
	let generated = std::fs::read_to_string(
		fixture
			.root
			.join("migrations/transitions/account_1_01/v0_to_v1.rs"),
	)
	.unwrap_or_else(|error| panic!("read transition: {error}"));
	assert!(
		generated.contains("data.copy_within(2..10, 2)"),
		"the rename must move the stored bytes: {generated}"
	);

	// Re-running make without answers stays stable: the recorded rename
	// already answers the question, so nothing re-asks or rewrites.
	let output = make_migrations_with_answers(&fixture.root, &MigrationAnswers::default())
		.unwrap_or_else(|error| panic!("re-run make: {error:?}"));
	assert_eq!(output.unchanged_contracts, ["account:1:01".to_owned()]);
}

#[test]
fn assumed_removals_drop_data_with_a_warning() {
	let fixture = publication_fixture();
	publish_current(&fixture);
	write_state_source(&fixture, "points: u64");

	let answers = MigrationAnswers::from_flags(&[], &["value".to_owned()], true)
		.unwrap_or_else(|error| panic!("answers: {error}"));
	let output = make_migrations_with_answers(&fixture.root, &answers)
		.unwrap_or_else(|error| panic!("make with removal: {error:?}"));
	assert_eq!(output.advanced_versions, ["account:1:01@1".to_owned()]);
	assert!(
		output
			.data_warnings
			.iter()
			.any(|warning| warning.contains("field `value`")),
		"data-dropping removals must warn: {:?}",
		output.data_warnings
	);
	let manifest = load_manifest(&fixture.root.join(MANIFEST_PATH))
		.unwrap_or_else(|error| panic!("load manifest: {error:?}"))
		.expect("fixture manifest");
	let transition = manifest.contracts["account:1:01"].versions[1]
		.transition
		.as_ref()
		.expect("advanced version carries a transition");
	assert_eq!(transition.mode, TransitionMode::Automatic);
	assert!(transition.renames.is_empty());
}

#[test]
fn answers_naming_fields_outside_the_diff_fail_closed() {
	// `value` is retained by the new source, so acknowledging its removal
	// or renaming it away would silently drop live data.
	let retained = published_fixture_with(&[("value", "u64"), ("extra", "u8")]);
	publish_current(&retained);
	write_state_source(&retained, "value: u64, extra: u8, points: u64");

	let removal = MigrationAnswers::from_flags(&[], &["value".to_owned()], true)
		.unwrap_or_else(|error| panic!("answers: {error}"));
	let rejection = make_migrations_with_answers(&retained.root, &removal)
		.expect_err("answers must match the diff they answer");
	let rendered = format!("{rejection}");
	assert!(
		rendered.contains("`--assume-removed value` names a field that was not removed"),
		"{rendered}"
	);

	let rename = MigrationAnswers::from_flags(&["value:points".to_owned()], &[], true)
		.unwrap_or_else(|error| panic!("answers: {error}"));
	let rejection = make_migrations_with_answers(&retained.root, &rename)
		.expect_err("answers must match the diff they answer");
	let rendered = format!("{rejection}");
	assert!(
		rendered.contains("`--rename value:points` names a field that was not removed"),
		"{rendered}"
	);

	// A field answered twice leaves the intent undefined.
	let ambiguous = published_fixture_with(&[("value", "u64")]);
	publish_current(&ambiguous);
	write_state_source(&ambiguous, "points: u64");
	let both =
		MigrationAnswers::from_flags(&["value:points".to_owned()], &["value".to_owned()], true)
			.unwrap_or_else(|error| panic!("answers: {error}"));
	let rejection = make_migrations_with_answers(&ambiguous.root, &both)
		.expect_err("one field cannot carry two answers");
	let rendered = format!("{rejection}");
	assert!(
		rendered.contains("answered with both `--rename` and `--assume-removed`"),
		"{rendered}"
	);

	// Renaming across types cannot preserve bytes and must stay manual.
	let retyped = published_fixture_with(&[("count", "u64")]);
	publish_current(&retyped);
	write_state_source(&retyped, "total: u32");
	let changed_type = MigrationAnswers::from_flags(&["count:total".to_owned()], &[], true)
		.unwrap_or_else(|error| panic!("answers: {error}"));
	let rejection = make_migrations_with_answers(&retyped.root, &changed_type)
		.expect_err("type-changing renames are manual");
	let rendered = format!("{rejection}");
	assert!(rendered.contains("changes the field type"), "{rendered}");
}

#[test]
fn rename_answers_select_their_own_target() {
	// `one` pairs with `two` by the first-candidate heuristic; the
	// developer's `one:three` answer must win instead of being stuck
	// behind the guess. The move stays direction-safe (everything shifts
	// right), so the transition remains automatic.
	let fixture = published_fixture_with(&[("one", "u64"), ("keep", "u32")]);
	publish_current(&fixture);
	write_state_source(&fixture, "two: u64, three: u64, keep: u32");

	let answers = MigrationAnswers::from_flags(&["one:three".to_owned()], &[], true)
		.unwrap_or_else(|error| panic!("answers: {error}"));
	let output = make_migrations_with_answers(&fixture.root, &answers)
		.unwrap_or_else(|error| panic!("make with overriding rename: {error:?}"));
	assert_eq!(output.advanced_versions, ["account:1:01@1".to_owned()]);
	let manifest = load_manifest(&fixture.root.join(MANIFEST_PATH))
		.unwrap_or_else(|error| panic!("load manifest: {error:?}"))
		.expect("fixture manifest");
	let transition = manifest.contracts["account:1:01"].versions[1]
		.transition
		.as_ref()
		.expect("advanced version carries a transition");
	assert_eq!(transition.mode, TransitionMode::Automatic);
	assert_eq!(
		transition.renames,
		vec![pina_abi::RenameMapping {
			from: "one".to_owned(),
			to: "three".to_owned()
		}]
	);
}

#[test]
fn growing_transitions_warn_about_rent_funding() {
	let fixture = publication_fixture();
	publish_current(&fixture);
	// v0 is 10 bytes; adding `enabled: bool` grows the account to 11.
	write_state_source(&fixture, "value: u64, enabled: bool");

	let output = make_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("make growth transition: {error:?}"));
	let warning = output
		.data_warnings
		.iter()
		.find(|warning| warning.contains("grows from 10 to 11 bytes"))
		.unwrap_or_else(|| {
			panic!(
				"growth must warn about rent funding: {:?}",
				output.data_warnings
			)
		});
	assert!(
		warning.contains("6960 lamports"),
		"the warning sizes the rent deficit: {warning}"
	);
	assert!(
		warning.contains("lamport budget"),
		"the warning names the budget to raise: {warning}"
	);
}

#[test]
fn field_type_changes_and_compact_changes_are_manual() {
	let old = schema(LayoutKind::Fixed, &[("count", "u64")]);
	let changed = schema(LayoutKind::Fixed, &[("count", "u32")]);
	let compact = schema(LayoutKind::Compact, &[("label", "String<8>")]);

	assert_eq!(transition_mode(&old, &changed), TransitionMode::Manual);
	assert_eq!(transition_mode(&old, &compact), TransitionMode::Manual);
}

struct ClosedTerminal;

impl std::io::Write for ClosedTerminal {
	fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
		Err(std::io::Error::other("terminal closed"))
	}

	fn flush(&mut self) -> std::io::Result<()> {
		Err(std::io::Error::other("terminal closed"))
	}
}

impl std::io::Read for ClosedTerminal {
	fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
		Err(std::io::Error::other("terminal closed"))
	}
}

impl std::io::BufRead for ClosedTerminal {
	fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
		Err(std::io::Error::other("terminal closed"))
	}

	fn consume(&mut self, _: usize) {}
}

#[test]
fn prompts_read_scripted_answers_and_print_the_question() {
	let rename_question = DisambiguationQuestion {
		contract: "account:1:01".to_owned(),
		from: "value".to_owned(),
		to: "points".to_owned(),
		rust_type: "u64".to_owned(),
	};
	for (answer, expected) in [
		(&b"y\n"[..], RenameAnswer::Rename),
		(b"yes\n", RenameAnswer::Rename),
		(b"rename\n", RenameAnswer::Rename),
		(b"n\n", RenameAnswer::Remove),
		(b"no\n", RenameAnswer::Remove),
	] {
		let mut reader: &[u8] = answer;
		let mut transcript = Vec::new();
		let mut prompts = PromptIo::new(&mut reader, &mut transcript, true);
		assert_eq!(prompts.prompt_rename(&rename_question), expected);
		assert!(
			String::from_utf8_lossy(&transcript)
				.contains("Field `value` was removed and `points` (same type `u64`)"),
			"the transcript must print the question: {transcript:?}"
		);
	}

	let removal_question = DisambiguationQuestion {
		to: String::new(),
		..rename_question.clone()
	};
	for (answer, expected) in [
		(&b"y\n"[..], RenameAnswer::Remove),
		(b"yes\n", RenameAnswer::Remove),
		(b"remove\n", RenameAnswer::Remove),
		(b"n\n", RenameAnswer::Abort),
		(b"abort\n", RenameAnswer::Abort),
	] {
		let mut reader: &[u8] = answer;
		let mut transcript = Vec::new();
		let mut prompts = PromptIo::new(&mut reader, &mut transcript, true);
		assert_eq!(prompts.prompt_removal(&removal_question), expected);
		assert!(
			String::from_utf8_lossy(&transcript)
				.contains("Field `value` (type `u64`) is removed for `account:1:01`"),
			"the transcript must print the removal: {transcript:?}"
		);
	}
}

#[test]
fn prompts_retry_twice_then_abort_on_invalid_answers() {
	let question = DisambiguationQuestion {
		contract: "account:1:01".to_owned(),
		from: "value".to_owned(),
		to: "points".to_owned(),
		rust_type: "u64".to_owned(),
	};
	let mut reader: &[u8] = b"maybe\nmaybe\nmaybe\n";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, true);
	assert_eq!(prompts.prompt_rename(&question), RenameAnswer::Abort);
	assert_eq!(
		String::from_utf8_lossy(&transcript)
			.matches("Answer `y` or `n`.")
			.count(),
		3,
		"each invalid answer must be retried exactly three times"
	);

	let mut reader: &[u8] = b"maybe\nmaybe\nmaybe\n";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, true);
	assert_eq!(prompts.prompt_removal(&question), RenameAnswer::Abort);
	assert_eq!(
		String::from_utf8_lossy(&transcript)
			.matches("Answer `y` or `n`.")
			.count(),
		3
	);

	// An exhausted retry budget degrades to the flag error instead of a
	// wrong guess, and an invalid answer followed by a valid one resolves.
	let mut reader: &[u8] = b"maybe\ny\n";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, true);
	assert_eq!(prompts.prompt_rename(&question), RenameAnswer::Rename);
}

#[test]
fn prompts_abort_when_the_terminal_breaks() {
	let question = DisambiguationQuestion {
		contract: "account:1:01".to_owned(),
		from: "value".to_owned(),
		to: "points".to_owned(),
		rust_type: "u64".to_owned(),
	};
	let mut closed_write = ClosedTerminal;
	let mut live_read: &[u8] = b"y\n";
	let mut prompts = PromptIo::new(&mut live_read, &mut closed_write, true);
	assert_eq!(prompts.prompt_rename(&question), RenameAnswer::Abort);

	let mut live_write = Vec::new();
	let mut closed_read = ClosedTerminal;
	let mut prompts = PromptIo::new(&mut closed_read, &mut live_write, true);
	assert_eq!(prompts.prompt_rename(&question), RenameAnswer::Abort);

	let mut closed_write = ClosedTerminal;
	let mut live_read: &[u8] = b"y\n";
	let mut prompts = PromptIo::new(&mut live_read, &mut closed_write, true);
	assert_eq!(prompts.prompt_removal(&question), RenameAnswer::Abort);

	let mut live_write = Vec::new();
	let mut closed_read = ClosedTerminal;
	let mut prompts = PromptIo::new(&mut closed_read, &mut live_write, true);
	assert_eq!(prompts.prompt_removal(&question), RenameAnswer::Abort);
}

#[test]
fn recorded_renames_contradicted_by_flags_fail_closed() {
	let source = schema(LayoutKind::Fixed, &[("value", "u64")]);
	let destination = schema(LayoutKind::Fixed, &[("points", "u64"), ("total", "u64")]);
	let answers = MigrationAnswers::from_flags(&["value:total".to_owned()], &[], true).unwrap();
	let mut warnings = Vec::new();
	let mut reader: &[u8] = b"";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, true);

	let rejection = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&[pina_abi::RenameMapping {
			from: "value".to_owned(),
			to: "points".to_owned(),
		}],
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.expect_err("a contradictory --rename must fail closed");
	assert!(
		format!("{rejection}").contains(
			"previously recorded the rename `value:points`; `--rename value:total` contradicts it"
		),
		"{rejection}"
	);
}

#[test]
fn renames_must_target_added_fields() {
	// `spare` is retained by the destination, so renaming the removed `count`
	// onto it would overwrite live bytes through the effective schema.
	let source = schema(LayoutKind::Fixed, &[("count", "u64"), ("spare", "u64")]);
	let destination = schema(LayoutKind::Fixed, &[("spare", "u64")]);
	let answers = MigrationAnswers::from_flags(&["count:spare".to_owned()], &[], true).unwrap();
	let mut warnings = Vec::new();
	let mut reader: &[u8] = b"";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, true);

	let rejection = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&[],
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.expect_err("renaming onto a retained field must fail closed");
	assert!(
		format!("{rejection}")
			.contains("`--rename count:spare` targets `spare`, which is not an added field"),
		"{rejection}"
	);
}

#[test]
fn recorded_renames_settle_draft_refreshes_without_asking_again() {
	let source = schema(LayoutKind::Fixed, &[("value", "u64")]);
	let destination = schema(LayoutKind::Fixed, &[("points", "u64")]);
	let answers = MigrationAnswers::default();
	let mut warnings = Vec::new();
	let mut reader: &[u8] = b"";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, true);

	let (renames, dropped) = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&[pina_abi::RenameMapping {
			from: "value".to_owned(),
			to: "points".to_owned(),
		}],
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.unwrap_or_else(|error| panic!("settled draft refresh: {error:?}"));
	assert_eq!(
		renames,
		vec![pina_abi::RenameMapping {
			from: "value".to_owned(),
			to: "points".to_owned(),
		}]
	);
	assert!(dropped.is_empty());
}

#[test]
fn interactive_rename_prompts_resolve_ambiguities() {
	let source = schema(LayoutKind::Fixed, &[("value", "u64")]);
	let destination = schema(LayoutKind::Fixed, &[("points", "u64")]);
	let answers = MigrationAnswers {
		no_interactive: false,
		..MigrationAnswers::default()
	};

	let mut reader: &[u8] = b"y\n";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, true);
	let mut warnings = Vec::new();
	let (renames, dropped) = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&[],
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.unwrap_or_else(|error| panic!("answered rename: {error:?}"));
	assert_eq!(
		renames,
		vec![pina_abi::RenameMapping {
			from: "value".to_owned(),
			to: "points".to_owned(),
		}]
	);
	assert!(dropped.is_empty());

	let mut reader: &[u8] = b"n\n";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, true);
	let mut warnings = Vec::new();
	let (renames, dropped) = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&[],
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.unwrap_or_else(|error| panic!("declined rename: {error:?}"));
	assert!(renames.is_empty());
	assert!(dropped.contains("value"));
	assert!(
		warnings
			.iter()
			.any(|warning| warning.contains("`points` starts zeroed")),
		"{warnings:?}"
	);

	// An aborted prompt degrades to the flag error with the question intact.
	let mut reader: &[u8] = b"maybe\nmaybe\nmaybe\n";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, true);
	let mut warnings = Vec::new();
	let rejection = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&[],
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.expect_err("an aborted prompt must ask again with flags");
	assert!(
		matches!(rejection, MigrationError::DisambiguationRequired { .. }),
		"{rejection:?}"
	);
}

#[test]
fn interactive_removal_prompts_collect_acknowledgements() {
	let source = schema(LayoutKind::Fixed, &[("value", "u64"), ("kept", "u8")]);
	let destination = schema(LayoutKind::Fixed, &[("kept", "u8")]);
	let answers = MigrationAnswers {
		no_interactive: false,
		..MigrationAnswers::default()
	};

	let mut reader: &[u8] = b"y\n";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, true);
	let mut warnings = Vec::new();
	let (renames, dropped) = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&[],
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.unwrap_or_else(|error| panic!("acknowledged removal: {error:?}"));
	assert!(renames.is_empty());
	assert!(dropped.contains("value"));
	assert!(
		warnings
			.iter()
			.any(|warning| warning.contains("field `value` (type `u64`) is removed")),
		"{warnings:?}"
	);

	let mut reader: &[u8] = b"maybe\nmaybe\nmaybe\n";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, true);
	let mut warnings = Vec::new();
	let rejection = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&[],
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.expect_err("an aborted removal must ask again with flags");
	let rendered = format!("{rejection}");
	assert!(
		rendered.contains("field `value` (type `u64`) is removed and its stored data is discarded"),
		"{rendered}"
	);
	assert!(
		rendered.contains("Answer with `--assume-removed value`"),
		"{rendered}"
	);
}

#[test]
fn dropped_fields_without_candidates_warn_without_questions() {
	let source = schema(LayoutKind::Fixed, &[("value", "u64"), ("kept", "u8")]);
	let destination = schema(LayoutKind::Fixed, &[("kept", "u8")]);
	let answers = MigrationAnswers::from_flags(&[], &["value".to_owned()], true).unwrap();
	let mut warnings = Vec::new();
	let mut reader: &[u8] = b"";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, true);

	let (renames, dropped) = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&[],
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.unwrap_or_else(|error| panic!("acknowledged unpaired removal: {error:?}"));
	assert!(renames.is_empty());
	assert!(dropped.contains("value"));
	assert!(
		warnings
			.iter()
			.any(|warning| warning.contains("field `value` (type `u64`) is removed")),
		"{warnings:?}"
	);
}

#[test]
fn effective_schema_rebuilds_reject_corrupted_manifest_schemas() {
	let mut version = SchemaVersion {
		version: 0,
		schema_sha256: "irrelevant".to_owned(),
		schema: schema(LayoutKind::Fixed, &[("value", "u64")]),
		process: None,
		process_sha256: None,
		transition: None,
	};
	// A hand-edited manifest can carry a field type the closed grammar
	// rejects; the rebuilt effective schema must fail instead of building a
	// broken transition from it.
	version.schema.fields[0].rust_type = "Widget".to_owned();

	let rejection = effective_source_schema(&version, &[], &BTreeSet::new())
		.expect_err("corrupted field types must fail the rebuild");
	assert!(
		format!("{rejection}").contains("produce an invalid schema"),
		"{rejection}"
	);
}

#[test]
fn draft_refreshes_ask_new_questions_through_the_recorded_history() {
	let fixture = published_fixture_with(&[("alpha", "u64"), ("beta", "u64")]);
	publish_current(&fixture);
	write_state_source(&fixture, "points: u64, beta: u64");
	let answers = MigrationAnswers::from_flags(&["alpha:points".to_owned()], &[], true).unwrap();
	let output = make_migrations_with_answers(&fixture.root, &answers)
		.unwrap_or_else(|error| panic!("record the rename on the draft: {error:?}"));
	assert_eq!(output.advanced_versions, ["account:1:01@1".to_owned()]);

	// The draft's destination now changes again: `beta` is removed while
	// `total` appears, and the recorded `alpha:points` rename no longer
	// answers the new question, so the refresh must ask instead of guessing.
	write_state_source(&fixture, "points: u64, total: u64");
	let rejection = make_migrations_with_answers(&fixture.root, &MigrationAnswers::default())
		.expect_err("an unanswered draft refresh must ask again");
	let rendered = format!("{rejection}");
	assert!(
		rendered.contains("was `beta` renamed to `total`"),
		"{rendered}"
	);
	assert!(
		!rendered.contains("was `alpha` renamed"),
		"the recorded rename must settle `alpha` instead of re-asking: {rendered}"
	);
}

#[test]
fn from_flags_rejects_one_sided_and_separatorless_renames() {
	for flag in [":points", "value:", ":"] {
		let error = MigrationAnswers::from_flags(&[flag.to_owned()], &[], true)
			.expect_err("a one-sided rename must be rejected");
		assert!(error.contains("must name both fields"), "{error}");
	}
	let error = MigrationAnswers::from_flags(&["value-points".to_owned()], &[], true)
		.expect_err("a rename without the separator must be rejected");
	assert!(
		error.contains("must be written as `--rename from:to`"),
		"{error}"
	);
}

#[test]
fn unpaired_removal_without_candidate_asks_with_flags_when_not_interactive() {
	// `value` is removed and nothing of its type is added, so there is no
	// rename to propose: the question must still demand an explicit answer.
	let source = schema(LayoutKind::Fixed, &[("value", "u64"), ("kept", "u8")]);
	let destination = schema(LayoutKind::Fixed, &[("kept", "u8")]);
	let answers = MigrationAnswers {
		no_interactive: true,
		..MigrationAnswers::default()
	};
	let mut warnings = Vec::new();
	let mut reader: &[u8] = b"";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, true);

	let rejection = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&[],
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.expect_err("an unanswered unpaired removal must ask again");
	let rendered = format!("{rejection}");
	assert!(
		rendered.contains("field `value` (type `u64`) is removed and its stored data is discarded"),
		"{rendered}"
	);
	assert!(
		rendered.contains("Answer with `--assume-removed value`"),
		"{rendered}"
	);
}

#[test]
fn one_added_field_cannot_receive_two_renames() {
	let source = schema(LayoutKind::Fixed, &[("alpha", "u64"), ("beta", "u64")]);
	let destination = schema(LayoutKind::Fixed, &[("points", "u64")]);
	let answers = MigrationAnswers::from_flags(
		&["alpha:points".to_owned(), "beta:points".to_owned()],
		&[],
		true,
	)
	.unwrap();
	let mut warnings = Vec::new();
	let mut reader: &[u8] = b"";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, true);

	let rejection = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&[],
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.expect_err("two renames onto one field must fail closed");
	assert!(
		format!("{rejection}").contains("two renames target `points`"),
		"{rejection}"
	);
}

#[test]
fn unclaimed_candidates_pair_once_across_several_removals() {
	// Two same-typed removals and two additions: each removal pairs with its
	// own candidate instead of both proposing the same field.
	let source = schema(
		LayoutKind::Fixed,
		&[("alpha", "u64"), ("beta", "u64"), ("kept", "u8")],
	);
	let destination = schema(
		LayoutKind::Fixed,
		&[("kept", "u8"), ("points", "u64"), ("total", "u64")],
	);
	let answers = MigrationAnswers::default();
	let mut warnings = Vec::new();
	let mut reader: &[u8] = b"";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, true);

	let rejection = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&[],
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.expect_err("both removals still need answers");
	let rendered = format!("{rejection}");
	assert!(
		rendered.contains("was `alpha` renamed to `points`"),
		"{rendered}"
	);
	assert!(
		rendered.contains("was `beta` renamed to `total`"),
		"{rendered}"
	);
	assert_eq!(
		rendered.matches("renamed to `points`").count(),
		1,
		"`points` must be proposed exactly once: {rendered}"
	);
}

#[test]
fn growth_warnings_only_apply_to_account_contracts() {
	let identity = ContractIdentity::try_new(ContractKind::Instruction, 1, 0).unwrap();
	let source = SchemaVersion {
		version: 0,
		schema_sha256: "irrelevant".to_owned(),
		schema: schema(LayoutKind::Fixed, &[("value", "u64")]),
		process: None,
		process_sha256: None,
		transition: None,
	};
	let destination = schema(
		LayoutKind::Fixed,
		&[("value", "u64"), ("padding", "u64"), ("extra", "u64")],
	);
	let mut output = MakeMigrationsOutput::default();
	warn_about_account_growth(
		&identity,
		"Update",
		&source,
		&destination,
		MigrationVersionType::U8.bytes(),
		&mut output,
	);
	assert!(
		output.data_warnings.is_empty(),
		"instruction growth must not warn about rent"
	);
}

#[test]
fn compact_growth_warnings_estimate_rent_from_capacity() {
	let identity = ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap();
	let source = SchemaVersion {
		version: 0,
		schema_sha256: "irrelevant".to_owned(),
		schema: schema(LayoutKind::Fixed, &[("value", "u64")]),
		process: None,
		process_sha256: None,
		transition: None,
	};
	let destination = schema(
		LayoutKind::Compact,
		&[("label", "String<8>"), ("tags", "Vec<u16, 2>")],
	);
	let mut output = MakeMigrationsOutput::default();
	warn_about_account_growth(
		&identity,
		"State",
		&source,
		&destination,
		MigrationVersionType::U8.bytes(),
		&mut output,
	);
	let from_total = 2 + source.schema.maximum_payload_size().expect("fixed size");
	let to_total = 2 + destination.maximum_payload_size().expect("compact maximum");
	assert_eq!(
		(from_total, to_total),
		(10, 17),
		"compact capacity arithmetic"
	);
	let rent = 6_960_u64 * u64::try_from(to_total - from_total).expect("small growth");
	let warning = &output.data_warnings[0];
	assert!(
		warning.contains("worst-case size grows from 10 to 17 bytes"),
		"{warning}"
	);
	assert!(warning.contains("(compact capacity)"), "{warning}");
	assert!(
		warning.contains(&format!("roughly {rent} lamports of rent exemption")),
		"{warning}"
	);
}

#[test]
fn manual_instruction_transitions_require_fixed_layouts() {
	let instruction = ContractIdentity::try_new(ContractKind::Instruction, 1, 0).unwrap();
	let fixed_source = SchemaVersion {
		version: 0,
		schema_sha256: "irrelevant".to_owned(),
		schema: schema(LayoutKind::Fixed, &[("value", "u64")]),
		process: None,
		process_sha256: None,
		transition: None,
	};
	let compact_destination = schema(LayoutKind::Compact, &[("label", "String<8>")]);
	let rejection = manual_transition_source(
		&instruction,
		MigrationVersionType::U8,
		&fixed_source,
		1,
		&compact_destination,
	)
	.expect_err("a compact instruction destination must fail closed");
	assert!(
		format!("{rejection}").contains("instruction and event histories must use fixed layouts"),
		"{rejection}"
	);

	let compact_source = SchemaVersion {
		version: 0,
		schema_sha256: "irrelevant".to_owned(),
		schema: schema(LayoutKind::Compact, &[("label", "String<8>")]),
		process: None,
		process_sha256: None,
		transition: None,
	};
	let fixed_destination = schema(LayoutKind::Fixed, &[("value", "u64")]);
	let rejection = manual_transition_source(
		&instruction,
		MigrationVersionType::U8,
		&compact_source,
		1,
		&fixed_destination,
	)
	.expect_err("a compact instruction source must fail closed");
	assert!(
		format!("{rejection}").contains("instruction and event histories must use fixed layouts"),
		"{rejection}"
	);
}

#[test]
fn scan_surfaces_the_parser_diagnostic_for_duplicate_account_identities() {
	let fixture = migration_fixture();
	let scan = |body: &str| {
		std::fs::write(fixture.root.join("src/lib.rs"), body.to_owned())
			.unwrap_or_else(|error| panic!("write scan source: {error:?}"));
		let project = Project::discover(&fixture.root)
			.unwrap_or_else(|error| panic!("discover scan fixture: {error:?}"));
		scan_current_contracts(&project)
	};

	// Colliding account or instruction identities are rejected by the program
	// parser before discovery assembles contracts.
	let duplicate_accounts = scan(&format!(
		"use pina::*;\ndeclare_id!(\"{}\");\n#[discriminator]\nenum Kind {{ State = 1 \
		 }}\n#[account(discriminator = Kind::State, migrations)]\nstruct First {{ value: u64 \
		 }}\n#[account(discriminator = Kind::State, migrations)]\nstruct Second {{ value: u64 }}\n",
		fixture.program_id
	));
	let rejection = match duplicate_accounts {
		Err(error) => error.to_string(),
		Ok(_) => panic!("colliding accounts must be rejected"),
	};
	assert!(
		rejection.contains("share discriminator value 1"),
		"colliding account identities must surface the parser diagnostic: {rejection}"
	);
}

#[test]
fn insert_current_rejects_one_contract_claiming_an_identity_twice() {
	let mut contracts = BTreeMap::new();
	let identity = ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap();
	let contract = |rust_name: &str| {
		CurrentContract {
			identity: identity.clone(),
			rust_name: rust_name.to_owned(),
			schema: schema(LayoutKind::Fixed, &[("value", "u64")]),
			process: None,
		}
	};

	insert_current(&mut contracts, contract("First"))
		.unwrap_or_else(|error| panic!("first claim: {error:?}"));
	let rejection = insert_current(&mut contracts, contract("Second"))
		.expect_err("a second claim on one identity must fail");
	assert!(
		matches!(rejection, MigrationError::DuplicateIdentity { ref identity } if identity == "account:1:01"),
		"{rejection:?}"
	);
}

#[test]
fn create_transition_propagates_manual_layout_errors() {
	let fixture = migration_fixture();
	let project = Project::discover(&fixture.root)
		.unwrap_or_else(|error| panic!("discover fixture: {error:?}"));
	let identity = ContractIdentity::try_new(ContractKind::Instruction, 1, 0).unwrap();
	let process = ProcessContract {
		accounts: vec![ProcessAccount {
			name: "authority".to_owned(),
			writable: false,
			signer: true,
			optional: false,
			default_value: None,
			pda: None,
			constraints: vec![],
		}],
	};
	let source = SchemaVersion {
		version: 0,
		schema_sha256: "source".to_owned(),
		schema: schema(LayoutKind::Fixed, &[("value", "u64")]),
		process: Some(process.clone()),
		process_sha256: None,
		transition: None,
	};
	let destination = schema(LayoutKind::Compact, &[("label", "String<8>")]);
	let mut output = MakeMigrationsOutput::default();

	let rejection = create_transition(
		&project,
		TransitionRequest {
			identity: &identity,
			rust_name: "Update",
			source: &source,
			renames: vec![],
			destination_version: 1,
			destination: &destination,
			destination_process: Some(&process),
			preserve_manual: false,
		},
		&mut output,
	)
	.expect_err("a compact instruction destination has no manual transition");
	assert!(
		format!("{rejection}").contains("instruction and event histories must use fixed layouts"),
		"{rejection}"
	);
}

#[test]
fn reconciliation_fails_when_the_manifest_disappears() {
	let fixture = publication_fixture();
	publish_current(&fixture);
	let digest: [u8; 32] = Sha256::digest(
		std::fs::read(&fixture.artifact)
			.unwrap_or_else(|error| panic!("read fixture artifact: {error:?}")),
	)
	.into();
	write_state_source(&fixture, "value: u64, padding: u64");
	make_migrations(&fixture.root).unwrap_or_else(|error| panic!("advance draft: {error:?}"));
	begin_publication(
		&fixture.root,
		"devnet",
		"https://api.devnet.solana.com",
		fixture.program_id,
		&fixture.artifact,
		digest,
	)
	.unwrap_or_else(|error| panic!("begin fixture publication: {error:?}"));

	std::fs::remove_file(fixture.root.join(MANIFEST_PATH))
		.unwrap_or_else(|error| panic!("remove manifest: {error:?}"));
	let rejection = reconcile_publication(&fixture.root, false)
		.expect_err("a missing manifest cannot be reconciled");
	assert!(
		format!("{rejection}").contains("migration manifest disappeared during reconciliation"),
		"{rejection}"
	);
}

#[test]
fn published_contracts_must_carry_versions_and_matching_pins() {
	let identity = ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap();
	let mut manifest = MigrationManifest::new("program".to_owned(), MigrationVersionType::U8);
	manifest.contracts.insert(
		identity.key(),
		ContractHistory {
			identity: identity.clone(),
			rust_name: "State".to_owned(),
			versions: vec![],
		},
	);
	let rejection = validate_published_contract(
		"receipt",
		&identity.key(),
		&PublishedContract::legacy(0),
		&manifest,
	)
	.expect_err("a versionless history cannot back a publication");
	assert!(
		format!("{rejection}").contains("has no versions"),
		"{rejection}"
	);

	// The receipt pinned a different transition implementation than the
	// manifest now records, so the publication cannot be trusted.
	let pinned_schema = schema(LayoutKind::Fixed, &[("value", "u64")]);
	let pinned_schema_sha256 = pinned_schema.sha256();
	manifest.contracts.insert(
		identity.key(),
		ContractHistory {
			identity: identity.clone(),
			rust_name: "State".to_owned(),
			versions: vec![SchemaVersion {
				version: 0,
				schema_sha256: pinned_schema.sha256(),
				schema: pinned_schema,
				process: None,
				process_sha256: None,
				transition: Some(Transition {
					from: 0,
					to: 0,
					mode: TransitionMode::Automatic,
					renames: vec![],
					source_schema_sha256: "source".to_owned(),
					destination_schema_sha256: "destination".to_owned(),
					source_process_sha256: None,
					destination_process_sha256: None,
					process: None,
					implementation_sha256: Some("manifest-implementation".to_owned()),
				}),
			}],
		},
	);
	let published = PublishedContract {
		version: 0,
		history: vec![pina_abi::PublishedSchema {
			schema_sha256: pinned_schema_sha256,
			transition_sha256: Some("pinned-implementation".to_owned()),
		}],
	};
	let rejection = validate_published_contract("receipt", &identity.key(), &published, &manifest)
		.expect_err("mismatched transition pins must fail closed");
	assert!(
		format!("{rejection}").contains("pinned a different transition implementation"),
		"{rejection}"
	);
}

#[test]
fn persisted_answers_answer_makes_without_flags() {
	let fixture = publication_fixture();
	publish_current(&fixture);
	write_state_source(&fixture, "points: u64");
	std::fs::write(
		fixture.root.join("pina.toml"),
		"[migrations.answers]\nrename = [\"value:points\"]\n",
	)
	.unwrap_or_else(|error| panic!("write pina.toml: {error:?}"));
	let project =
		Project::discover(&fixture.root).unwrap_or_else(|error| panic!("discover: {error:?}"));

	let answers = MigrationAnswers::from_layers(&project.migration_answers, &[], &[], false)
		.unwrap_or_else(|error| panic!("layer answers: {error}"));
	let output = make_migrations_with_answers(&fixture.root, &answers)
		.unwrap_or_else(|error| panic!("make with persisted answers: {error:?}"));
	assert_eq!(output.advanced_versions, ["account:1:01@1".to_owned()]);
}

#[test]
fn flag_answers_override_persisted_answers_without_conflict() {
	let persisted = crate::project::MigrationsAnswersConfig {
		rename: vec!["value:points".to_owned()],
		assume_removed: vec![],
	};
	let answers = MigrationAnswers::from_layers(&persisted, &["value:total".to_owned()], &[], true)
		.unwrap_or_else(|error| panic!("layer overriding answers: {error}"));
	let source = schema(LayoutKind::Fixed, &[("value", "u64")]);
	let destination = schema(LayoutKind::Fixed, &[("total", "u64")]);
	let mut warnings = Vec::new();
	let mut reader: &[u8] = b"";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, true);

	let (renames, dropped) = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&[],
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.unwrap_or_else(|error| panic!("resolved override: {error:?}"));
	assert_eq!(
		renames,
		vec![pina_abi::RenameMapping {
			from: "value".to_owned(),
			to: "total".to_owned(),
		}]
	);
	assert!(dropped.is_empty());
}

#[test]
fn flag_answers_contradicting_persisted_answers_fail_closed() {
	let persisted = crate::project::MigrationsAnswersConfig {
		rename: vec!["value:points".to_owned()],
		assume_removed: vec![],
	};
	let removal_conflict =
		MigrationAnswers::from_layers(&persisted, &[], &["value".to_owned()], true)
			.expect_err("a removal against a persisted rename must fail closed");
	assert!(
		removal_conflict.contains("contradicts the persisted rename `value:points`"),
		"{removal_conflict}"
	);
}
