//! Checked-in ABI snapshots and adjacent migration generation.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read as _;
use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;

use atomic_write_file::AtomicWriteFile;
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
use pina_abi::ProcessTransition;
use pina_abi::PublicationLedger;
use pina_abi::PublicationReceipt;
use pina_abi::SchemaVersion;
use pina_abi::Transition;
use pina_abi::TransitionMode;
use serde::Serialize;
use sha2::Digest as _;
use sha2::Sha256;

use crate::error::IdlError;
use crate::ir::DefaultValueIr;
use crate::ir::DiscriminatorIr;
use crate::ir::InstructionIr;
use crate::parse;
use crate::project::Project;
use crate::project::ProjectError;

/// One source contract discovered during migration inspection.
#[derive(Clone, Debug, PartialEq, Eq)]
struct CurrentContract {
	identity: ContractIdentity,
	rust_name: String,
	schema: DataSchema,
	process: Option<ProcessContract>,
}

/// Result of creating or refreshing draft migrations.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MakeMigrationsOutput {
	pub manifest: PathBuf,
	pub created_contracts: Vec<String>,
	pub advanced_versions: Vec<String>,
	pub updated_drafts: Vec<String>,
	pub unchanged_contracts: Vec<String>,
	pub manual_transitions: Vec<PathBuf>,
}

/// Current status of one migration-aware wire contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationStatus {
	pub identity: String,
	pub kind: String,
	pub rust_name: String,
	pub current_version: u32,
	pub published: bool,
	pub schema_sha256: String,
}

/// Current version constants required to serialize the latest public IDL.
pub(crate) struct IdlMigrationMetadata {
	pub version_type: MigrationVersionType,
	pub current_versions: BTreeMap<String, u32>,
}

/// Errors produced by migration snapshot and compatibility operations.
#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
	#[error(transparent)]
	Project(#[from] ProjectError),

	#[error(transparent)]
	Parse(#[from] IdlError),

	#[error("Could not read migration file {path}: {source}")]
	Read {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Could not decode migration file {path}: {reason}")]
	InvalidDocument { path: PathBuf, reason: String },

	#[error("Could not serialize migration file {path}: {source}")]
	SerializeJson {
		path: PathBuf,
		source: serde_json::Error,
	},

	#[error("Could not create migration directory {path}: {source}")]
	CreateDirectory {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Could not write migration file {path}: {source}")]
	Write {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Invalid migration history: {0}")]
	InvalidHistory(String),

	#[error("Migration history belongs to program {found}, but current source declares {expected}")]
	ProgramIdentityChanged { expected: String, found: String },

	#[error("Migration version encoding is frozen as {found}, but pina.toml configures {expected}")]
	VersionTypeChanged { expected: String, found: String },

	#[error(
		"Instruction process `{name}` changed incompatibly under discriminator `{identity}`: \
		 {reason}. Pina currently preserves an unchanged positional prefix and permits only newly \
		 appended optional accounts. Use a new instruction discriminator for this process change."
	)]
	ProcessChanged {
		name: String,
		identity: String,
		reason: String,
	},

	#[error(
		"Migration-aware {kind} `{name}` has no checked-in snapshot. Run `pina migrations make`."
	)]
	MissingSnapshot { kind: String, name: String },

	#[error(
		"Migration-aware {kind} `{name}` differs from version {version}. Its data schema or \
		 instruction process ABI changed. Run `pina migrations make` and review the transition."
	)]
	SchemaDrift {
		kind: String,
		name: String,
		version: u32,
	},

	#[error(
		"Migration implementation {path} differs from the recorded hash for {kind} `{name}` \
		 version {version}. Run `pina migrations make` and review the transition."
	)]
	TransitionDrift {
		kind: String,
		name: String,
		version: u32,
		path: PathBuf,
	},

	#[error(
		"Historical {kind} contract `{name}` ({identity}) is no longer present in source. \
		 Published decoders and account migrations cannot be silently removed. Restore it or \
		 perform an explicitly reviewed retirement."
	)]
	ContractRemoved {
		kind: String,
		name: String,
		identity: String,
	},

	#[error("Configured {version_type} migration versions are exhausted for `{identity}`")]
	VersionExhausted {
		version_type: String,
		identity: String,
	},

	#[error(
		"Published migration implementation {path} changed after publication. Add a repair \
		 migration instead of rewriting live history."
	)]
	PublishedImplementationChanged { path: PathBuf },

	#[error(
		"Manual migration {path} is unfinished. Replace the `TODO(pina-manual-migration)` body \
		 and run `pina migrations make`."
	)]
	ManualTransitionIncomplete { path: PathBuf },

	#[error("Migration transition file is missing: {path}")]
	MissingTransition { path: PathBuf },

	#[error("Multiple migration-aware source contracts resolve to `{identity}`")]
	DuplicateIdentity { identity: String },

	#[error("Migration path is a symbolic link or reparse point: {path}")]
	UnsafePath { path: PathBuf },

	#[error("Could not lock migration history {path}: {source}")]
	Lock {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Deployment program ID {deployed} does not match migration history {manifest}")]
	PublicationProgramMismatch { deployed: String, manifest: String },

	#[error("Deployed artifact changed before its migration publication was recorded: {path}")]
	PublicationArtifactChanged { path: PathBuf },

	#[error("Publication receipt sequence exceeded u64")]
	PublicationSequenceExhausted,
}

