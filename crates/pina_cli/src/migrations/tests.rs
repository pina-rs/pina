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

/// Build one recorded rename mapping.
fn pin_rename(from: &str, to: &str) -> pina_abi::RenameMapping {
	pina_abi::RenameMapping {
		from: from.to_owned(),
		to: to.to_owned(),
	}
}

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
	// `authority` is dropped deliberately, which the developer acknowledges
	// before the proof will consider the change automatic.
	let intent = SourceIntent {
		dropped: BTreeSet::from(["authority".to_owned()]),
		..SourceIntent::default()
	};

	assert_eq!(
		transition_mode(&old, &intent, &new),
		TransitionMode::Automatic
	);
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

	assert_eq!(
		transition_mode(&old, &SourceIntent::default(), &reordered),
		TransitionMode::Manual
	);
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
		schema: source_schema,
		process: None,
		transition: None,
	};
	let destination = schema(LayoutKind::Fixed, &[("amount", "u16")]);

	let account_source = manual_transition_source(
		&account,
		MigrationVersionType::U8,
		&source,
		0,
		1,
		&destination,
		&[],
	)
	.unwrap_or_else(|error| panic!("manual account transition: {error:?}"));
	assert!(account_source.contains("fn migrate(data: &mut [u8]) {"));
	assert!(!account_source.contains("fn migrate(data: &mut [u8]) -> bool"));
	assert!(account_source.contains("conversion must be total"));

	let instruction_source = manual_transition_source(
		&instruction,
		MigrationVersionType::U8,
		&source,
		0,
		1,
		&destination,
		&[],
	)
	.unwrap_or_else(|error| panic!("manual transition: {error:?}"));

	assert!(instruction_source.contains("fn migrate(data: &mut [u8]) -> bool"));
}

#[test]
fn compact_account_transition_uses_checked_runtime_sizing() {
	let account = ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap();
	let source_schema = schema(LayoutKind::Compact, &[("name", "String<4>")]);
	let source = SchemaVersion {
		schema: source_schema,
		process: None,
		transition: None,
	};
	let destination = schema(
		LayoutKind::Compact,
		&[("name", "String<4>"), ("tags", "Vec<u16, 2>")],
	);

	let generated = manual_transition_source(
		&account,
		MigrationVersionType::U8,
		&source,
		0,
		1,
		&destination,
		&[],
	)
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
		}],
	};
	let source = SchemaVersion {
		schema: source_schema.clone(),
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
				source_version: 0,
				stale_ladder: &[(0, &source)],
				intent: SourceIntent::default(),
				destination_version: 1,
				destination: &source_schema,
				destination_process: Some(&escalated),
				preserve_manual: false,
			},
			&mut CreateMigrationsOutput::default(),
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
		schema: source_schema,
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
				source_version: 0,
				stale_ladder: &[(0, &account_source)],
				intent: SourceIntent::default(),
				destination_version: 1,
				destination: &destination,
				destination_process: None,
				preserve_manual: false,
			},
			&mut CreateMigrationsOutput::default(),
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
		create_migrations(&frozen.root),
		Err(MigrationError::CreateDirectory { .. })
	));

	let draft = publication_fixture();
	publish_current(&draft);
	write_state_source(&draft, "value: u64, enabled: bool");
	create_migrations(&draft.root)
		.unwrap_or_else(|error| panic!("create version-one draft: {error}"));
	let transitions = draft.root.join("migrations/transitions");
	std::fs::remove_dir_all(&transitions)
		.unwrap_or_else(|error| panic!("remove generated transitions: {error}"));
	std::fs::write(&transitions, b"blocked")
		.unwrap_or_else(|error| panic!("block draft transition directory: {error}"));
	write_state_source(&draft, "value: u64, enabled: bool, counter: u16");
	assert!(matches!(
		create_migrations(&draft.root),
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
	let mixed_intent = SourceIntent {
		dropped: BTreeSet::from(["removed".to_owned()]),
		..SourceIntent::default()
	};
	// One field moves right while another moves left, so no single copy order
	// is safe.
	assert!(automatic_move_plan(&mixed_source, &mixed_intent, &mixed_destination).is_none());

	let source_schema = schema(LayoutKind::Fixed, &[("value", "u64")]);
	let destination = schema(LayoutKind::Fixed, &[("prefix", "u8"), ("value", "u64")]);
	let plan = automatic_move_plan(&source_schema, &SourceIntent::default(), &destination)
		.expect("inserting a leading field keeps every field moving right");
	assert_eq!(plan.direction, MoveDirection::Backward);
	let identity = ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap();
	let generated = automatic_transition_source(&identity, MigrationVersionType::U8, 0, 1, &plan);
	assert!(generated.contains("copy_within(2..10, 3)"));

	let compact = schema(LayoutKind::Compact, &[("name", "String<4>")]);
	let compact_source = SchemaVersion {
		schema: compact,
		process: None,
		transition: None,
	};
	let generated = manual_transition_source(
		&identity,
		MigrationVersionType::U8,
		&compact_source,
		0,
		1,
		&destination,
		&[],
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

	let created = create_migrations(&fixture.root)
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

	let unchanged = create_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("refresh unchanged draft: {error}"));
	assert_eq!(unchanged.unchanged_contracts, ["account:1:01"]);
	write_state_source(&fixture, "value: u64, enabled: bool");
	let updated = create_migrations(&fixture.root)
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
		create_migrations(&fixture.root),
		Err(MigrationError::ContractRemoved { .. })
	));
}