/// Create the initial ABI database or refresh its latest draft versions.
///
/// An unpublished latest version is mutable. A published latest version is
/// immutable and a source change appends one adjacent version.
pub fn make_migrations(start: &Path) -> Result<MakeMigrationsOutput, MigrationError> {
	let project = Project::discover(start)?;
	let _lock = acquire_migration_lock(&project.program_dir)?;
	let current = scan_current_contracts(&project)?;
	let manifest_path = project.program_dir.join(MANIFEST_PATH);
	let publication_path = project.program_dir.join(PUBLICATIONS_PATH);
	let ledger = load_publication_ledger(&publication_path)?;
	let mut manifest = load_manifest(&manifest_path)?.unwrap_or_else(|| {
		MigrationManifest::new(current.program_id.clone(), project.migration_version_type)
	});
	validate_program_configuration(&project, &current.program_id, &manifest)?;
	validate_ledger_for_manifest(&ledger, &manifest)?;

	let mut output = MakeMigrationsOutput {
		manifest: manifest_path.clone(),
		..MakeMigrationsOutput::default()
	};
	let mut seen = BTreeMap::new();

	for source in current.contracts {
		let key = source.identity.key();
		seen.insert(
			key.clone(),
			(source.identity.kind, source.rust_name.clone()),
		);
		match manifest.contracts.get_mut(&key) {
			None => {
				let schema_sha256 = source.schema.sha256();
				let process_sha256 = source.process.as_ref().map(ProcessContract::sha256);
				manifest.contracts.insert(
					key.clone(),
					ContractHistory {
						identity: source.identity,
						rust_name: source.rust_name,
						versions: vec![SchemaVersion {
							version: 0,
							schema_sha256,
							schema: source.schema,
							process: source.process,
							process_sha256,
							transition: None,
						}],
					},
				);
				output.created_contracts.push(key);
			}
			Some(history) => {
				history.rust_name = source.rust_name;
				let latest = history.current().ok_or_else(|| {
					MigrationError::InvalidHistory(format!("contract `{key}` has no versions"))
				})?;
				if latest.schema == source.schema && latest.process == source.process {
					refresh_draft_transition_hash(&project, &ledger, &key, history, &mut output)?;
					output.unchanged_contracts.push(key);
					continue;
				}

				let latest_version = latest.version;
				if ledger.ever_published(&key, latest_version) {
					if latest_version == manifest.version_type.max_version() {
						return Err(MigrationError::VersionExhausted {
							version_type: manifest.version_type.to_string(),
							identity: key,
						});
					}
					let next = latest_version + 1;
					let transition = create_transition(
						&project,
						TransitionRequest {
							identity: &history.identity,
							rust_name: &history.rust_name,
							source: latest,
							destination_version: next,
							destination: &source.schema,
							destination_process: source.process.as_ref(),
							preserve_manual: false,
						},
						&mut output,
					)?;
					let schema_sha256 = source.schema.sha256();
					history.versions.push(SchemaVersion {
						version: next,
						schema_sha256,
						schema: source.schema,
						process_sha256: source.process.as_ref().map(ProcessContract::sha256),
						process: source.process,
						transition: Some(transition),
					});
					output.advanced_versions.push(format!("{key}@{next}"));
				} else {
					let replacement = if latest_version == 0 {
						SchemaVersion {
							version: 0,
							schema_sha256: source.schema.sha256(),
							schema: source.schema,
							process_sha256: source.process.as_ref().map(ProcessContract::sha256),
							process: source.process,
							transition: None,
						}
					} else {
						let previous = history
							.versions
							.get((latest_version - 1) as usize)
							.ok_or_else(|| {
								MigrationError::InvalidHistory(format!(
									"contract `{key}` has no previous version"
								))
							})?;
						let transition = create_transition(
							&project,
							TransitionRequest {
								identity: &history.identity,
								rust_name: &history.rust_name,
								source: previous,
								destination_version: latest_version,
								destination: &source.schema,
								destination_process: source.process.as_ref(),
								preserve_manual: true,
							},
							&mut output,
						)?;
						SchemaVersion {
							version: latest_version,
							schema_sha256: source.schema.sha256(),
							schema: source.schema,
							process_sha256: source.process.as_ref().map(ProcessContract::sha256),
							process: source.process,
							transition: Some(transition),
						}
					};
					let index = latest_version as usize;
					history.versions[index] = replacement;
					output
						.updated_drafts
						.push(format!("{key}@{latest_version}"));
				}
			}
		}
	}

	for (key, history) in &manifest.contracts {
		if !seen.contains_key(key) {
			return Err(MigrationError::ContractRemoved {
				kind: history.identity.kind.to_string(),
				name: history.rust_name.clone(),
				identity: key.clone(),
			});
		}
	}

	manifest
		.validate()
		.map_err(MigrationError::InvalidHistory)?;
	write_json_atomic(&manifest_path, &manifest)?;
	write_json_atomic(&publication_path, &ledger)?;
	Ok(output)
}

/// Verify source, snapshots, process contracts, and frozen transition code.
pub fn check_migrations(start: &Path) -> Result<Vec<MigrationStatus>, MigrationError> {
	let project = Project::discover(start)?;
	check_project_migrations(&project)
}

pub(crate) fn check_project_migrations(
	project: &Project,
) -> Result<Vec<MigrationStatus>, MigrationError> {
	let current = scan_current_contracts(project)?;
	let manifest_path = project.program_dir.join(MANIFEST_PATH);
	let manifest = load_manifest(&manifest_path)?;
	if current.contracts.is_empty() && manifest.is_none() {
		return Ok(Vec::new());
	}
	let manifest = manifest.ok_or_else(|| {
		let first = &current.contracts[0];
		MigrationError::MissingSnapshot {
			kind: first.identity.kind.to_string(),
			name: first.rust_name.clone(),
		}
	})?;
	let ledger = load_publication_ledger(&project.program_dir.join(PUBLICATIONS_PATH))?;
	validate_program_configuration(project, &current.program_id, &manifest)?;
	manifest
		.validate()
		.map_err(MigrationError::InvalidHistory)?;
	validate_ledger_for_manifest(&ledger, &manifest)?;

	let mut seen = BTreeMap::new();
	let mut statuses = Vec::new();
	for source in current.contracts {
		let key = source.identity.key();
		seen.insert(
			key.clone(),
			(source.identity.kind, source.rust_name.clone()),
		);
		let history = manifest.contracts.get(&key).ok_or_else(|| {
			MigrationError::MissingSnapshot {
				kind: source.identity.kind.to_string(),
				name: source.rust_name.clone(),
			}
		})?;
		let latest = history.current().ok_or_else(|| {
			MigrationError::InvalidHistory(format!("contract `{key}` has no versions"))
		})?;
		if latest.schema != source.schema || latest.process != source.process {
			return Err(MigrationError::SchemaDrift {
				kind: source.identity.kind.to_string(),
				name: source.rust_name,
				version: latest.version,
			});
		}
		verify_transition_files(project, &ledger, &key, history)?;
		statuses.push(MigrationStatus {
			identity: key.clone(),
			kind: source.identity.kind.to_string(),
			rust_name: source.rust_name,
			current_version: latest.version,
			published: ledger.ever_published(&key, latest.version),
			schema_sha256: latest.schema_sha256.clone(),
		});
	}
	for (key, history) in &manifest.contracts {
		if !seen.contains_key(key) {
			return Err(MigrationError::ContractRemoved {
				kind: history.identity.kind.to_string(),
				name: history.rust_name.clone(),
				identity: key.clone(),
			});
		}
	}
	Ok(statuses)
}

/// Return migration status after applying every build-time compatibility check.
pub fn migration_status(start: &Path) -> Result<Vec<MigrationStatus>, MigrationError> {
	check_migrations(start)
}

/// Verify history and return only the current constants needed by IDL codegen.
pub(crate) fn idl_migration_metadata(
	start: &Path,
) -> Result<Option<IdlMigrationMetadata>, MigrationError> {
	let project = Project::discover(start)?;
	let statuses = check_project_migrations(&project)?;
	if statuses.is_empty() {
		return Ok(None);
	}

	Ok(Some(IdlMigrationMetadata {
		version_type: project.migration_version_type,
		current_versions: statuses
			.into_iter()
			.map(|status| (status.identity, status.current_version))
			.collect(),
	}))
}

/// Append one receipt after a successful persistent deployment.
///
/// Local deployments must not call this function. The expected digest comes
/// from the immutable deployment plan and is compared with the artifact again
/// before any version is marked published.
pub fn record_publication(
	start: &Path,
	cluster: &str,
	rpc_url: &str,
	deployed_program_id: &str,
	artifact: &Path,
	expected_artifact_digest: [u8; 32],
) -> Result<Option<PublicationReceipt>, MigrationError> {
	let project = Project::discover(start)?;
	let _lock = acquire_migration_lock(&project.program_dir)?;
	let statuses = check_migrations(&project.program_dir)?;
	if statuses.is_empty() {
		return Ok(None);
	}
	let manifest_path = project.program_dir.join(MANIFEST_PATH);
	let manifest = load_manifest(&manifest_path)?.ok_or_else(|| {
		MigrationError::InvalidHistory(
			"migration manifest disappeared during publication".to_owned(),
		)
	})?;
	if manifest.program_id != deployed_program_id {
		return Err(MigrationError::PublicationProgramMismatch {
			deployed: deployed_program_id.to_owned(),
			manifest: manifest.program_id,
		});
	}
	let publication_path = project.program_dir.join(PUBLICATIONS_PATH);
	let mut ledger = load_publication_ledger(&publication_path)?;
	validate_ledger_for_manifest(&ledger, &manifest)?;
	let artifact_digest = hash_regular_file(artifact)?;
	if artifact_digest != expected_artifact_digest {
		return Err(MigrationError::PublicationArtifactChanged {
			path: artifact.to_path_buf(),
		});
	}
	let versions = statuses
		.into_iter()
		.map(|status| (status.identity, status.current_version))
		.collect();
	let sequence = u64::try_from(ledger.receipts.len())
		.map_err(|_| MigrationError::PublicationSequenceExhausted)?;
	let receipt = PublicationReceipt {
		sequence,
		cluster: cluster.to_owned(),
		rpc_url: rpc_url.to_owned(),
		program_id: deployed_program_id.to_owned(),
		executable_sha256: hex_digest(artifact_digest),
		manifest_sha256: manifest.sha256(),
		versions,
		previous_receipt_sha256: ledger.receipts.last().map(PublicationReceipt::sha256),
	};
	ledger.receipts.push(receipt.clone());
	validate_ledger_for_manifest(&ledger, &manifest)?;
	write_json_atomic(&publication_path, &ledger)?;
	Ok(Some(receipt))
}

fn validate_ledger_for_manifest(
	ledger: &PublicationLedger,
	manifest: &MigrationManifest,
) -> Result<(), MigrationError> {
	ledger.validate().map_err(MigrationError::InvalidHistory)?;
	for receipt in &ledger.receipts {
		if receipt.program_id != manifest.program_id {
			return Err(MigrationError::InvalidHistory(format!(
				"publication receipt {} belongs to program {}, expected {}",
				receipt.sequence, receipt.program_id, manifest.program_id
			)));
		}
		for (key, published) in &receipt.versions {
			let history = manifest.contracts.get(key).ok_or_else(|| {
				MigrationError::InvalidHistory(format!(
					"publication receipt {} names unknown contract `{key}`",
					receipt.sequence
				))
			})?;
			let current = history.current().ok_or_else(|| {
				MigrationError::InvalidHistory(format!("contract `{key}` has no versions"))
			})?;
			if *published > current.version {
				return Err(MigrationError::InvalidHistory(format!(
					"publication receipt {} claims future version {} for `{key}`",
					receipt.sequence, published
				)));
			}
		}
	}
	Ok(())
}

struct CurrentProgram {
	program_id: String,
	contracts: Vec<CurrentContract>,
}