#[test]
fn discovery_snapshots_accounts_instructions_events_and_processes() {
	let fixture = migration_fixture();
	std::fs::write(
		fixture.root.join("pina.toml"),
		"[project]\nprogram = \".\"\n\n[migrations]\nversion_type = \"u8\"\nauto = true\n",
	)
	.unwrap_or_else(|error| panic!("write auto policy: {error}"));
	std::fs::write(
		fixture.root.join("src/lib.rs"),
		include_str!("../../../../examples/migrations_program/src/lib.rs"),
	)
	.unwrap_or_else(|error| panic!("write complete migration source: {error}"));
	let project = Project::discover(&fixture.root)
		.unwrap_or_else(|error| panic!("discover complete fixture: {error}"));
	assert_eq!(project.migration_auto, MigrationAuto::all());
	let current = scan_current_contracts(&project, &project.migration_auto)
		.unwrap_or_else(|error| panic!("scan complete fixture: {error}"));
	assert_eq!(current.contracts.len(), 5);
	// The example's relay payload opts out explicitly; auto must not envelop it.
	assert!(
		current
			.opt_outs
			.iter()
			.any(|opt_out| opt_out.rust_name == "RelayInstruction"),
		"RelayInstruction must be reported as an explicit opt-out",
	);
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

	let output = create_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("snapshot complete fixture: {error}"));
	assert_eq!(output.created_contracts.len(), 5);
	assert_eq!(output.auto, ["accounts", "instructions", "events"]);
	assert!(matches!(
		output.build_script,
		Some(BuildScriptStatus::Created { .. })
	));
	let manifest = load_manifest(&fixture.root.join(MANIFEST_PATH))
		.unwrap_or_else(|error| panic!("read recorded manifest: {error}"))
		.expect("auto run writes a manifest");
	assert_eq!(manifest.auto, MigrationAuto::all());
	// A second run verifies the scaffold instead of rewriting it.
	let unchanged = create_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("refresh auto fixture: {error}"));
	assert!(matches!(
		unchanged.build_script,
		Some(BuildScriptStatus::Verified { .. })
	));
	check_migrations(&fixture.root).unwrap_or_else(|error| panic!("check auto fixture: {error}"));
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
		scan_current_contracts(&project, &MigrationAuto::none())
	};

	let first = scan("#[event(migrations)] struct ValueEvent { value: u64 }")
		.map(|program| format!("Ok({} contracts)", program.contracts.len()))
		.map_err(|error| format!("Err({error})"));
	let failure = first.expect_err("an event without a discriminator argument must fail");
	assert!(
		failure.contains("missing `discriminator` argument"),
		"{failure}"
	);
	// An unresolved discriminator enum now fails during the shared parse
	// stage, before the migration scan can classify it as invalid history.
	let unresolved = scan(
		"#[event(discriminator = Missing::Value, migrations)] struct ValueEvent { value: u64 }",
	)
	.map(|program| format!("Ok({} contracts)", program.contracts.len()))
	.map_err(|error| format!("Err({error})"));
	let failure = unresolved.expect_err("an unresolved event discriminator must fail");
	assert!(failure.contains("Missing"), "{failure}");
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
	create_migrations(&fixture.root)
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
		"[project]\nprogram = \".\"\n[migrations]\nversion_type = \"u16\"\n",
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
	create_migrations(&fixture.root)
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
				docs: vec![],
				constraints: vec![],
			},
			crate::ir::InstructionAccountIr {
				name: "authority".to_owned(),
				is_writable: false,
				is_signer: false,
				is_optional: false,
				default_value: Some(DefaultValueIr::PublicKey("address".to_owned())),
				is_pda: false,
				pda_name: None,
				docs: vec![],
				constraints: vec![],
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
	let advanced = create_migrations(&fixture.root)
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
	let refreshed = create_migrations(&fixture.root)
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
		mode: TransitionMode::Automatic,
		renames: Vec::new(),
		implementation_sha256: Some("e".repeat(64)),
	};
	advanced.contracts.insert(
		identity.key(),
		ContractHistory {
			identity,
			rust_name: "State".to_owned(),
			versions: vec![
				SchemaVersion {
					schema: base,
					process: None,
					transition: None,
				},
				SchemaVersion {
					schema: advanced_schema,
					process: None,
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
	// without this check `create` would rewrite published v1 in place.
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
				schema: tampered_schema,
				process: None,
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
	let generated = create_migrations(&fixture.root)
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
	let refreshed = create_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("refresh manual hash: {error}"));
	assert_eq!(refreshed.updated_drafts, ["account:1:01@1"]);
	check_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("check completed transition: {error}"));
	let unchanged = create_migrations(&fixture.root)
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
		create_migrations(&fixture.root),
		Err(MigrationError::FrozenImplementationChanged { .. })
	));

	let missing = publication_fixture();
	publish_current(&missing);
	write_state_source(&missing, "value: u64, enabled: bool");
	let generated = create_migrations(&missing.root)
		.unwrap_or_else(|error| panic!("generate automatic transition: {error}"));
	let manifest = load_manifest(&generated.manifest)
		.unwrap_or_else(|error| panic!("read generated manifest: {error}"))
		.expect("generated manifest");
	let transition = manifest.contracts["account:1:01"].versions[1]
		.transition
		.as_ref()
		.expect("generated transition");
	assert_eq!(transition.mode, TransitionMode::Automatic);
	// The transition sits on `versions[1]`, so it converts v0 into v1.
	let path = transition_path(
		&Project::discover(&missing.root).expect("discover fixture"),
		&manifest.contracts["account:1:01"].identity,
		0,
		1,
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
		create_migrations(&missing.root),
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
				schema,
				process: None,
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
	let rejection = create_migrations_with_answers(
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
	let output = create_migrations_with_answers(&fixture.root, &answers)
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
	let output = create_migrations_with_answers(&fixture.root, &MigrationAnswers::default())
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
	let output = create_migrations_with_answers(&fixture.root, &answers)
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
	let rejection = create_migrations_with_answers(&retained.root, &removal)
		.expect_err("answers must match the diff they answer");
	let rendered = format!("{rejection}");
	assert!(
		rendered.contains("`--assume-removed value` names a field that was not removed"),
		"{rendered}"
	);

	let rename = MigrationAnswers::from_flags(&["value:points".to_owned()], &[], true)
		.unwrap_or_else(|error| panic!("answers: {error}"));
	let rejection = create_migrations_with_answers(&retained.root, &rename)
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
	let rejection = create_migrations_with_answers(&ambiguous.root, &both)
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
	let rejection = create_migrations_with_answers(&retyped.root, &changed_type)
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
	let output = create_migrations_with_answers(&fixture.root, &answers)
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

	let output = create_migrations(&fixture.root)
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
	// The warning must quote the same remedy the on-chain
	// `MigrationLamportBudgetExceeded` documents, so the pre-deploy estimate
	// and the runtime failure name the same constant.
	assert!(
		warning.contains(super::remedy::LAMPORT_BUDGET_REMEDY),
		"the warning carries the shared remedy text: {warning}"
	);
	assert!(
		warning.contains("MigrationLamportBudgetExceeded"),
		"the warning names the on-chain error: {warning}"
	);
}

#[test]
fn oversized_growth_warns_about_the_runtime_realloc_cap() {
	let identity = ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap();
	let source = SchemaVersion {
		schema: schema(LayoutKind::Fixed, &[("value", "u64")]),
		process: None,
		transition: None,
	};
	// 2,000 u64 fields grow one transition by more than the runtime's 10 KiB
	// per-instruction realloc cap.
	let fields = (0..2_000)
		.map(|index| (format!("field_{index}"), "u64".to_owned()))
		.collect::<Vec<_>>();
	let fields = fields
		.iter()
		.map(|(name, ty)| (name.as_str(), ty.as_str()))
		.collect::<Vec<_>>();
	let destination = schema(LayoutKind::Fixed, &fields);
	let mut output = CreateMigrationsOutput::default();
	warn_about_account_growth(
		&identity,
		"State",
		(0, &source),
		&[(0, &source)],
		&destination,
		MigrationVersionType::U8.bytes(),
		&mut output,
	);

	let warning = output
		.data_warnings
		.iter()
		.find(|warning| warning.contains("MAX_PERMITTED_DATA_INCREASE"))
		.unwrap_or_else(|| {
			panic!(
				"growth beyond the runtime cap must warn: {:?}",
				output.data_warnings
			)
		});
	assert!(
		warning.contains(super::remedy::ACCOUNT_GROWTH_REMEDY),
		"the warning carries the shared remedy text: {warning}"
	);
	assert!(
		warning.contains("MigrationAccountGrowthExceeded"),
		"the warning names the on-chain error: {warning}"
	);
	assert!(
		warning.contains("in transition v0 to v1"),
		"one adjacent hop keeps the single-transition wording: {warning}"
	);
	// The rent estimate still prints: an oversized transition needs both the
	// budget and a rebalanced ladder.
	assert!(
		output
			.data_warnings
			.iter()
			.any(|warning| warning.contains("lamports of rent exemption")),
		"the rent warning must remain: {:?}",
		output.data_warnings
	);
}

/// A fixed schema of `count` trailing u64 fields, used to build individually
/// sub-limit hops whose cumulative ladder crosses the runtime cap.
fn growing_schema(count: usize) -> DataSchema {
	let fields = (0..count)
		.map(|index| (format!("field_{index}"), "u64".to_owned()))
		.collect::<Vec<_>>();
	let fields = fields
		.iter()
		.map(|(name, ty)| (name.as_str(), ty.as_str()))
		.collect::<Vec<_>>();
	schema(LayoutKind::Fixed, &fields)
}

/// Source text for [`growing_schema`] fields, prefixed with `value: u64`.
fn growing_fields(count: usize) -> String {
	let fields = (0..count)
		.map(|index| format!("field_{index}: u64"))
		.collect::<Vec<_>>();
	if fields.is_empty() {
		"value: u64".to_owned()
	} else {
		format!("value: u64, {}", fields.join(", "))
	}
}

/// Each adjacent hop grows 6,000 bytes, under the 10,240-byte runtime cap on
/// its own, but a v0 account walks two hops in one instruction and the
/// executor measures both against the size captured before the ladder. The
/// warning must use the cumulative worst case, not the adjacent hop.
#[test]
fn cumulative_ladder_growth_warns_about_the_runtime_cap() {
	let identity = ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap();
	let version = |fields: usize| {
		SchemaVersion {
			schema: growing_schema(fields),
			process: None,
			transition: None,
		}
	};
	let oldest = version(0);
	let adjacent = version(750);
	let destination = growing_schema(1_500);

	// The adjacent hop alone stays under the runtime cap.
	let mut adjacent_only = CreateMigrationsOutput::default();
	warn_about_account_growth(
		&identity,
		"State",
		(1, &adjacent),
		&[(1, &adjacent)],
		&destination,
		MigrationVersionType::U8.bytes(),
		&mut adjacent_only,
	);
	assert!(
		!adjacent_only
			.data_warnings
			.iter()
			.any(|warning| warning.contains("MAX_PERMITTED_DATA_INCREASE")),
		"one sub-limit hop must not warn: {:?}",
		adjacent_only.data_warnings
	);

	// A v0 account walks both hops, so the cumulative growth crosses the cap.
	let mut output = CreateMigrationsOutput::default();
	warn_about_account_growth(
		&identity,
		"State",
		(1, &adjacent),
		&[(0, &oldest), (1, &adjacent)],
		&destination,
		MigrationVersionType::U8.bytes(),
		&mut output,
	);
	let warning = output
		.data_warnings
		.iter()
		.find(|warning| warning.contains("MAX_PERMITTED_DATA_INCREASE"))
		.unwrap_or_else(|| {
			panic!(
				"cumulative growth must warn about the runtime cap: {:?}",
				output.data_warnings
			)
		});
	assert!(
		warning.contains("grows from 2 to 12002 bytes across the 2-step inline ladder v0 to v2"),
		"the warning quotes the cumulative ladder: {warning}"
	);
	assert!(
		warning.contains(
			"an intermediate version only resets the runtime cap when it migrates in a separate \
			 transaction"
		),
		"the warning explains when intermediates help: {warning}"
	);
	assert!(
		warning.contains(super::remedy::ACCOUNT_GROWTH_REMEDY),
		"the warning carries the shared remedy text: {warning}"
	);
	assert!(
		warning.contains("MigrationAccountGrowthExceeded"),
		"the warning names the on-chain error: {warning}"
	);
}

/// The same two sub-limit hops through a real `create` run: the caller must
/// assemble the whole supported stale ladder, not only the adjacent schema.
#[test]
fn make_warns_when_a_stale_ladder_exceeds_the_runtime_cap_cumulatively() {
	let fixture = publication_fixture();
	publish_current(&fixture);
	// v0 is 10 bytes; v1 adds 750 u64 fields (6,000 bytes), still under the cap.
	write_state_source(&fixture, &growing_fields(750));
	let first = create_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("create v1 draft: {error:?}"));
	assert!(
		!first
			.data_warnings
			.iter()
			.any(|warning| warning.contains("MAX_PERMITTED_DATA_INCREASE")),
		"a single sub-limit hop must not warn: {:?}",
		first.data_warnings
	);

	// Publish v1 so the next source appends v2 instead of replacing a draft.
	publish_current(&fixture);
	write_state_source(&fixture, &growing_fields(1_500));
	let second = create_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("create v2 draft: {error:?}"));
	let warning = second
		.data_warnings
		.iter()
		.find(|warning| warning.contains("MAX_PERMITTED_DATA_INCREASE"))
		.unwrap_or_else(|| {
			panic!(
				"cumulative growth must warn about the runtime cap: {:?}",
				second.data_warnings
			)
		});
	assert!(
		warning.contains("across the 2-step inline ladder v0 to v2"),
		"the warning quotes the cumulative ladder: {warning}"
	);
	assert!(
		warning.contains(
			"an intermediate version only resets the runtime cap when it migrates in a separate \
			 transaction"
		),
		"the warning explains when intermediates help: {warning}"
	);
	assert!(
		warning.contains(super::remedy::ACCOUNT_GROWTH_REMEDY),
		"the warning carries the shared remedy text: {warning}"
	);
	assert!(
		warning.contains("MigrationAccountGrowthExceeded"),
		"the warning names the on-chain error: {warning}"
	);
}

#[test]
fn field_type_changes_and_compact_changes_are_manual() {
	let old = schema(LayoutKind::Fixed, &[("count", "u64")]);
	let changed = schema(LayoutKind::Fixed, &[("count", "u32")]);
	let compact = schema(LayoutKind::Compact, &[("label", "String<8>")]);

	assert_eq!(
		transition_mode(&old, &SourceIntent::default(), &changed),
		TransitionMode::Manual
	);
	assert_eq!(
		transition_mode(&old, &SourceIntent::default(), &compact),
		TransitionMode::Manual
	);
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
		&SourceIntent {
			renames: vec![pina_abi::RenameMapping {
				from: "value".to_owned(),
				to: "points".to_owned(),
			}],
			..SourceIntent::default()
		},
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
		&SourceIntent::default(),
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

	let intent = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&SourceIntent {
			renames: vec![pina_abi::RenameMapping {
				from: "value".to_owned(),
				to: "points".to_owned(),
			}],
			..SourceIntent::default()
		},
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.unwrap_or_else(|error| panic!("settled draft refresh: {error:?}"));
	assert_eq!(
		intent.renames,
		vec![pina_abi::RenameMapping {
			from: "value".to_owned(),
			to: "points".to_owned(),
		}]
	);
	assert!(intent.dropped.is_empty());
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
	let intent = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&SourceIntent::default(),
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.unwrap_or_else(|error| panic!("answered rename: {error:?}"));
	assert_eq!(
		intent.renames,
		vec![pina_abi::RenameMapping {
			from: "value".to_owned(),
			to: "points".to_owned(),
		}]
	);
	assert!(intent.dropped.is_empty());

	let mut reader: &[u8] = b"n\n";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, true);
	let mut warnings = Vec::new();
	let intent = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&SourceIntent::default(),
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.unwrap_or_else(|error| panic!("declined rename: {error:?}"));
	assert!(intent.renames.is_empty());
	assert!(intent.dropped.contains("value"));
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
		&SourceIntent::default(),
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
	let intent = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&SourceIntent::default(),
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.unwrap_or_else(|error| panic!("acknowledged removal: {error:?}"));
	assert!(intent.renames.is_empty());
	assert!(intent.dropped.contains("value"));
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
		&SourceIntent::default(),
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

	let intent = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&SourceIntent::default(),
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.unwrap_or_else(|error| panic!("acknowledged unpaired removal: {error:?}"));
	assert!(intent.renames.is_empty());
	assert!(intent.dropped.contains("value"));
	assert!(
		warnings
			.iter()
			.any(|warning| warning.contains("field `value` (type `u64`) is removed")),
		"{warnings:?}"
	);
}

#[test]
fn corrupted_manifest_schemas_fail_validation_before_a_transition_is_planned() {
	let mut version = SchemaVersion {
		schema: schema(LayoutKind::Fixed, &[("value", "u64")]),
		process: None,
		transition: None,
	};
	// A hand-edited manifest can carry a field type the closed grammar
	// rejects. Manifest validation is the gate that fails, so no transition is
	// ever planned from a schema the generator cannot measure.
	version.schema.fields[0].rust_type = "Widget".to_owned();
	let rejection = version
		.schema
		.validate()
		.expect_err("corrupted field types must fail schema validation");
	assert!(
		rejection.contains("unsupported fixed ABI type"),
		"{rejection}"
	);
}