fn scan_current_contracts(project: &Project) -> Result<CurrentProgram, MigrationError> {
	let ir = parse::parse_program(&project.program_dir, Some(&project.library_name))?;
	let src_dir = project.program_dir.join("src");
	let files = parse::module_resolver::resolve_crate(&src_dir, &src_dir.join("lib.rs"))?;
	let mut discriminators = Vec::new();
	let mut accounts = Vec::new();
	let mut instructions = Vec::new();
	let mut events = Vec::new();
	for resolved in &files {
		discriminators.extend(parse::discriminator::extract_discriminator_enums(
			&resolved.file,
		)?);
		accounts.extend(parse::account_state::extract_account_structs(
			&resolved.file,
		)?);
		instructions.extend(parse::instruction_data::extract_instruction_structs(
			&resolved.file,
		)?);
		events.extend(parse::event_data::extract_migratable_events(
			&resolved.file,
		)?);
	}
	let discriminator_map = parse::build_discriminator_map(&discriminators);
	let mut contracts = BTreeMap::new();

	for account in accounts.into_iter().filter(|account| account.migratable) {
		let discriminator = resolve_discriminator(
			&discriminator_map,
			&account.discriminator_enum,
			&account.variant,
			"account",
		)?;
		let identity = ContractIdentity::try_new(
			ContractKind::Account,
			discriminator.repr_size,
			discriminator.value,
		)
		.map_err(MigrationError::InvalidHistory)?;
		let layout = if account.is_compact() {
			LayoutKind::Compact
		} else {
			LayoutKind::Fixed
		};
		let fields = account
			.fields
			.into_iter()
			.map(|field| {
				FieldSchema {
					name: field.name,
					rust_type: field.rust_type,
				}
			})
			.collect();
		let schema = DataSchema::try_new(layout, fields).map_err(MigrationError::InvalidHistory)?;
		insert_current(
			&mut contracts,
			CurrentContract {
				identity,
				rust_name: account.name,
				schema,
				process: None,
			},
		)?;
	}

	for instruction in instructions
		.into_iter()
		.filter(|instruction| instruction.migratable)
	{
		let discriminator = resolve_discriminator(
			&discriminator_map,
			&instruction.discriminator_enum,
			&instruction.variant,
			"instruction",
		)?;
		let identity = ContractIdentity::try_new(
			ContractKind::Instruction,
			discriminator.repr_size,
			discriminator.value,
		)
		.map_err(MigrationError::InvalidHistory)?;
		let ir_instruction =
			find_instruction(&ir.instructions, discriminator).ok_or_else(|| {
				MigrationError::InvalidHistory(format!(
					"could not resolve process contract for instruction `{}`",
					instruction.name
				))
			})?;
		let fields = instruction
			.fields
			.into_iter()
			.map(|field| {
				FieldSchema {
					name: field.name,
					rust_type: field.rust_type,
				}
			})
			.collect();
		let schema = DataSchema::try_new(LayoutKind::Fixed, fields)
			.map_err(MigrationError::InvalidHistory)?;
		insert_current(
			&mut contracts,
			CurrentContract {
				identity,
				rust_name: instruction.name,
				schema,
				process: Some(process_contract(ir_instruction)),
			},
		)?;
	}

	for event in events {
		let discriminator = resolve_discriminator(
			&discriminator_map,
			&event.discriminator_enum,
			&event.variant,
			"event",
		)?;
		let identity = ContractIdentity::try_new(
			ContractKind::Event,
			discriminator.repr_size,
			discriminator.value,
		)
		.map_err(MigrationError::InvalidHistory)?;
		insert_current(
			&mut contracts,
			CurrentContract {
				identity,
				rust_name: event.name,
				schema: event.schema,
				process: None,
			},
		)?;
	}

	Ok(CurrentProgram {
		program_id: ir.public_key,
		contracts: contracts.into_values().collect(),
	})
}

fn resolve_discriminator<'a>(
	map: &'a std::collections::HashMap<(String, String), DiscriminatorIr>,
	enum_name: &str,
	variant: &str,
	kind: &str,
) -> Result<&'a DiscriminatorIr, MigrationError> {
	map.get(&(enum_name.to_owned(), variant.to_owned()))
		.ok_or_else(|| {
			MigrationError::InvalidHistory(format!(
				"could not resolve {kind} discriminator `{enum_name}::{variant}`"
			))
		})
}

fn insert_current(
	contracts: &mut BTreeMap<String, CurrentContract>,
	contract: CurrentContract,
) -> Result<(), MigrationError> {
	let key = contract.identity.key();
	if contracts.insert(key.clone(), contract).is_some() {
		return Err(MigrationError::DuplicateIdentity { identity: key });
	}
	Ok(())
}

fn find_instruction<'a>(
	instructions: &'a [InstructionIr],
	discriminator: &DiscriminatorIr,
) -> Option<&'a InstructionIr> {
	instructions.iter().find(|instruction| {
		instruction.discriminator.value == discriminator.value
			&& instruction.discriminator.repr_size == discriminator.repr_size
	})
}

fn process_contract(instruction: &InstructionIr) -> ProcessContract {
	ProcessContract {
		accounts: instruction
			.accounts
			.iter()
			.map(|account| {
				ProcessAccount {
					name: account.name.clone(),
					writable: account.is_writable,
					signer: account.is_signer,
					optional: account.is_optional,
					default_value: account.default_value.as_ref().map(|value| {
						match value {
							DefaultValueIr::ProgramId(value) => format!("program:{value}"),
							DefaultValueIr::PublicKey(value) => format!("publicKey:{value}"),
						}
					}),
					pda: account.pda_name.clone(),
					constraints: account.constraints.clone(),
				}
			})
			.collect(),
	}
}

fn validate_program_configuration(
	project: &Project,
	program_id: &str,
	manifest: &MigrationManifest,
) -> Result<(), MigrationError> {
	if manifest.program_id != program_id {
		return Err(MigrationError::ProgramIdentityChanged {
			expected: program_id.to_owned(),
			found: manifest.program_id.clone(),
		});
	}
	if manifest.version_type != project.migration_version_type {
		return Err(MigrationError::VersionTypeChanged {
			expected: project.migration_version_type.to_string(),
			found: manifest.version_type.to_string(),
		});
	}
	Ok(())
}