#[test]
fn draft_refreshes_ask_new_questions_through_the_recorded_history() {
	let fixture = published_fixture_with(&[("alpha", "u64"), ("beta", "u64")]);
	publish_current(&fixture);
	write_state_source(&fixture, "points: u64, beta: u64");
	let answers = MigrationAnswers::from_flags(&["alpha:points".to_owned()], &[], true).unwrap();
	let output = create_migrations_with_answers(&fixture.root, &answers)
		.unwrap_or_else(|error| panic!("record the rename on the draft: {error:?}"));
	assert_eq!(output.advanced_versions, ["account:1:01@1".to_owned()]);

	// The draft's destination now changes again: `beta` is removed while
	// `total` appears, and the recorded `alpha:points` rename no longer
	// answers the new question, so the refresh must ask instead of guessing.
	write_state_source(&fixture, "points: u64, total: u64");
	let rejection = create_migrations_with_answers(&fixture.root, &MigrationAnswers::default())
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
		&SourceIntent::default(),
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
		&SourceIntent::default(),
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
		&SourceIntent::default(),
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
		schema: schema(LayoutKind::Fixed, &[("value", "u64")]),
		process: None,
		transition: None,
	};
	let destination = schema(
		LayoutKind::Fixed,
		&[("value", "u64"), ("padding", "u64"), ("extra", "u64")],
	);
	let mut output = CreateMigrationsOutput::default();
	warn_about_account_growth(
		&identity,
		"Update",
		(0, &source),
		&[(0, &source)],
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
		schema: schema(LayoutKind::Fixed, &[("value", "u64")]),
		process: None,
		transition: None,
	};
	let destination = schema(
		LayoutKind::Compact,
		&[("label", "String<8>"), ("tags", "Vec<u16, 2>")],
	);
	let mut output = CreateMigrationsOutput::default();
	warn_about_account_growth(
		&identity,
		"State",
		(0, &source),
		&[(0, &source)],
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
		schema: schema(LayoutKind::Fixed, &[("value", "u64")]),
		process: None,
		transition: None,
	};
	let compact_destination = schema(LayoutKind::Compact, &[("label", "String<8>")]);
	let rejection = manual_transition_source(
		&instruction,
		MigrationVersionType::U8,
		&fixed_source,
		0,
		1,
		&compact_destination,
		&[],
	)
	.expect_err("a compact instruction destination must fail closed");
	assert!(
		format!("{rejection}").contains("instruction and event histories must use fixed layouts"),
		"{rejection}"
	);

	let compact_source = SchemaVersion {
		schema: schema(LayoutKind::Compact, &[("label", "String<8>")]),
		process: None,
		transition: None,
	};
	let fixed_destination = schema(LayoutKind::Fixed, &[("value", "u64")]);
	let rejection = manual_transition_source(
		&instruction,
		MigrationVersionType::U8,
		&compact_source,
		0,
		1,
		&fixed_destination,
		&[],
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
		scan_current_contracts(&project, &MigrationAuto::none())
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
		}],
	};
	let source = SchemaVersion {
		schema: schema(LayoutKind::Fixed, &[("value", "u64")]),
		process: Some(process.clone()),
		transition: None,
	};
	let destination = schema(LayoutKind::Compact, &[("label", "String<8>")]);
	let mut output = CreateMigrationsOutput::default();

	let rejection = create_transition(
		&project,
		TransitionRequest {
			identity: &identity,
			rust_name: "Update",
			source: &source,
			source_version: 0,
			stale_ladder: &[(0, &source)],
			intent: SourceIntent::default(),
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
	create_migrations(&fixture.root).unwrap_or_else(|error| panic!("advance draft: {error:?}"));
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
				schema: pinned_schema,
				process: None,
				transition: Some(Transition {
					mode: TransitionMode::Automatic,
					renames: vec![],
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
	let output = create_migrations_with_answers(&fixture.root, &answers)
		.unwrap_or_else(|error| panic!("make with persisted answers: {error:?}"));
	assert_eq!(output.advanced_versions, ["account:1:01@1".to_owned()]);
}

#[test]
fn flag_answers_override_persisted_answers_without_conflict() {
	let persisted = crate::project::MigrationsAnswersConfig {
		rename: vec!["value:points".to_owned()],
		assume_removed: vec![],
		manual: vec![],
	};
	let answers = MigrationAnswers::from_layers(&persisted, &["value:total".to_owned()], &[], true)
		.unwrap_or_else(|error| panic!("layer overriding answers: {error}"));
	let source = schema(LayoutKind::Fixed, &[("value", "u64")]);
	let destination = schema(LayoutKind::Fixed, &[("total", "u64")]);
	let mut warnings = Vec::new();
	let mut reader: &[u8] = b"";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, true);

	let intent = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&SourceIntent::default(),
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.unwrap_or_else(|error| panic!("resolved override: {error:?}"));
	assert_eq!(
		intent.renames,
		vec![pina_abi::RenameMapping {
			from: "value".to_owned(),
			to: "total".to_owned(),
		}]
	);
	assert!(intent.dropped.is_empty());
}

#[test]
fn flag_answers_contradicting_persisted_answers_fail_closed() {
	let persisted = crate::project::MigrationsAnswersConfig {
		rename: vec!["value:points".to_owned()],
		assume_removed: vec![],
		manual: vec![],
	};
	let removal_conflict =
		MigrationAnswers::from_layers(&persisted, &[], &["value".to_owned()], true)
			.expect_err("a removal against a persisted rename must fail closed");
	assert!(
		removal_conflict.contains("contradicts the persisted rename `value:points`"),
		"{removal_conflict}"
	);
}

/// The generated ABI layout test is the machine-checked replacement for the
/// hand-written offset asserts every consumer maintains today. It must record
/// the current size, every field offset, and the envelope geometry from the
/// manifest, and it must go stale when the schema changes.
#[test]
fn abi_layout_test_records_manifest_geometry() {
	let fixture = publication_fixture();
	create_migrations_with_answers(
		&fixture.root,
		&MigrationAnswers {
			no_interactive: true,
			..MigrationAnswers::default()
		},
	)
	.unwrap_or_else(|error| panic!("make migrations: {error}"));

	let generated = std::fs::read_to_string(fixture.root.join(ABI_LAYOUT_TEST_PATH))
		.unwrap_or_else(|error| panic!("read generated abi layout test: {error}"));

	// The fixture is one account with a single `u64` field behind a
	// `version_type = "u8"` envelope: a 1-byte discriminator, a 1-byte version,
	// and an 8-byte payload. Asserting the exact values pins the geometry
	// rather than merely finding the words "SIZE" or a digit somewhere.
	for expected in [
		"pub const DISCRIMINATOR_BYTES: usize = 1;",
		"pub const VERSION_OFFSET: usize = 1;",
		"pub const VERSION_BYTES: usize = 1;",
		"pub const MIGRATION_HEADER_SIZE: usize = 2;",
		"pub const PAYLOAD_SIZE: usize = 8;",
		"pub const MANIFEST_PAYLOAD_SIZE: usize = 8;",
		"pub const SIZE: usize = MIGRATION_HEADER_SIZE + PAYLOAD_SIZE;",
		"pub const VERSION: u32 = 0;",
	] {
		assert!(
			generated.contains(expected),
			"generated test must record `{expected}`:\n{generated}"
		);
	}
	// The single field sits after the envelope, at the header size.
	assert!(
		generated.contains(r#"("value", MIGRATION_HEADER_SIZE + 0, 8),"#),
		"generated test must record the field offset:\n{generated}"
	);
	// The program id is what a downstream decoder keys on.
	assert!(
		generated.contains("pub const PROGRAM_ID: &str ="),
		"generated test must record the program id:\n{generated}"
	);
}

/// Regeneration must be idempotent: running `create` twice produces identical
/// bytes, so `check` can compare content instead of guessing.
#[test]
fn abi_layout_test_regeneration_is_stable() {
	let fixture = publication_fixture();
	let answers = MigrationAnswers {
		no_interactive: true,
		..MigrationAnswers::default()
	};
	create_migrations_with_answers(&fixture.root, &answers)
		.unwrap_or_else(|error| panic!("first make: {error}"));
	let first = std::fs::read(fixture.root.join(ABI_LAYOUT_TEST_PATH))
		.unwrap_or_else(|error| panic!("read first: {error}"));

	create_migrations_with_answers(&fixture.root, &answers)
		.unwrap_or_else(|error| panic!("second make: {error}"));
	let second = std::fs::read(fixture.root.join(ABI_LAYOUT_TEST_PATH))
		.unwrap_or_else(|error| panic!("read second: {error}"));

	assert_eq!(first, second, "regeneration must be byte-stable");
}

/// A stale generated test is a failure, not a silent pass: `check` must report
/// it so the drift is fixed in the same change that moved the layout.
#[test]
fn check_rejects_a_stale_abi_layout_test() {
	let fixture = publication_fixture();
	let answers = MigrationAnswers {
		no_interactive: true,
		..MigrationAnswers::default()
	};
	create_migrations_with_answers(&fixture.root, &answers)
		.unwrap_or_else(|error| panic!("make: {error}"));

	// Simulate a hand edit that no longer matches the manifest.
	std::fs::write(
		fixture.root.join(ABI_LAYOUT_TEST_PATH),
		"// stale\nfn main() {}\n",
	)
	.unwrap_or_else(|error| panic!("write stale test: {error}"));

	let error = check_migrations_with_abi_layout(&fixture.root)
		.expect_err("a stale abi layout test must fail the check");
	assert!(
		error.to_string().contains("abi_layout") || error.to_string().contains("layout test"),
		"the error must name the stale layout test: {error}"
	);
}

/// Formatting the guard file must not make it stale.
///
/// `rustfmt` re-wraps the long `SCHEMA_SHA256` constants past 100 columns, so a
/// formatted file is never byte-identical to generator output. `fix:format` runs
/// on every checkout, so byte equality would leave `create` and the formatter
/// fighting: format, then `check`, then `create`, forever.
#[test]
fn formatting_the_abi_layout_test_keeps_it_current() {
	let fixture = publication_fixture();
	let answers = MigrationAnswers {
		no_interactive: true,
		..MigrationAnswers::default()
	};
	create_migrations_with_answers(&fixture.root, &answers)
		.unwrap_or_else(|error| panic!("make: {error}"));
	check_migrations_with_abi_layout(&fixture.root)
		.unwrap_or_else(|error| panic!("freshly generated guard must pass: {error}"));

	// Emulate the wrapping rustfmt applies to a long constant, without invoking
	// the toolchain from a unit test.
	let path = fixture.root.join(ABI_LAYOUT_TEST_PATH);
	let generated = std::fs::read_to_string(&path)
		.unwrap_or_else(|error| panic!("read generated guard: {error}"));
	let formatted = generated.replace(
		"pub const SCHEMA_SHA256: &str = \"",
		"pub const SCHEMA_SHA256: &str =\n\t\t\"",
	);
	assert_ne!(
		formatted, generated,
		"the fixture must contain a schema hash to wrap"
	);
	std::fs::write(&path, &formatted)
		.unwrap_or_else(|error| panic!("write formatted guard: {error}"));

	check_migrations_with_abi_layout(&fixture.root)
		.unwrap_or_else(|error| panic!("formatting must not report drift: {error}"));
}

/// A missing generated test is also stale: the account has migration history,
/// so the guard file is required.
#[test]
fn check_requires_the_abi_layout_test_when_history_exists() {
	let fixture = publication_fixture();
	let answers = MigrationAnswers {
		no_interactive: true,
		..MigrationAnswers::default()
	};
	create_migrations_with_answers(&fixture.root, &answers)
		.unwrap_or_else(|error| panic!("make: {error}"));
	std::fs::remove_file(fixture.root.join(ABI_LAYOUT_TEST_PATH))
		.unwrap_or_else(|error| panic!("remove test: {error}"));

	let error = check_migrations_with_abi_layout(&fixture.root)
		.expect_err("a missing abi layout test must fail the check");
	assert!(
		error.to_string().contains("abi_layout") || error.to_string().contains("layout test"),
		"the error must name the missing layout test: {error}"
	);
}

/// A current, matching test passes the check.
#[test]
fn check_accepts_a_current_abi_layout_test() {
	let fixture = publication_fixture();
	let answers = MigrationAnswers {
		no_interactive: true,
		..MigrationAnswers::default()
	};
	create_migrations_with_answers(&fixture.root, &answers)
		.unwrap_or_else(|error| panic!("make: {error}"));

	check_migrations_with_abi_layout(&fixture.root)
		.unwrap_or_else(|error| panic!("check: {error}"));
}

/// Enabling the migration envelope on a contract in a program that is already
/// live is a wire-format change: every byte after the discriminator shifts, so
/// every generated client, fixture, and hand-written decoder for that contract
/// sees it. The command must say so and require acknowledgement, which is the
/// `[migrations].auto` widening that made kickjump's 0.17 upgrade a 273-file
/// change discovered mid-flight.
#[test]
fn enveloping_new_contracts_on_a_live_program_requires_acknowledgement() {
	let fixture = publication_fixture();
	// The program is live: publish its current single contract.
	publish_current(&fixture);
	// A second contract appears in the source and now opts into migrations.
	write_state_source(&fixture, "value: u64");
	std::fs::write(
		fixture.root.join("src/lib.rs"),
		format!(
			"use pina::*;\ndeclare_id!(\"{}\");\n#[discriminator]\nenum Kind {{ State = 1, Other \
			 = 2 }}\n#[account(discriminator = Kind::State, migrations)]\nstruct State {{ value: \
			 u64 }}\n#[account(discriminator = Kind::Other, migrations)]\nstruct Other {{ amount: \
			 u64 }}\n",
			fixture.program_id
		),
	)
	.unwrap_or_else(|error| panic!("write expanded source: {error}"));

	let error = create_migrations_with_answers(
		&fixture.root,
		&MigrationAnswers {
			no_interactive: true,
			..MigrationAnswers::default()
		},
	)
	.expect_err("enveloping a new contract on a live program must require acknowledgement");
	assert!(
		error.to_string().contains("--envelope-ack"),
		"the error must name the acknowledgement flag: {error}"
	);
	assert!(
		error.to_string().contains("Other"),
		"the error must name the contract it covers: {error}"
	);
}

/// The acknowledgement flag records the change.
#[test]
fn envelope_acknowledgement_allows_the_change() {
	let fixture = publication_fixture();
	publish_current(&fixture);
	std::fs::write(
		fixture.root.join("src/lib.rs"),
		format!(
			"use pina::*;\ndeclare_id!(\"{}\");\n#[discriminator]\nenum Kind {{ State = 1, Other \
			 = 2 }}\n#[account(discriminator = Kind::State, migrations)]\nstruct State {{ value: \
			 u64 }}\n#[account(discriminator = Kind::Other, migrations)]\nstruct Other {{ amount: \
			 u64 }}\n",
			fixture.program_id
		),
	)
	.unwrap_or_else(|error| panic!("write expanded source: {error}"));

	create_migrations_with_answers(
		&fixture.root,
		&MigrationAnswers {
			no_interactive: true,
			envelope_ack: true,
			..MigrationAnswers::default()
		},
	)
	.unwrap_or_else(|error| panic!("acknowledged envelope must be recorded: {error}"));
}

/// A program with nothing published has nothing live to break, so a
/// first-time envelope needs no acknowledgement.
#[test]
fn enveloping_on_an_unpublished_program_needs_no_acknowledgement() {
	let fixture = publication_fixture();
	create_migrations_with_answers(
		&fixture.root,
		&MigrationAnswers {
			no_interactive: true,
			..MigrationAnswers::default()
		},
	)
	.unwrap_or_else(|error| panic!("unpublished envelope must not need an ack: {error}"));
}

/// An unreadable guard file is reported as a read error, not silently
/// overwritten or mistaken for a missing file.
#[test]
fn make_reports_an_unreadable_abi_layout_guard() {
	let fixture = publication_fixture();
	let guard = fixture.root.join(ABI_LAYOUT_TEST_PATH);
	std::fs::create_dir_all(&guard).unwrap_or_else(|error| panic!("create dir: {error}"));

	let error = create_migrations_with_answers(
		&fixture.root,
		&MigrationAnswers {
			no_interactive: true,
			..MigrationAnswers::default()
		},
	)
	.expect_err("an unreadable guard must fail the run");

	assert!(
		matches!(error, MigrationError::Read { .. }),
		"expected a read error, got {error}"
	);
}

/// Read a `const NAME: usize = VALUE;` out of a generated transition.
///
/// The generator wraps the constants in a `pub(crate)` module, so the
/// declaration may carry that visibility prefix; only the name and value are
/// load-bearing here.
fn generated_constant(generated: &str, name: &str) -> usize {
	let prefix = format!("const {name}: usize = ");
	generated
		.lines()
		.find_map(|line| {
			let line = line.trim();
			let line = line.strip_prefix("pub(crate) ").unwrap_or(line);
			line.strip_prefix(&prefix)
		})
		.and_then(|value| value.trim_end_matches(';').parse::<usize>().ok())
		.unwrap_or_else(|| panic!("generated transition has no `{name}`:\n{generated}"))
}

/// Apply a generated automatic transition to raw account bytes.
///
/// The generator emits `data.copy_within(a..b, c);` and `data[d..e].fill(0);`
/// statements. Interpreting those statements is the only way to prove the
/// offsets are right: a plan whose numbers are self-consistent can still read
/// and write the wrong bytes, and asserting on the rendered text cannot tell
/// the difference. Anything the interpreter does not recognize is a panic, so
/// the generator cannot quietly start emitting a form these tests ignore.
///
/// The buffer handling mirrors the on-chain executor: the account is grown to
/// `WORKING_SIZE` before `migrate` runs, the transition edits it in place, and
/// the result is then shrunk to `DESTINATION_SIZE`. Growing first matters for
/// transitions that write above the stored length.
fn apply_automatic_transition(generated: &str, mut data: Vec<u8>) -> Vec<u8> {
	let working_size = generated_constant(generated, "WORKING_SIZE");
	let destination_size = generated_constant(generated, "DESTINATION_SIZE");
	assert!(
		data.len() <= working_size,
		"executor only grows an account, never shrinks before `migrate`"
	);
	data.resize(working_size, 0);
	let mut copies = 0;
	let mut fills = 0;
	for line in generated.lines() {
		let line = line.trim();
		// Skip the `if data.len() < WORKING_SIZE { return; }` guard and the
		// braces that surround it; every other line is a byte edit or a panic.
		if !line.starts_with("data[") && !line.starts_with("data.") {
			continue;
		}
		let Some(statement) = line.strip_suffix(';') else {
			panic!("unterminated transition statement: {line}");
		};
		if let Some(rest) = statement.strip_prefix("data.") {
			if let Some(arguments) = rest.strip_prefix("copy_within(") {
				let arguments = arguments
					.strip_suffix(')')
					.unwrap_or_else(|| panic!("unterminated copy_within: {line}"));
				let (range, destination) = arguments
					.split_once(", ")
					.unwrap_or_else(|| panic!("copy_within needs two arguments: {line}"));
				let (start, end) = range
					.split_once("..")
					.unwrap_or_else(|| panic!("copy_within needs a range: {line}"));
				let start = start
					.parse::<usize>()
					.unwrap_or_else(|error| panic!("copy source start: {error}"));
				let end = end
					.parse::<usize>()
					.unwrap_or_else(|error| panic!("copy source end: {error}"));
				let destination = destination
					.parse::<usize>()
					.unwrap_or_else(|error| panic!("copy destination: {error}"));
				assert!(
					end <= data.len() && destination + (end - start) <= data.len(),
					"transition statement {line} runs past {} bytes",
					data.len()
				);
				// `copy_within` is memmove semantics: the plan's ordering is what
				// makes overlapping copies safe, so the interpreter must not
				// buffer for the generator.
				data.copy_within(start..end, destination);
				copies += 1;
				continue;
			}
			panic!("unrecognized data statement: {line}");
		}
		if let Some(arguments) = statement.strip_prefix("data[") {
			let (range, operation) = arguments
				.split_once("].")
				.unwrap_or_else(|| panic!("malformed fill statement: {line}"));
			assert_eq!(
				operation, "fill(0)",
				"the only supported fill writes zeroes: {line}"
			);
			let (start, end) = range
				.split_once("..")
				.unwrap_or_else(|| panic!("fill needs a range: {line}"));
			let start = start
				.parse::<usize>()
				.unwrap_or_else(|error| panic!("fill start: {error}"));
			let end = end
				.parse::<usize>()
				.unwrap_or_else(|error| panic!("fill end: {error}"));
			assert!(
				end <= data.len(),
				"fill {line} runs past {} bytes",
				data.len()
			);
			data[start..end].fill(0);
			fills += 1;
			continue;
		}
		panic!("unrecognized transition statement: {line}");
	}
	// The guard's own `data.len()` is a read, not an edit, so count only the
	// statements that reach the interpreter above.
	let edits = generated
		.lines()
		.filter(|line| {
			let line = line.trim();
			(line.starts_with("data[") || line.starts_with("data.")) && line.ends_with(';')
		})
		.count();
	assert_eq!(
		copies + fills,
		edits,
		"every generated byte edit must be interpreted:\n{generated}"
	);
	assert!(
		copies + fills > 0,
		"a transition with no byte movement cannot be verified:\n{generated}"
	);
	data.truncate(destination_size);
	data
}

/// Read one field out of an already-generated destination payload.
fn field_bytes(data: &[u8], offsets: &BTreeMap<String, (usize, usize)>, name: &str) -> Vec<u8> {
	let &(offset, size) = offsets
		.get(name)
		.unwrap_or_else(|| panic!("destination has no field `{name}`"));
	data[offset..offset + size].to_vec()
}

/// Payload offsets for a schema, panicking when it is not fixed.
fn offsets(schema: &DataSchema) -> BTreeMap<String, (usize, usize)> {
	schema
		.fixed_field_offsets()
		.unwrap_or_else(|| panic!("test schema must be a fixed layout"))
}

/// Plan and generate an automatic transition, failing closed when the proof
/// refuses the change.
fn generate_automatic(
	stored: &DataSchema,
	intent: &SourceIntent,
	destination: &DataSchema,
	version_type: MigrationVersionType,
) -> (String, MovePlan) {
	let plan = automatic_move_plan(stored, intent, destination).unwrap_or_else(|| {
		panic!(
			"expected an automatic plan for {:?} -> {:?}",
			stored.fields.iter().map(|f| &f.name).collect::<Vec<_>>(),
			destination
				.fields
				.iter()
				.map(|f| &f.name)
				.collect::<Vec<_>>()
		)
	});
	let identity = ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap();
	let generated = automatic_transition_source(&identity, version_type, 0, 1, &plan);
	(generated, plan)
}

/// Encode a fixed-layout version payload by concatenating field values.
fn payload(offsets: &BTreeMap<String, (usize, usize)>, values: &[(&str, &[u8])]) -> Vec<u8> {
	let total = offsets.values().map(|(o, s)| o + s).max().unwrap_or(0);
	let mut out = vec![0_u8; total];
	for (name, value) in values {
		let &(offset, size) = offsets
			.get(*name)
			.unwrap_or_else(|| panic!("no field `{name}`"));
		assert_eq!(value.len(), size, "field `{name}` takes {size} bytes");
		out[offset..offset + size].copy_from_slice(value);
	}
	out
}

/// Every removed field keeps occupying its bytes, so a retained field that
/// follows one must be read from the stored offset, not the compacted one.
///
/// This is the regression test for the offset bug: the generator previously
/// derived offsets from a source schema with dropped fields removed, which made
/// the surviving field after a removal read the *removed* field's bytes. Every
/// value here is a distinct byte, so a wrong offset is unambiguous.
#[test]
fn automatic_transition_reads_retained_fields_from_the_stored_layout() {
	// Stored v0: a, b, c. Destination v1: a, c (the middle field is dropped).
	let stored = schema(LayoutKind::Fixed, &[("a", "u8"), ("b", "u8"), ("c", "u8")]);
	let destination = schema(LayoutKind::Fixed, &[("a", "u8"), ("c", "u8")]);
	let intent = SourceIntent {
		dropped: BTreeSet::from(["b".to_owned()]),
		..SourceIntent::default()
	};

	let (generated, plan) =
		generate_automatic(&stored, &intent, &destination, MigrationVersionType::U8);
	// The stored payload is three bytes even though only two survive.
	assert_eq!(plan.source_size, 3);
	assert_eq!(plan.destination_size, 2);
	// Header (1 discriminator + 1 version) plus the three stored payload bytes.
	assert!(generated.contains("SOURCE_SIZE: usize = 5"), "{generated}");
	assert!(
		generated.contains("DESTINATION_SIZE: usize = 4"),
		"{generated}"
	);

	let stored_offsets = offsets(&stored);
	let destination_offsets = offsets(&destination);
	let header = 2_usize;
	let mut account = vec![1_u8, 0];
	account.extend(payload(
		&stored_offsets,
		&[("a", &[0xAA]), ("b", &[0xBB]), ("c", &[0xCC])],
	));

	let migrated = apply_automatic_transition(&generated, account);
	let body = &migrated[header..];
	assert_eq!(
		field_bytes(body, &destination_offsets, "c"),
		vec![0xCC],
		"`c` must keep its own byte, not inherit the dropped field's"
	);
	assert_eq!(field_bytes(body, &destination_offsets, "a"), vec![0xAA]);
}

/// The same proof for a removal at the head, where every retained field shifts
/// down and the last one is most easily overwritten.
#[test]
fn automatic_transition_handles_a_leading_removal_without_shifting_bytes() {
	let stored = schema(
		LayoutKind::Fixed,
		&[("gone", "u64"), ("first", "u8"), ("second", "u16")],
	);
	let destination = schema(LayoutKind::Fixed, &[("first", "u8"), ("second", "u16")]);
	let intent = SourceIntent {
		dropped: BTreeSet::from(["gone".to_owned()]),
		..SourceIntent::default()
	};

	let (generated, _) =
		generate_automatic(&stored, &intent, &destination, MigrationVersionType::U8);

	let stored_offsets = offsets(&stored);
	let destination_offsets = offsets(&destination);
	let mut account = vec![1_u8, 0];
	account.extend(payload(
		&stored_offsets,
		&[
			("gone", &[0xDE, 0xAD, 0xBE, 0xEF, 0xDE, 0xAD, 0xBE, 0xEF]),
			("first", &[0x11]),
			("second", &[0x22, 0x33]),
		],
	));

	let migrated = apply_automatic_transition(&generated, account);
	let body = &migrated[2..];
	assert_eq!(field_bytes(body, &destination_offsets, "first"), vec![0x11]);
	assert_eq!(
		field_bytes(body, &destination_offsets, "second"),
		vec![0x22, 0x33]
	);
}

/// A rename is a stored field arriving under a new name, so its bytes move from
/// the *old* name's offset. The stored schema is what supplies that offset.
#[test]
fn automatic_transition_moves_renamed_bytes_from_the_stored_field() {
	let stored = schema(
		LayoutKind::Fixed,
		&[("authority", "Address"), ("value", "u64")],
	);
	let destination = schema(
		LayoutKind::Fixed,
		&[("authority", "Address"), ("points", "u64")],
	);
	let intent = SourceIntent {
		renames: vec![pina_abi::RenameMapping {
			from: "value".to_owned(),
			to: "points".to_owned(),
		}],
		..SourceIntent::default()
	};

	let (generated, plan) =
		generate_automatic(&stored, &intent, &destination, MigrationVersionType::U8);
	assert_eq!(plan.moves.len(), 2, "both fields survive the rename");
	assert_eq!(plan.zero_fills.len(), 0, "a rename creates nothing new");

	let stored_offsets = offsets(&stored);
	let destination_offsets = offsets(&destination);
	let authority = [7_u8; 32];
	let mut account = vec![1_u8, 0];
	account.extend(payload(
		&stored_offsets,
		&[
			("authority", &authority),
			("value", &[0x39, 0x30, 0, 0, 0, 0, 0, 0]),
		],
	));

	let migrated = apply_automatic_transition(&generated, account);
	let body = &migrated[2..];
	assert_eq!(
		field_bytes(body, &destination_offsets, "points"),
		vec![0x39, 0x30, 0, 0, 0, 0, 0, 0]
	);
	assert_eq!(
		field_bytes(body, &destination_offsets, "authority"),
		authority
	);
}

/// An inserted field with no stored counterpart is zero-filled, and the fields
/// around it must still arrive intact.
#[test]
fn automatic_transition_zero_fills_an_inserted_field_and_keeps_neighbours() {
	let stored = schema(
		LayoutKind::Fixed,
		&[("authority", "Address"), ("value", "u64")],
	);
	let destination = schema(
		LayoutKind::Fixed,
		&[
			("authority", "Address"),
			("enabled", "bool"),
			("value", "u64"),
		],
	);
	let intent = SourceIntent::default();

	let (generated, plan) =
		generate_automatic(&stored, &intent, &destination, MigrationVersionType::U8);
	assert_eq!(plan.zero_fills.len(), 1);

	let stored_offsets = offsets(&stored);
	let destination_offsets = offsets(&destination);
	let authority = [3_u8; 32];
	let mut account = vec![1_u8, 0];
	account.extend(payload(
		&stored_offsets,
		&[
			("authority", &authority),
			("value", &[1, 2, 3, 4, 5, 6, 7, 8]),
		],
	));

	let migrated = apply_automatic_transition(&generated, account);
	let body = &migrated[2..];
	assert_eq!(field_bytes(body, &destination_offsets, "enabled"), vec![0]);
	assert_eq!(
		field_bytes(body, &destination_offsets, "value"),
		vec![1, 2, 3, 4, 5, 6, 7, 8]
	);
	assert_eq!(
		field_bytes(body, &destination_offsets, "authority"),
		authority
	);
}

/// Widening the version envelope must not shift the payload offsets the plan
/// reports: the plan is payload-relative and the generator adds the header.
#[test]
fn automatic_transition_offsets_are_payload_relative_across_version_widths() {
	let stored = schema(LayoutKind::Fixed, &[("a", "u8"), ("b", "u8"), ("c", "u8")]);
	let destination = schema(LayoutKind::Fixed, &[("a", "u8"), ("c", "u8")]);
	let intent = SourceIntent {
		dropped: BTreeSet::from(["b".to_owned()]),
		..SourceIntent::default()
	};
	let plan =
		automatic_move_plan(&stored, &intent, &destination).expect("removal after a survives");
	assert_eq!(
		plan.source_size, 3,
		"the stored payload keeps the dropped byte"
	);
	assert_eq!(plan.moves, vec![(0, 0, 1), (2, 1, 1)]);

	for version_type in [
		MigrationVersionType::U8,
		MigrationVersionType::U16,
		MigrationVersionType::U32,
	] {
		let identity = ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap();
		let generated = automatic_transition_source(&identity, version_type, 0, 1, &plan);
		let header = 1 + version_type.bytes();
		let expected_source = header + 3;
		assert!(
			generated.contains(&format!("SOURCE_SIZE: usize = {expected_source}")),
			"{version_type:?}: {generated}"
		);
		// The last surviving field is read from stored offset 2 regardless of
		// how wide the header is.
		assert!(
			generated.contains(&format!(
				"copy_within({}..{}, {})",
				header + 2,
				header + 3,
				header + 1
			)),
			"{version_type:?}: {generated}"
		);
	}
}

/// A field that moves right and a field that moves left cannot be expressed as
/// one sequence of `copy_within` calls, so the proof refuses it.
#[test]
fn automatic_plan_refuses_two_way_movement() {
	let stored = schema(
		LayoutKind::Fixed,
		&[("removed", "u64"), ("first", "u8"), ("second", "u8")],
	);
	let destination = schema(
		LayoutKind::Fixed,
		&[("first", "u8"), ("inserted", "u128"), ("second", "u8")],
	);
	let intent = SourceIntent {
		dropped: BTreeSet::from(["removed".to_owned()]),
		..SourceIntent::default()
	};
	assert!(automatic_move_plan(&stored, &intent, &destination).is_none());
}

/// A stored field whose type changes in place reinterprets live bytes, so it is
/// never automatic even when the widths happen to match.
#[test]
fn automatic_plan_refuses_a_same_width_type_change() {
	for (before, after) in [
		("u64", "i64"),
		("u32", "f32"),
		("u8", "bool"),
		("Address", "[u8; 32]"),
	] {
		let stored = schema(LayoutKind::Fixed, &[("value", before)]);
		let destination = schema(LayoutKind::Fixed, &[("value", after)]);
		assert!(
			automatic_move_plan(&stored, &SourceIntent::default(), &destination).is_none(),
			"{before} -> {after} must stay manual"
		);
	}
}

/// Every stored field must be accounted for. A type change in the middle leaves
/// the changed field unpaired, and the plan fails closed rather than emitting
/// copies that skip it.
#[test]
fn automatic_plan_requires_every_stored_field_to_be_accounted_for() {
	let stored = schema(
		LayoutKind::Fixed,
		&[("head", "u8"), ("amount", "u32"), ("tail", "u8")],
	);
	let destination = schema(
		LayoutKind::Fixed,
		&[("head", "u8"), ("amount", "u64"), ("tail", "u8")],
	);
	assert!(
		automatic_move_plan(&stored, &SourceIntent::default(), &destination).is_none(),
		"an unpaired stored field cannot be copied or discarded silently"
	);
}

/// Two destination fields may not claim the same stored field: the second copy
/// would duplicate bytes the developer meant to place once.
#[test]
fn automatic_plan_refuses_two_fields_reading_one_stored_field() {
	let stored = schema(LayoutKind::Fixed, &[("value", "u64")]);
	let destination = schema(LayoutKind::Fixed, &[("left", "u64"), ("right", "u64")]);
	let intent = SourceIntent {
		renames: vec![
			pina_abi::RenameMapping {
				from: "value".to_owned(),
				to: "left".to_owned(),
			},
			pina_abi::RenameMapping {
				from: "value".to_owned(),
				to: "right".to_owned(),
			},
		],
		..SourceIntent::default()
	};
	assert!(automatic_move_plan(&stored, &intent, &destination).is_none());
}

/// A manual conversion is developer-owned by definition, so the proof refuses
/// the change even when the byte movement would otherwise be trivial.
#[test]
fn manual_intent_forces_a_manual_transition() {
	let stored = schema(
		LayoutKind::Fixed,
		&[("first_name", "String<8>"), ("last_name", "String<8>")],
	);
	let destination = schema(LayoutKind::Fixed, &[("name", "String<16>")]);
	let intent = SourceIntent {
		renames: vec![pina_abi::RenameMapping {
			from: "first_name".to_owned(),
			to: "name".to_owned(),
		}],
		manual: BTreeSet::from(["name".to_owned()]),
		dropped: BTreeSet::from(["last_name".to_owned()]),
		..SourceIntent::default()
	};
	assert!(automatic_move_plan(&stored, &intent, &destination).is_none());
	assert_eq!(
		transition_mode(&stored, &intent, &destination),
		TransitionMode::Manual
	);
}

/// A rename that only changes the name is automatic; the same rename with a
/// manual answer is not. This is the escape hatch that lets a developer own the
/// conversion without making the schema look different.
#[test]
fn a_manual_answer_turns_an_otherwise_automatic_rename_into_a_manual_transition() {
	let stored = schema(LayoutKind::Fixed, &[("value", "u64")]);
	let destination = schema(LayoutKind::Fixed, &[("points", "u64")]);
	let rename = pina_abi::RenameMapping {
		from: "value".to_owned(),
		to: "points".to_owned(),
	};
	let automatic = SourceIntent {
		renames: vec![rename.clone()],
		..SourceIntent::default()
	};
	assert_eq!(
		transition_mode(&stored, &automatic, &destination),
		TransitionMode::Automatic
	);

	let manual = SourceIntent {
		renames: vec![rename],
		manual: BTreeSet::from(["points".to_owned()]),
		..SourceIntent::default()
	};
	assert_eq!(
		transition_mode(&stored, &manual, &destination),
		TransitionMode::Manual
	);
}

/// An unacknowledged removal cannot become automatic: the stored bytes would be
/// copied over a retained field or silently left behind.
#[test]
fn an_unacknowledged_removal_is_not_automatic() {
	let stored = schema(LayoutKind::Fixed, &[("a", "u8"), ("b", "u8"), ("c", "u8")]);
	let destination = schema(LayoutKind::Fixed, &[("a", "u8"), ("c", "u8")]);
	assert!(
		automatic_move_plan(&stored, &SourceIntent::default(), &destination).is_none(),
		"dropping `b` needs the developer's acknowledgement"
	);
}

/// A trailing removal shifts nothing, so it stays automatic with or without the
/// dropped field's bytes being read by anything.
#[test]
fn a_trailing_removal_shifts_nothing() {
	let stored = schema(
		LayoutKind::Fixed,
		&[("a", "u8"), ("b", "u8"), ("stale", "u64")],
	);
	let destination = schema(LayoutKind::Fixed, &[("a", "u8"), ("b", "u8")]);
	let intent = SourceIntent {
		dropped: BTreeSet::from(["stale".to_owned()]),
		..SourceIntent::default()
	};
	let plan = automatic_move_plan(&stored, &intent, &destination).expect("trailing removal");
	assert_eq!(plan.moves, vec![(0, 0, 1), (1, 1, 1)]);

	let stored_offsets = offsets(&stored);
	let destination_offsets = offsets(&destination);
	let mut account = vec![1_u8, 0];
	account.extend(payload(
		&stored_offsets,
		&[("a", &[0x0A]), ("b", &[0x0B]), ("stale", &[9; 8])],
	));
	let identity = ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap();
	let generated = automatic_transition_source(&identity, MigrationVersionType::U8, 0, 1, &plan);
	let migrated = apply_automatic_transition(&generated, account);
	let body = &migrated[2..];
	assert_eq!(field_bytes(body, &destination_offsets, "a"), vec![0x0A]);
	assert_eq!(field_bytes(body, &destination_offsets, "b"), vec![0x0B]);
}

/// The rename escape hatch: a fixed-layout rename that Pina would otherwise
/// generate can be handed to the developer so a conversion such as combining
/// two fields into one is written by hand.
#[test]
fn manual_answers_generate_an_editable_transition_for_a_rename() {
	let fixture = publication_fixture();
	publish_current(&fixture);
	write_state_source(&fixture, "points: u64");

	let answers = MigrationAnswers::from_flags_with_manual(&[], &[], &["points".to_owned()], true)
		.unwrap_or_else(|error| panic!("answers: {error}"));
	let output = create_migrations_with_answers(&fixture.root, &answers)
		.unwrap_or_else(|error| panic!("make with manual answer: {error:?}"));
	assert_eq!(output.advanced_versions, ["account:1:01@1".to_owned()]);
	// Windows reports manual transition paths with the `\\?\` extended-length
	// prefix while `join` builds a plain path, so compare the file names.
	assert_eq!(
		output
			.manual_transitions
			.iter()
			.map(|path| path.file_name().and_then(|name| name.to_str()))
			.collect::<Vec<_>>(),
		[Some("v0_to_v1.rs")]
	);

	let manifest = load_manifest(&fixture.root.join(MANIFEST_PATH))
		.unwrap_or_else(|error| panic!("load manifest: {error:?}"))
		.expect("fixture manifest");
	let transition = manifest.contracts["account:1:01"].versions[1]
		.transition
		.as_ref()
		.expect("advanced version carries a transition");
	assert_eq!(transition.mode, TransitionMode::Manual);
	// The manual field is derived from `mode` plus the recorded renames, so the
	// recorded history is what keeps a repeated run manual.
	assert_eq!(
		recorded_intent(Some(transition)).manual,
		BTreeSet::from(["points".to_owned()]),
		"the manual answer survives into the recorded intent"
	);
	// A version with no recorded transition — the initial draft — carries no
	// intent at all, so the first hop is diffed from scratch.
	let fresh = recorded_intent(None);
	assert!(fresh.manual.is_empty() && fresh.renames.is_empty() && !fresh.force_manual);

	let generated = std::fs::read_to_string(
		fixture
			.root
			.join("migrations/transitions/account_1_01/v0_to_v1.rs"),
	)
	.unwrap_or_else(|error| panic!("read transition: {error}"));
	assert!(
		generated.contains("TODO(pina-manual-migration)"),
		"the developer owns the body: {generated}"
	);
	assert!(
		generated.contains("SOURCE_SIZE: usize = 10"),
		"the preflight still describes the stored shape: {generated}"
	);
}

/// A manual answer is a standing instruction: replacing the stub body and
/// re-running `make` must not regenerate an automatic transition over it.
#[test]
fn a_manual_answer_keeps_the_developers_body_across_repeated_runs() {
	let fixture = publication_fixture();
	publish_current(&fixture);
	write_state_source(&fixture, "points: u64");

	let answers = MigrationAnswers::from_flags_with_manual(&[], &[], &["points".to_owned()], true)
		.unwrap_or_else(|error| panic!("answers: {error}"));
	create_migrations_with_answers(&fixture.root, &answers)
		.unwrap_or_else(|error| panic!("make with manual answer: {error:?}"));

	let path = fixture
		.root
		.join("migrations/transitions/account_1_01/v0_to_v1.rs");
	let body = "// developer-owned conversion\npub(crate) const FROM_VERSION: u32 = \
	            0;\npub(crate) const TO_VERSION: u32 = 1;\npub(crate) const SOURCE_SIZE: usize = \
	            10;\npub(crate) const DESTINATION_SIZE: usize = 10;\npub(crate) const \
	            WORKING_SIZE: usize = 10;\npub(crate) fn migrate(data: &mut [u8]) {\n\tlet _ = \
	            data;\n}\n";
	std::fs::write(&path, body).unwrap_or_else(|error| panic!("write body: {error}"));

	// A run whose recorded answers already include the manual field keeps the
	// developer's body instead of replacing it with a regenerated stub.
	let output = create_migrations_with_answers(&fixture.root, &MigrationAnswers::default())
		.unwrap_or_else(|error| panic!("re-run make: {error:?}"));
	assert_eq!(output.unchanged_contracts, ["account:1:01".to_owned()]);
	let after = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("read: {error}"));
	assert_eq!(after, body, "the developer's conversion must survive");
}

/// `--manual` names an added field. Naming anything else is a mistake worth
/// reporting rather than silently ignoring.
#[test]
fn manual_answers_reject_fields_the_change_did_not_add() {
	let source = schema(LayoutKind::Fixed, &[("value", "u64")]);
	let destination = schema(LayoutKind::Fixed, &[("value", "u64"), ("added", "u8")]);
	let answers =
		MigrationAnswers::from_flags_with_manual(&[], &[], &["value".to_owned()], true).unwrap();
	let mut warnings = Vec::new();
	let mut reader: &[u8] = b"";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, false);

	let rejection = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&SourceIntent::default(),
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.expect_err("`--manual value` names a retained field");
	assert!(
		format!("{rejection}")
			.contains("`--manual value` names a field that this change did not add"),
		"{rejection}"
	);
}

/// `--manual` claims an added field's value, and a rename pairs it with a
/// stored field. Acknowledging that stored field's removal contradicts both, so
/// the pair fails closed instead of leaving a conversion without its input.
#[test]
fn manual_and_removal_answers_contradict_each_other() {
	let source = schema(
		LayoutKind::Fixed,
		&[("first_name", "String<8>"), ("last_name", "String<8>")],
	);
	let destination = schema(LayoutKind::Fixed, &[("name", "String<17>")]);
	let answers = MigrationAnswers::from_flags_with_manual(
		&["first_name:name".to_owned()],
		&["first_name".to_owned()],
		&["name".to_owned()],
		true,
	)
	.unwrap();
	let mut warnings = Vec::new();
	let mut reader: &[u8] = b"";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, false);

	let rejection = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&SourceIntent::default(),
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.expect_err("a field cannot be both renamed and discarded");
	assert!(
		format!("{rejection}")
			.contains("field `first_name` is answered with both `--rename` and `--assume-removed`"),
		"{rejection}"
	);
}