fn load_manifest(path: &Path) -> Result<Option<MigrationManifest>, MigrationError> {
	if !path.exists() {
		return Ok(None);
	}
	let source = read_bytes(path)?;
	pina_abi::decode_manifest(&source)
		.map(Some)
		.map_err(|reason| {
			MigrationError::InvalidDocument {
				path: path.to_path_buf(),
				reason,
			}
		})
}

fn load_publication_ledger(path: &Path) -> Result<PublicationLedger, MigrationError> {
	if !path.exists() {
		return Ok(PublicationLedger::default());
	}
	let source = read_bytes(path)?;
	pina_abi::decode_publication_ledger(&source).map_err(|reason| {
		MigrationError::InvalidDocument {
			path: path.to_path_buf(),
			reason,
		}
	})
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, MigrationError> {
	ensure_safe_path(path)?;
	std::fs::read(path).map_err(|source| {
		MigrationError::Read {
			path: path.to_path_buf(),
			source,
		}
	})
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<(), MigrationError> {
	ensure_safe_path(path)?;
	let parent = path.parent().ok_or_else(|| {
		MigrationError::InvalidHistory(format!("{} has no parent directory", path.display()))
	})?;
	std::fs::create_dir_all(parent).map_err(|source| {
		MigrationError::CreateDirectory {
			path: parent.to_path_buf(),
			source,
		}
	})?;
	ensure_safe_path(path)?;
	let mut bytes = Vec::new();
	let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
	let mut serializer = serde_json::Serializer::with_formatter(&mut bytes, formatter);
	value.serialize(&mut serializer).map_err(|source| {
		MigrationError::SerializeJson {
			path: path.to_path_buf(),
			source,
		}
	})?;
	bytes.push(b'\n');
	write_atomic(path, &bytes)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), MigrationError> {
	ensure_safe_path(path)?;
	let mut file = AtomicWriteFile::open(path).map_err(|source| {
		MigrationError::Write {
			path: path.to_path_buf(),
			source,
		}
	})?;
	file.write_all(bytes).map_err(|source| {
		MigrationError::Write {
			path: path.to_path_buf(),
			source,
		}
	})?;
	file.commit().map_err(|source| {
		MigrationError::Write {
			path: path.to_path_buf(),
			source,
		}
	})
}

#[derive(Debug)]
struct MigrationLock(File);

impl Drop for MigrationLock {
	fn drop(&mut self) {
		let _ = fs2::FileExt::unlock(&self.0);
	}
}

fn acquire_migration_lock(program_dir: &Path) -> Result<MigrationLock, MigrationError> {
	let directory = program_dir.join("migrations");
	ensure_safe_path(&directory)?;
	std::fs::create_dir_all(&directory).map_err(|source| {
		MigrationError::CreateDirectory {
			path: directory.clone(),
			source,
		}
	})?;
	let path = directory.join(".lock");
	ensure_safe_path(&path)?;
	let file = OpenOptions::new()
		.read(true)
		.write(true)
		.create(true)
		.truncate(false)
		.open(&path)
		.map_err(|source| {
			MigrationError::Lock {
				path: path.clone(),
				source,
			}
		})?;
	fs2::FileExt::lock_exclusive(&file).map_err(|source| MigrationError::Lock { path, source })?;
	Ok(MigrationLock(file))
}

fn ensure_safe_path(path: &Path) -> Result<(), MigrationError> {
	if crate::path_security::has_link_like_component(path).map_err(|source| {
		MigrationError::Read {
			path: path.to_path_buf(),
			source,
		}
	})? {
		return Err(MigrationError::UnsafePath {
			path: path.to_path_buf(),
		});
	}
	Ok(())
}

fn hash_regular_file(path: &Path) -> Result<[u8; 32], MigrationError> {
	ensure_safe_path(path)?;
	let metadata = std::fs::symlink_metadata(path).map_err(|source| {
		MigrationError::Read {
			path: path.to_path_buf(),
			source,
		}
	})?;
	if !metadata.is_file() {
		return Err(MigrationError::InvalidHistory(format!(
			"publication artifact {} is not a regular file",
			path.display()
		)));
	}
	let mut file = File::open(path).map_err(|source| {
		MigrationError::Read {
			path: path.to_path_buf(),
			source,
		}
	})?;
	let mut digest = Sha256::new();
	let mut buffer = vec![0_u8; 64 * 1024];
	loop {
		let read = file.read(&mut buffer).map_err(|source| {
			MigrationError::Read {
				path: path.to_path_buf(),
				source,
			}
		})?;
		if read == 0 {
			break;
		}
		digest.update(&buffer[..read]);
	}
	Ok(digest.finalize().into())
}

fn hex_digest(digest: [u8; 32]) -> String {
	let mut output = String::with_capacity(64);
	for byte in digest {
		let _ = write!(output, "{byte:02x}");
	}
	output
}

#[derive(Clone, Copy)]
struct TransitionRequest<'a> {
	identity: &'a ContractIdentity,
	rust_name: &'a str,
	source: &'a SchemaVersion,
	destination_version: u32,
	destination: &'a DataSchema,
	destination_process: Option<&'a ProcessContract>,
	preserve_manual: bool,
}

fn create_transition(
	project: &Project,
	request: TransitionRequest<'_>,
	output: &mut MakeMigrationsOutput,
) -> Result<Transition, MigrationError> {
	let TransitionRequest {
		identity,
		rust_name,
		source,
		destination_version,
		destination,
		destination_process,
		preserve_manual,
	} = request;
	let process = process_transition(
		identity,
		rust_name,
		source.process.as_ref(),
		destination_process,
	)?;
	let mode = transition_mode(&source.schema, destination);
	let path = transition_path(project, identity, source.version, destination_version);
	let generated = match mode {
		TransitionMode::Automatic => {
			automatic_transition_source(
				identity,
				project.migration_version_type,
				source,
				destination_version,
				destination,
			)
		}
		TransitionMode::Manual => {
			manual_transition_source(
				identity,
				project.migration_version_type,
				source,
				destination_version,
				destination,
			)
		}
	};
	if mode == TransitionMode::Manual && preserve_manual && path.exists() {
		// Keep a developer-owned draft body while its destination snapshot evolves.
	} else {
		let parent = path.parent().expect("transition path always has parent");
		std::fs::create_dir_all(parent).map_err(|source| {
			MigrationError::CreateDirectory {
				path: parent.to_path_buf(),
				source,
			}
		})?;
		write_atomic(&path, generated.as_bytes())?;
	}
	if mode == TransitionMode::Manual {
		output.manual_transitions.push(path.clone());
	}
	let implementation_sha256 = Some(hash_transition_file(&path)?);
	Ok(Transition {
		from: source.version,
		to: destination_version,
		mode,
		source_schema_sha256: source.schema_sha256.clone(),
		destination_schema_sha256: destination.sha256(),
		source_process_sha256: source.process_sha256.clone(),
		destination_process_sha256: destination_process.map(ProcessContract::sha256),
		process,
		implementation_sha256,
	})
}

fn process_transition(
	identity: &ContractIdentity,
	rust_name: &str,
	source: Option<&ProcessContract>,
	destination: Option<&ProcessContract>,
) -> Result<Option<ProcessTransition>, MigrationError> {
	match (identity.kind, source, destination) {
		(ContractKind::Instruction, Some(source), Some(destination)) => {
			pina_abi::classify_process_transition(source, destination)
				.map(Some)
				.map_err(|reason| {
					MigrationError::ProcessChanged {
						name: rust_name.to_owned(),
						identity: identity.key(),
						reason,
					}
				})
		}
		(ContractKind::Instruction, ..) => {
			Err(MigrationError::InvalidHistory(format!(
				"instruction contract `{}` is missing a process snapshot",
				identity.key()
			)))
		}
		(ContractKind::Account | ContractKind::Event, None, None) => Ok(None),
		(ContractKind::Account | ContractKind::Event, ..) => {
			Err(MigrationError::InvalidHistory(format!(
				"{} contract `{}` unexpectedly contains a process snapshot",
				identity.kind,
				identity.key()
			)))
		}
	}
}

fn transition_mode(source: &DataSchema, destination: &DataSchema) -> TransitionMode {
	if source.layout != LayoutKind::Fixed || destination.layout != LayoutKind::Fixed {
		return TransitionMode::Manual;
	}
	let destination_fields = destination
		.fields
		.iter()
		.map(|field| (field.name.as_str(), field.rust_type.as_str()))
		.collect::<BTreeMap<_, _>>();
	let compatible = source.fields.iter().all(|field| {
		destination_fields
			.get(field.name.as_str())
			.is_none_or(|destination_type| **destination_type == field.rust_type)
	});
	if compatible
		&& source.fixed_payload_size().is_some()
		&& destination.fixed_payload_size().is_some()
		&& automatic_direction(source, destination).is_some()
	{
		TransitionMode::Automatic
	} else {
		TransitionMode::Manual
	}
}

#[derive(Clone, Copy)]
enum MoveDirection {
	Forward,
	Backward,
}

fn automatic_direction(source: &DataSchema, destination: &DataSchema) -> Option<MoveDirection> {
	let source_offsets = source.fixed_field_offsets()?;
	let destination_offsets = destination.fixed_field_offsets()?;
	let destination_types = destination
		.fields
		.iter()
		.map(|field| (field.name.as_str(), field.rust_type.as_str()))
		.collect::<BTreeMap<_, _>>();
	let mut previous_destination = None;
	let mut moves_left = false;
	let mut moves_right = false;
	for field in &source.fields {
		if destination_types.get(field.name.as_str()) != Some(&field.rust_type.as_str()) {
			continue;
		}
		let (source_offset, _) = source_offsets.get(&field.name)?;
		let (destination_offset, _) = destination_offsets.get(&field.name)?;
		if previous_destination.is_some_and(|previous| *destination_offset < previous) {
			return None;
		}
		previous_destination = Some(*destination_offset);
		moves_left |= destination_offset < source_offset;
		moves_right |= destination_offset > source_offset;
	}
	match (moves_left, moves_right) {
		(true, true) => None,
		(false, true) => Some(MoveDirection::Backward),
		(true | false, false) => Some(MoveDirection::Forward),
	}
}

fn automatic_transition_source(
	identity: &ContractIdentity,
	version_type: MigrationVersionType,
	source: &SchemaVersion,
	destination_version: u32,
	destination: &DataSchema,
) -> String {
	let discriminator_bytes = usize::from(identity.discriminator_bytes);
	let header = discriminator_bytes + version_type.bytes();
	let source_size = header + source.schema.fixed_payload_size().unwrap_or(0);
	let destination_size = header + destination.fixed_payload_size().unwrap_or(0);
	let working_size = source_size.max(destination_size);
	let source_offsets = source.schema.fixed_field_offsets().unwrap_or_default();
	let destination_offsets = destination.fixed_field_offsets().unwrap_or_default();
	let source_types = source
		.schema
		.fields
		.iter()
		.map(|field| (field.name.as_str(), field.rust_type.as_str()))
		.collect::<BTreeMap<_, _>>();
	let direction = automatic_direction(&source.schema, destination)
		.expect("automatic transition must have a safe move direction");
	let mut mapped = destination
		.fields
		.iter()
		.filter_map(|field| {
			if source_types.get(field.name.as_str()) != Some(&field.rust_type.as_str()) {
				return None;
			}
			let &(source_offset, size) = source_offsets.get(&field.name)?;
			let &(destination_offset, _) = destination_offsets.get(&field.name)?;
			Some((source_offset, destination_offset, size))
		})
		.collect::<Vec<_>>();
	match direction {
		MoveDirection::Forward => mapped.sort_by_key(|(source, ..)| *source),
		MoveDirection::Backward => mapped.sort_by_key(|(source, ..)| std::cmp::Reverse(*source)),
	}
	let mut moves = String::new();
	for (source_offset, destination_offset, size) in mapped {
		let source_start = header + source_offset;
		let source_end = source_start + size;
		let destination_start = header + destination_offset;
		let _ = writeln!(
			moves,
			"\tdata.copy_within({source_start}..{source_end}, {destination_start});"
		);
	}
	let source_names = source_types.keys().copied().collect::<Vec<_>>();
	let mut zeroes = String::new();
	for field in &destination.fields {
		if source_names.contains(&field.name.as_str())
			&& source_types.get(field.name.as_str()) == Some(&field.rust_type.as_str())
		{
			continue;
		}
		let Some(&(destination_offset, size)) = destination_offsets.get(&field.name) else {
			continue;
		};
		let start = header + destination_offset;
		let end = start + size;
		let _ = writeln!(zeroes, "\tdata[{start}..{end}].fill(0);");
	}
	format!(
		"// @generated by `pina migrations make`; do not edit an automatic \
		 transition.\npub(crate) const FROM_VERSION: u32 = {};\npub(crate) const TO_VERSION: u32 \
		 = {destination_version};\npub(crate) const SOURCE_SIZE: usize = \
		 {source_size};\npub(crate) const DESTINATION_SIZE: usize = \
		 {destination_size};\n\npub(crate) const WORKING_SIZE: usize = \
		 {working_size};\n\npub(crate) fn migrate(data: &mut [u8]) {{\n\tif data.len() < \
		 WORKING_SIZE {{\n\t\treturn;\n\t}}\n{moves}{zeroes}}}\n",
		source.version,
	)
}

fn manual_transition_source(
	identity: &ContractIdentity,
	version_type: MigrationVersionType,
	source: &SchemaVersion,
	destination_version: u32,
	destination: &DataSchema,
) -> String {
	let header = usize::from(identity.discriminator_bytes) + version_type.bytes();
	let source_size = source.schema.fixed_payload_size().map(|size| header + size);
	let destination_size = destination.fixed_payload_size().map(|size| header + size);
	let source_description =
		source_size.map_or_else(|| "variable".to_owned(), |size| size.to_string());
	let destination_description =
		destination_size.map_or_else(|| "variable".to_owned(), |size| size.to_string());
	let dynamic =
		source.schema.layout == LayoutKind::Compact || destination.layout == LayoutKind::Compact;
	let (requirement, sizing, migrate) = match identity.kind {
		ContractKind::Account => {
			let sizing = if dynamic {
				let target = destination_size.map_or_else(
					|| {
						"pub(crate) fn target_size(data: &[u8]) -> Option<usize> {\n\tlet _ = \
						 data;\n\tNone\n}\n"
							.to_owned()
					},
					|size| {
						format!(
							"pub(crate) fn target_size(_: &[u8]) -> Option<usize> \
							 {{\n\tSome({size})\n}}\n"
						)
					},
				);
				format!(
					"{target}\n#[allow(clippy::unnecessary_wraps)]\npub(crate) fn \
					 working_size(\n\tdata: &[u8],\n\ttarget_size: usize,\n) -> Option<usize> \
					 {{\n\tSome(data.len().max(target_size))\n}}\n"
				)
			} else {
				let source_size = source_size.expect("fixed source size");
				let destination_size = destination_size.expect("fixed destination size");
				let working_size = source_size.max(destination_size);
				format!(
					"pub(crate) const SOURCE_SIZE: usize = {source_size};\npub(crate) const \
					 DESTINATION_SIZE: usize = {destination_size};\npub(crate) const \
					 WORKING_SIZE: usize = {working_size};\n"
				)
			};
			(
				"the source shape is preflighted; this conversion must be total and fully \
				 initialize destination",
				sizing,
				"pub(crate) fn migrate(data: &mut [u8]) {\n\tlet _ = data;\n}\n",
			)
		}
		ContractKind::Instruction | ContractKind::Event => {
			let source_size = source_size.expect("instructions and events use fixed layouts");
			let destination_size =
				destination_size.expect("instructions and events use fixed layouts");
			let working_size = source_size.max(destination_size);
			(
				"validate the exact historical bytes, then fully initialize destination",
				format!(
					"pub(crate) const SOURCE_SIZE: usize = {source_size};\npub(crate) const \
					 DESTINATION_SIZE: usize = {destination_size};\npub(crate) const \
					 WORKING_SIZE: usize = {working_size};\n"
				),
				"pub(crate) fn migrate(data: &mut [u8]) -> bool {\n\tlet _ = data;\n\tfalse\n}\n",
			)
		}
	};
	format!(
		"// Manual adjacent ABI migration generated by `pina migrations make`.\n// Source \
		 version: {} ({source_description} bytes)\n// Destination version: {destination_version} \
		 ({destination_description} bytes)\n// TODO(pina-manual-migration): \
		 {requirement}.\n{sizing}\n{migrate}",
		source.version,
	)
}

fn transition_path(project: &Project, identity: &ContractIdentity, from: u32, to: u32) -> PathBuf {
	project
		.program_dir
		.join(pina_abi::transition_path(identity, from, to))
}

fn hash_transition_file(path: &Path) -> Result<String, MigrationError> {
	let source = std::fs::read_to_string(path).map_err(|source| {
		MigrationError::Read {
			path: path.to_path_buf(),
			source,
		}
	})?;
	// Rust normalizes CRLF to LF before tokenization. Hash the same canonical
	// source so one reviewed transition remains stable across Git checkouts.
	let canonical = source.replace("\r\n", "\n");
	Ok(pina_abi::sha256_bytes(canonical.as_bytes()))
}

fn verify_transition_files(
	project: &Project,
	ledger: &PublicationLedger,
	key: &str,
	history: &ContractHistory,
) -> Result<(), MigrationError> {
	for version in history.versions.iter().skip(1) {
		let transition = version.transition.as_ref().ok_or_else(|| {
			MigrationError::InvalidHistory(format!(
				"contract `{key}` version {} has no transition",
				version.version
			))
		})?;
		let path = transition_path(project, &history.identity, transition.from, transition.to);
		if !path.is_file() {
			return Err(MigrationError::MissingTransition { path });
		}
		let contents = std::fs::read_to_string(&path).map_err(|source| {
			MigrationError::Read {
				path: path.clone(),
				source,
			}
		})?;
		if transition.mode == TransitionMode::Manual
			&& contents.contains("TODO(pina-manual-migration)")
		{
			return Err(MigrationError::ManualTransitionIncomplete { path });
		}
		let current_hash = hash_transition_file(&path)?;
		if transition.implementation_sha256.as_deref() != Some(current_hash.as_str()) {
			if ledger.ever_published(key, version.version) {
				return Err(MigrationError::PublishedImplementationChanged { path });
			}
			return Err(MigrationError::TransitionDrift {
				kind: history.identity.kind.to_string(),
				name: history.rust_name.clone(),
				version: version.version,
				path,
			});
		}
	}
	Ok(())
}

fn refresh_draft_transition_hash(
	project: &Project,
	ledger: &PublicationLedger,
	key: &str,
	history: &mut ContractHistory,
	output: &mut MakeMigrationsOutput,
) -> Result<(), MigrationError> {
	let Some(latest) = history.versions.last_mut() else {
		return Ok(());
	};
	let Some(transition) = latest.transition.as_mut() else {
		return Ok(());
	};
	let path = transition_path(project, &history.identity, transition.from, transition.to);
	if !path.is_file() {
		return Err(MigrationError::MissingTransition { path });
	}
	let hash = hash_transition_file(&path)?;
	if transition.implementation_sha256.as_deref() == Some(hash.as_str()) {
		return Ok(());
	}
	if ledger.ever_published(key, latest.version) {
		return Err(MigrationError::PublishedImplementationChanged { path });
	}
	transition.implementation_sha256 = Some(hash);
	output
		.updated_drafts
		.push(format!("{key}@{}", latest.version));
	Ok(())
}

#[cfg(test)]
mod tests {
	use tempfile::TempDir;

	use super::*;

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
			manual_transition_source(&account, MigrationVersionType::U8, &source, 1, &destination);
		assert!(account_source.contains("fn migrate(data: &mut [u8]) {"));
		assert!(!account_source.contains("fn migrate(data: &mut [u8]) -> bool"));
		assert!(account_source.contains("conversion must be total"));

		let instruction_source = manual_transition_source(
			&instruction,
			MigrationVersionType::U8,
			&source,
			1,
			&destination,
		);
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
			manual_transition_source(&account, MigrationVersionType::U8, &source, 1, &destination);

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
		assert!(
			process_transition(&identity, "Transfer", Some(&source), Some(&destination),).is_ok()
		);

		let mut escalated = source.clone();
		escalated.accounts[0].writable = true;
		assert!(matches!(
			process_transition(&identity, "Transfer", Some(&source), Some(&escalated),),
			Err(MigrationError::ProcessChanged { .. })
		));
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
		let crlf = hash_transition_file(&source)
			.unwrap_or_else(|error| panic!("hash CRLF source: {error}"));

		assert_eq!(lf, crlf);
	}

	#[test]
	fn publication_records_exact_artifact_and_freezes_current_versions() {
		let fixture = publication_fixture();
		let digest: [u8; 32] = Sha256::digest(b"artifact").into();
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
		assert_eq!(receipt.versions.get("account:1:01"), Some(&0));
		let ledger = load_publication_ledger(&fixture.root.join(PUBLICATIONS_PATH))
			.unwrap_or_else(|error| panic!("reload publication: {error}"));
		assert!(ledger.ever_published("account:1:01", 0));

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
			.unwrap_or_else(|error| panic!("reload rejected publication: {error}"));
		assert_eq!(ledger.receipts.len(), 1);
	}

	struct PublicationFixture {
		_temp: TempDir,
		root: PathBuf,
		artifact: PathBuf,
		program_id: &'static str,
	}

	fn publication_fixture() -> PublicationFixture {
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
				"use pina::*;\ndeclare_id!(\"{PROGRAM_ID}\");\n#[discriminator]\nenum Kind {{ \
				 State = 1 }}\n#[account(discriminator = Kind::State, migrations)]\nstruct State \
				 {{ value: u64 }}\n"
			),
		)
		.unwrap_or_else(|error| panic!("write source: {error}"));
		let identity = ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap();
		let schema = schema(LayoutKind::Fixed, &[("value", "u64")]);
		let mut manifest = MigrationManifest::new(PROGRAM_ID.to_owned(), MigrationVersionType::U8);
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
			root.join(MANIFEST_PATH),
			serde_json::to_vec_pretty(&manifest)
				.unwrap_or_else(|error| panic!("serialize manifest: {error}")),
		)
		.unwrap_or_else(|error| panic!("write manifest: {error}"));
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

	#[test]
	fn field_type_changes_and_compact_changes_are_manual() {
		let old = schema(LayoutKind::Fixed, &[("count", "u64")]);
		let changed = schema(LayoutKind::Fixed, &[("count", "u32")]);
		let compact = schema(LayoutKind::Compact, &[("label", "String<8>")]);

		assert_eq!(transition_mode(&old, &changed), TransitionMode::Manual);
		assert_eq!(transition_mode(&old, &compact), TransitionMode::Manual);
	}
}