/// Combining two stored fields into one is the motivating case: a manual answer
/// pairs the destination with a stored field even when the types differ, so the
/// developer can write the conversion.
#[test]
fn manual_answers_pair_a_destination_with_a_differently_typed_stored_field() {
	let source = schema(
		LayoutKind::Fixed,
		&[("first_name", "String<8>"), ("last_name", "String<8>")],
	);
	let destination = schema(LayoutKind::Fixed, &[("name", "String<17>")]);
	let answers = MigrationAnswers::from_flags_with_manual(
		&["first_name:name".to_owned()],
		&["last_name".to_owned()],
		&["name".to_owned()],
		true,
	)
	.unwrap();
	let mut warnings = Vec::new();
	let mut reader: &[u8] = b"";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, false);

	let intent = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&SourceIntent::default(),
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.unwrap_or_else(|error| panic!("manual pairing: {error}"));
	assert_eq!(
		intent.renames,
		[pin_rename("first_name", "name")],
		"the destination pairs with the field whose bytes it reads"
	);
	assert_eq!(intent.manual, BTreeSet::from(["name".to_owned()]));
	assert_eq!(intent.dropped, BTreeSet::from(["last_name".to_owned()]));
	assert!(
		automatic_move_plan(&source, &intent, &destination).is_none(),
		"a manual field is never automatic"
	);
}

/// A type change without `--manual` still fails closed, and the message names
/// the flag that makes it legal.
#[test]
fn a_type_changing_rename_needs_a_manual_answer() {
	let source = schema(LayoutKind::Fixed, &[("amount", "u16")]);
	let destination = schema(LayoutKind::Fixed, &[("amount", "u64")]);
	let answers = MigrationAnswers::from_flags(&["amount:amount".to_owned()], &[], true);
	let mut warnings = Vec::new();
	let mut reader: &[u8] = b"";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, false);

	// `amount` is retained, so the rename cannot target it at all.
	let rejection = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&SourceIntent::default(),
		&answers.unwrap_or_else(|error| panic!("answers: {error}")),
		&mut warnings,
		&mut prompts,
	)
	.expect_err("a retained field is not a rename target");
	assert!(
		format!("{rejection}").contains("was not removed by this change"),
		"{rejection}"
	);
}

/// A width change under an unchanged name is a manual transition, and the
/// generated stub reports both stored and destination sizes so the developer
/// can see the conversion they own.
#[test]
fn a_same_name_width_change_generates_a_manual_stub_with_both_sizes() {
	let fixture = published_fixture_with(&[("amount", "u16")]);
	publish_current(&fixture);
	write_state_source(&fixture, "amount: u64");

	let output = create_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("make with widen: {error:?}"));
	assert_eq!(output.advanced_versions, ["account:1:01@1".to_owned()]);
	assert_eq!(output.manual_transitions.len(), 1);

	let generated = std::fs::read_to_string(
		fixture
			.root
			.join("migrations/transitions/account_1_01/v0_to_v1.rs"),
	)
	.unwrap_or_else(|error| panic!("read transition: {error}"));
	assert!(
		generated.contains("Source version: 0 (4 bytes)"),
		"{generated}"
	);
	assert!(
		generated.contains("Destination version: 1 (10 bytes)"),
		"{generated}"
	);
}

/// Manual answers travel with the persisted table like the other disambiguation
/// answers, so a fresh clone reproduces the developer's conversion.
#[test]
fn persisted_manual_answers_replay_without_flags() {
	let persisted = crate::project::MigrationsAnswersConfig {
		rename: vec!["first_name:name".to_owned()],
		assume_removed: vec!["last_name".to_owned()],
		manual: vec!["name".to_owned()],
	};
	let answers = MigrationAnswers::from_layers_with_manual(&persisted, &[], &[], &[], false)
		.unwrap_or_else(|error| panic!("replay persisted answers: {error}"));
	assert_eq!(
		answers.manual,
		BTreeSet::from(["name".to_owned()]),
		"the manual conversion is remembered without a flag"
	);
	assert_eq!(
		answers.renames.get("first_name").map(String::as_str),
		Some("name")
	);

	// The persisted answers resolve the same diff the flags did, producing a
	// manual transition rather than a generated one.
	let source = schema(
		LayoutKind::Fixed,
		&[("first_name", "String<8>"), ("last_name", "String<8>")],
	);
	let destination = schema(LayoutKind::Fixed, &[("name", "String<17>")]);
	let mut warnings = Vec::new();
	let mut reader: &[u8] = b"";
	let mut transcript = Vec::new();
	let mut prompts = PromptIo::new(&mut reader, &mut transcript, false);
	let intent = resolve_field_changes(
		"account:1:01",
		&source,
		&destination,
		&SourceIntent::default(),
		&answers,
		&mut warnings,
		&mut prompts,
	)
	.unwrap_or_else(|error| panic!("resolve persisted answers: {error}"));
	assert!(automatic_move_plan(&source, &intent, &destination).is_none());
}

/// A flag answer replaces the persisted answer for the same destination field
/// in either direction: the developer's newest answer is the one that applies.
#[test]
fn flag_answers_replace_conflicting_persisted_manual_answers() {
	let persisted = crate::project::MigrationsAnswersConfig {
		rename: vec!["first_name:name".to_owned()],
		assume_removed: Vec::new(),
		manual: Vec::new(),
	};
	// `--manual name` upgrades a persisted automatic rename into a manual one.
	let manual = MigrationAnswers::from_layers_with_manual(
		&persisted,
		&[],
		&[],
		&["name".to_owned()],
		false,
	)
	.unwrap_or_else(|error| panic!("flag manual over persisted rename: {error}"));
	assert!(manual.manual.contains("name"));
	assert!(
		!manual.renames.contains_key("first_name"),
		"the persisted rename is superseded, not layered underneath"
	);

	// And a persisted manual answer is replaced by an explicit flag rename:
	// moving the bytes verbatim is a different decision from converting them.
	let persisted_manual = crate::project::MigrationsAnswersConfig {
		rename: Vec::new(),
		assume_removed: Vec::new(),
		manual: vec!["name".to_owned()],
	};
	let renamed = MigrationAnswers::from_layers_with_manual(
		&persisted_manual,
		&["first_name:name".to_owned()],
		&[],
		&[],
		false,
	)
	.unwrap_or_else(|error| panic!("flag rename over persisted manual: {error}"));
	assert!(!renamed.manual.contains("name"));
	assert_eq!(
		renamed.renames.get("first_name").map(String::as_str),
		Some("name")
	);
}

/// Answers that contradict each other fail closed whichever source they come
/// from: a persisted answer must not silently override a flag, or the reverse.
#[test]
fn persisted_and_flag_manual_answers_fail_closed_on_contradiction() {
	// A persisted removal of the field a flag now converts by hand.
	let persisted = crate::project::MigrationsAnswersConfig {
		rename: Vec::new(),
		assume_removed: vec!["name".to_owned()],
		manual: Vec::new(),
	};
	let rejection = MigrationAnswers::from_layers_with_manual(
		&persisted,
		&[],
		&[],
		&["name".to_owned()],
		false,
	)
	.expect_err("a persisted removal cannot coexist with a manual conversion");
	assert!(
		rejection.contains("contradicts the persisted removal of `name` in pina.toml"),
		"{rejection}"
	);
	assert!(
		!rejection.contains('\t'),
		"continuation padding must not reach the message: {rejection:?}"
	);

	// And the reverse: a flag removal against a persisted manual conversion.
	let persisted = crate::project::MigrationsAnswersConfig {
		rename: Vec::new(),
		assume_removed: Vec::new(),
		manual: vec!["name".to_owned()],
	};
	let rejection = MigrationAnswers::from_layers_with_manual(
		&persisted,
		&[],
		&["name".to_owned()],
		&[],
		false,
	)
	.expect_err("a persisted manual conversion cannot coexist with a removal");
	// The rendered sentence must read cleanly: the fragment says "manual
	// conversion" (not "manual conversion for") and no continuation padding
	// leaks into the message.
	assert!(
		rejection.contains("contradicts the persisted manual conversion in pina.toml"),
		"{rejection}"
	);

	// A flag removal for a field no persisted answer mentions is not a
	// contradiction at all: it layers cleanly and answers the question.
	let unrelated = crate::project::MigrationsAnswersConfig {
		rename: vec!["first_name:name".to_owned()],
		assume_removed: Vec::new(),
		manual: Vec::new(),
	};
	let layered = MigrationAnswers::from_layers_with_manual(
		&unrelated,
		&[],
		&["last_name".to_owned()],
		&[],
		false,
	)
	.unwrap_or_else(|error| panic!("an unrelated removal is not a contradiction: {error}"));
	assert!(layered.removed.contains("last_name"));
	assert_eq!(
		layered.renames.get("first_name").map(String::as_str),
		Some("name"),
		"the persisted rename survives alongside the new removal"
	);
}

/// A `--manual` answer on a brand-new field has no rename to record it, so the
/// recorded mode is the only durable marker. A later schema change re-derives
/// the draft, and the developer's body must survive that refresh instead of
/// being replaced by a generated transition.
#[test]
fn a_manual_answer_on_an_added_field_survives_a_draft_refresh() {
	let fixture = publication_fixture();
	publish_current(&fixture);
	// A plain addition would be automatic; the developer chooses to own it.
	write_state_source(&fixture, "value: u64, derived: u64");

	let answers = MigrationAnswers::from_flags_with_manual(&[], &[], &["derived".to_owned()], true)
		.unwrap_or_else(|error| panic!("answers: {error}"));
	let output = create_migrations_with_answers(&fixture.root, &answers)
		.unwrap_or_else(|error| panic!("make with manual add: {error:?}"));
	assert_eq!(output.manual_transitions.len(), 1, "{output:?}");

	let path = fixture
		.root
		.join("migrations/transitions/account_1_01/v0_to_v1.rs");
	let body = "// developer-owned conversion\npub(crate) const FROM_VERSION: u32 = \
	            0;\npub(crate) const TO_VERSION: u32 = 1;\npub(crate) const SOURCE_SIZE: usize = \
	            10;\npub(crate) const DESTINATION_SIZE: usize = 18;\npub(crate) const \
	            WORKING_SIZE: usize = 18;\npub(crate) fn migrate(data: &mut [u8]) {\n\tlet _ = \
	            data;\n}\n";
	std::fs::write(&path, body).unwrap_or_else(|error| panic!("write body: {error}"));

	// A further change to the same draft forces the transition to be re-derived.
	// Without a durable marker the mode would flip to automatic and the body
	// would be regenerated over.
	write_state_source(&fixture, "value: u64, derived: u64, extra: u16");
	let output = create_migrations_with_answers(&fixture.root, &MigrationAnswers::default())
		.unwrap_or_else(|error| panic!("refresh draft: {error:?}"));
	assert_eq!(output.updated_drafts, ["account:1:01@1".to_owned()]);
	assert_eq!(
		output.manual_transitions.len(),
		1,
		"the transition stays manual across the refresh: {output:?}"
	);

	let after = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("read: {error}"));
	assert_eq!(after, body, "the developer's conversion must survive");
}

/// A manual transition Pina could not prove — an in-place type change — is
/// recorded with an empty `manual` set, since no answer named a field. The
/// recorded mode alone has to keep it manual across a refresh.
#[test]
fn an_unprovable_transition_stays_manual_without_a_named_field() {
	let stored = schema(LayoutKind::Fixed, &[("amount", "u16")]);
	let destination = schema(LayoutKind::Fixed, &[("amount", "u64")]);

	let plan = automatic_move_plan(&stored, &SourceIntent::default(), &destination);
	assert!(plan.is_none(), "a width change is not provable");
	assert_eq!(
		transition_mode(&stored, &SourceIntent::default(), &destination),
		TransitionMode::Manual
	);

	// Recording that mode reproduces an intent that still refuses to generate.
	let recorded = SourceIntent {
		force_manual: true,
		..SourceIntent::default()
	};
	assert!(recorded.manual.is_empty());
	assert!(
		automatic_move_plan(&stored, &recorded, &destination).is_none(),
		"a recorded manual transition is never re-derived as automatic"
	);
}

/// A recorded manual transition describes only the hop it was written for.
/// The change that follows it is a fresh diff: seeding that new hop with the
/// previous hop's manual mode would brand every later hop manual forever, even
/// one the byte-level proof fully justifies.
#[test]
fn a_manual_hop_does_not_brand_later_hops_manual() {
	let fixture = published_fixture_with(&[("value", "u64")]);
	publish_current(&fixture);

	// A same-name width change has no automatic proof, so v0 -> v1 is manual.
	write_state_source(&fixture, "value: u32");
	let manual_hop =
		create_migrations(&fixture.root).unwrap_or_else(|error| panic!("manual make: {error:?}"));
	assert_eq!(manual_hop.advanced_versions, ["account:1:01@1".to_owned()]);

	// Publishing v1 freezes it, but only once the developer replaces the TODO
	// body; the narrowing implementation mirrors what a developer would write.
	let stub_path = fixture
		.root
		.join("migrations/transitions/account_1_01/v0_to_v1.rs");
	let stub = std::fs::read_to_string(&stub_path)
		.unwrap_or_else(|error| panic!("read transition stub: {error}"));
	let implemented = stub
		.replace(
			"// TODO(pina-manual-migration): the source shape is preflighted; this conversion \
			 must be total and fully initialize destination.",
			"// Narrows the u64 value into u32, saturating at u32::MAX.",
		)
		.replace(
			"\tlet _ = data;\n}",
			concat!(
				"\tlet wide = u64::from_le_bytes(data[2..10].try_into().unwrap());\n",
				"\tlet narrowed = u32::try_from(wide).unwrap_or(u32::MAX);\n",
				"\tdata[2..6].copy_from_slice(&narrowed.to_le_bytes());\n",
				"\tdata[6..10].fill(0);\n",
				"}",
			),
		);
	assert_ne!(
		stub, implemented,
		"the stub TODO body should be replaceable"
	);
	std::fs::write(&stub_path, implemented)
		.unwrap_or_else(|error| panic!("write transition: {error}"));
	create_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("refresh draft hash: {error:?}"));
	publish_current(&fixture);

	// Freezing v1 makes the next source change a new adjacent hop. Adding a
	// field is provable, so it must record automatic despite the manual hop
	// beneath it.
	write_state_source(&fixture, "value: u32, extra: u64");
	let automatic_hop = create_migrations(&fixture.root)
		.unwrap_or_else(|error| panic!("make after manual hop: {error:?}"));
	assert_eq!(
		automatic_hop.advanced_versions,
		["account:1:01@2".to_owned()]
	);
	let manifest = load_manifest(&fixture.root.join(MANIFEST_PATH))
		.unwrap_or_else(|error| panic!("load manifest: {error:?}"))
		.expect("fixture manifest");
	let later = manifest.contracts["account:1:01"].versions[2]
		.transition
		.as_ref()
		.expect("the new hop carries a transition");
	assert_eq!(later.mode, TransitionMode::Automatic);

	// The manual hop itself keeps its recorded mode: the flag belongs to the
	// hop that earned it, and a refresh of that draft still replays it.
	let manual = manifest.contracts["account:1:01"].versions[1]
		.transition
		.as_ref()
		.expect("the manual hop carries a transition");
	assert_eq!(manual.mode, TransitionMode::Manual);
}

/// A compact layout has no fixed offsets to prove, so the plan refuses it and
/// the developer owns the conversion. Both directions of the pair are covered:
/// compact source, compact destination, and the mixed shape.
#[test]
fn automatic_plan_refuses_every_non_fixed_layout() {
	let fixed = schema(LayoutKind::Fixed, &[("name", "u64")]);
	let compact = schema(LayoutKind::Compact, &[("name", "String<8>")]);
	let intent = SourceIntent {
		renames: vec![pin_rename("name", "name")],
		dropped: BTreeSet::from(["name".to_owned()]),
		..SourceIntent::default()
	};

	for (stored, destination) in [(&compact, &compact), (&compact, &fixed), (&fixed, &compact)] {
		assert!(
			automatic_move_plan(stored, &intent, destination).is_none(),
			"a non-fixed layout ({:?} -> {:?}) must stay manual",
			stored.layout,
			destination.layout
		);
	}
}

/// A flag removal against a persisted manual conversion fails closed, mirroring
/// the check for a persisted rename. Both are answers about what happens to one
/// field's bytes, so a stale `pina.toml` must not silently drop the conversion.
#[test]
fn flag_removals_contradicting_a_persisted_manual_answer_fail_closed() {
	let persisted = crate::project::MigrationsAnswersConfig {
		rename: Vec::new(),
		assume_removed: Vec::new(),
		manual: vec!["name".to_owned()],
	};
	let rejection = MigrationAnswers::from_layers_with_manual(
		&persisted,
		&[],
		&["name".to_owned()],
		&[],
		false,
	)
	.expect_err("a flag removal cannot contradict a persisted manual conversion");
	assert!(
		rejection.contains("contradicts the persisted manual conversion"),
		"{rejection}"
	);
}

/// The manual stub's offset comment shows each field's stored range beside its
/// destination range, aligned in fixed columns. These tests exercise the
/// comment directly: every layout family, rename pairing, and annotation.
#[test]
fn offset_comment_aligns_fixed_removal_and_addition() {
	let stored = schema(
		LayoutKind::Fixed,
		&[("authority", "Address"), ("value", "u64"), ("memo", "u16")],
	);
	let destination = schema(
		LayoutKind::Fixed,
		&[("authority", "Address"), ("memo", "u16"), ("flags", "u8")],
	);

	let comment = layout_comment(&stored, &destination, &[]);
	let lines: Vec<&str> = comment.trim_end().split('\n').collect();
	assert_eq!(
		lines[0], "// Payload offsets, relative to the version envelope header:",
		"{comment}"
	);
	// The header row and every field row share the same column padding.
	assert_eq!(lines[1], "// field      stored  destination", "{comment}");
	// Rows follow stored declaration order, then added fields in destination
	// order, so a reader scans the stored layout top to bottom.
	assert_eq!(lines[2], "// authority  0..32   0..32", "{comment}");
	assert_eq!(
		lines[3], "// value      32..40  -            (removed)",
		"{comment}"
	);
	assert_eq!(lines[4], "// memo       40..42  32..34", "{comment}");
	assert_eq!(
		lines[5], "// flags      -       34..35       (added)",
		"{comment}"
	);
	assert!(
		!comment.contains("  \n"),
		"no row may end in trailing padding: {comment:?}"
	);
}

#[test]
fn offset_comment_pairs_a_renamed_field_under_its_new_name() {
	let stored = schema(
		LayoutKind::Fixed,
		&[("authority", "Address"), ("first_name", "String<8>")],
	);
	let destination = schema(
		LayoutKind::Fixed,
		&[("authority", "Address"), ("name", "String<20>")],
	);
	let renames = vec![pin_rename("first_name", "name")];
	let comment = layout_comment(&stored, &destination, &renames);
	assert!(
		comment.contains("// name       32..41  32..53"),
		"the renamed field shows its stored bytes under the new name: {comment}"
	);
	assert!(
		!comment.contains("first_name"),
		"the stored name must not appear as its own row: {comment}"
	);
}

#[test]
fn offset_comment_annotates_compact_tails() {
	let stored = schema(
		LayoutKind::Fixed,
		&[("owner", "Address"), ("balance", "u64")],
	);
	let destination = schema(
		LayoutKind::Compact,
		&[
			("owner", "Address"),
			("title", "String<12>"),
			("tags", "Vec<u16, 3>"),
		],
	);

	let comment = layout_comment(&stored, &destination, &[]);
	assert!(
		comment.contains("// balance  32..40  -            (removed)"),
		"{comment}"
	);
	assert!(
		comment.contains("(string prefix, capacity 12)"),
		"a string tail's prefix location and capacity are named: {comment}"
	);
	assert!(
		comment.contains("(vector prefix, capacity 3, 2-byte elements)"),
		"a vector tail's element width is named: {comment}"
	);
	// The title tail's prefix sits at payload offset 32: one presence byte the
	// conversion must read before its payload.
	assert!(
		comment.contains("title") && comment.contains("32..33"),
		"tail prefix ranges are exact: {comment}"
	);
}

#[test]
fn offset_comment_marks_optional_tails_and_header_fields() {
	let source = schema(
		LayoutKind::Compact,
		&[("title", "String<12>"), ("note", "Option<String<64>>")],
	);
	let destination = schema(
		LayoutKind::Compact,
		&[
			("title", "String<12>"),
			("note", "Option<String<64>>"),
			("tags", "Vec<u32, 5>"),
		],
	);
	let comment = layout_comment(&source, &destination, &[]);
	assert!(
		comment.contains("optional"),
		"an optional tail's presence byte is named: {comment}"
	);
	assert!(
		comment.contains("(vector prefix, capacity 5, 4-byte elements)"),
		"{comment}"
	);
}

/// The stub embeds the comment, so the developer opening the file sees the
/// offsets without running anything.
#[test]
fn manual_stub_embeds_the_offset_comment() {
	let account = ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap();
	let source = SchemaVersion {
		schema: schema(LayoutKind::Fixed, &[("value", "u64")]),
		process: None,
		transition: None,
	};
	let destination = schema(LayoutKind::Fixed, &[("value", "u32")]);
	let generated = manual_transition_source(
		&account,
		MigrationVersionType::U8,
		&source,
		0,
		1,
		&destination,
		&[],
	)
	.unwrap_or_else(|error| panic!("stub: {error:?}"));
	assert!(
		generated.contains("// Payload offsets, relative to the version envelope header:"),
		"{generated}"
	);
	assert!(generated.contains("// value  0..8    0..4"), "{generated}");
}

/// A hand-built schema the grammar rejects has no physical layout, so it
/// contributes no offset rows instead of failing the comment. `try_new` refuses
/// such a schema, which is why the generator's own schemas always carry one.
#[test]
fn offset_comment_skips_a_schema_the_grammar_rejects() {
	let unphysical = DataSchema {
		layout: LayoutKind::Fixed,
		fields: vec![FieldSchema {
			name: "value".to_owned(),
			rust_type: "NotAType".to_owned(),
		}],
		codec: pina_abi::DataCodec::PinaPodV2,
	};
	assert!(
		unphysical.physical().is_err(),
		"the fixture must be a schema the grammar rejects"
	);

	let valid = schema(LayoutKind::Fixed, &[("value", "u64")]);
	let comment = layout_comment(&unphysical, &valid, &[]);
	// The stored side contributes nothing, so the destination field reads as an
	// addition that the conversion must initialize.
	assert!(
		comment.contains("value"),
		"the destination rows still render: {comment}"
	);
	// With neither side physical there are no rows at all, so only the header
	// and its column labels remain.
	let header_only = layout_comment(&unphysical, &unphysical, &[]);
	assert_eq!(
		header_only.lines().count(),
		2,
		"the header and column row remain: {header_only}"
	);
	assert!(!header_only.contains("value"), "{header_only}");
}

/// A paired compact row describes the shape the conversion writes, not the one
/// it reads. Growing `String<12>` to `String<20>` keeps the stored range beside
/// the destination's, and the note must name the capacity the developer has to
/// write — the stored capacity would be a lie about the destination layout.
#[test]
fn offset_comment_prefers_the_destination_capacity_on_a_paired_row() {
	let stored = schema(LayoutKind::Compact, &[("title", "String<12>")]);
	let destination = schema(LayoutKind::Compact, &[("title", "String<20>")]);
	let comment = layout_comment(&stored, &destination, &[]);
	assert!(
		comment.contains("(string prefix, capacity 20)"),
		"the destination capacity is the one to write: {comment}"
	);
	assert!(
		!comment.contains("capacity 12"),
		"the stored capacity must not be offered as the destination shape: {comment}"
	);

	// A paired fixed row has a note on neither side, so none is fabricated.
	let fixed_stored = schema(LayoutKind::Fixed, &[("value", "u64")]);
	let fixed_destination = schema(LayoutKind::Fixed, &[("value", "u32"), ("added", "u8")]);
	let fixed_comment = layout_comment(&fixed_stored, &fixed_destination, &[]);
	assert!(
		!fixed_comment.contains("(string prefix"),
		"fixed rows carry no compact note: {fixed_comment}"
	);
}
