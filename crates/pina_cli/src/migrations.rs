//! Checked-in ABI snapshots and adjacent migration generation.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::File;
use std::fs::OpenOptions;
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
use pina_abi::PendingPublication;
use pina_abi::ProcessAccount;
use pina_abi::ProcessContract;
use pina_abi::ProcessTransition;
use pina_abi::PublicationLedger;
use pina_abi::PublicationReceipt;
use pina_abi::PublishedContract;
use pina_abi::PublishedSchema;
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
	pub publication_pending: bool,
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
		"Frozen migration implementation {path} changed after publication became possible. Add a \
		 repair migration instead of rewriting possibly-live history."
	)]
	FrozenImplementationChanged { path: PathBuf },

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

	#[error(
		"Another deployment may already be live for {program_id} on {cluster}. Restore its exact \
		 inputs and rerun `pina deploy` before starting a different deployment."
	)]
	PublicationPending { program_id: String, cluster: String },

	#[error("No matching pending deployment exists to complete its publication receipt")]
	MissingPendingPublication,
}

/// Create the initial ABI database or refresh its latest draft versions.
///
/// An unfrozen latest version is mutable. A published or pending latest
/// version is immutable and a source change appends one adjacent version.
pub fn make_migrations(start: &Path) -> Result<MakeMigrationsOutput, MigrationError> {
	let project = Project::discover(start)?;
	let _lock = acquire_migration_lock(&project.program_dir)?;
	let current = scan_current_contracts(&project)?;
	let manifest_path = project.program_dir.join(MANIFEST_PATH);
	let publication_path = project.program_dir.join(PUBLICATIONS_PATH);
	let mut manifest = load_manifest(&manifest_path)?.unwrap_or_else(|| {
		MigrationManifest::new(current.program_id.clone(), project.migration_version_type)
	});
	let ledger = load_publication_ledger_for_manifest(&publication_path, &manifest)?;
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
				let latest = history
					.current()
					.expect("decoded migration histories always contain a current version");
				if latest.schema == source.schema && latest.process == source.process {
					refresh_draft_transition_hash(&project, &ledger, &key, history, &mut output)?;
					output.unchanged_contracts.push(key);
					continue;
				}

				let latest_version = latest.version;
				if ledger.version_is_frozen(&key, latest_version) {
					let next = next_migration_version(&key, latest_version, manifest.version_type)?;
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
							.expect("decoded histories contain every adjacent prior version");
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

fn next_migration_version(
	identity: &str,
	current: u32,
	version_type: MigrationVersionType,
) -> Result<u32, MigrationError> {
	if current == version_type.max_version() {
		return Err(MigrationError::VersionExhausted {
			version_type: version_type.to_string(),
			identity: identity.to_owned(),
		});
	}

	// The configured maximum is at most `u32::MAX`, and equality returned above.
	Ok(current + 1)
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
	let publication_path = project.program_dir.join(PUBLICATIONS_PATH);
	let ledger = load_publication_ledger_for_manifest(&publication_path, &manifest)?;
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
		let latest = history
			.current()
			.expect("validated migration histories always contain a current version");
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
			publication_pending: ledger.pending.as_ref().is_some_and(|pending| {
				pending
					.versions
					.get(&key)
					.is_some_and(|published| published.version >= latest.version)
			}),
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

/// Persist the exact ABI candidate before a persistent deployment starts.
///
/// Repeating the same attempt is idempotent. A different attempt is rejected
/// until the pending deployment is reconciled, because it may already be live.
pub fn begin_publication(
	start: &Path,
	cluster: &str,
	rpc_url: &str,
	program_id: &str,
	artifact: &Path,
	expected_artifact_digest: [u8; 32],
) -> Result<Option<PendingPublication>, MigrationError> {
	let project = Project::discover(start)?;
	let _lock = acquire_migration_lock(&project.program_dir)?;
	let publication_path = project.program_dir.join(PUBLICATIONS_PATH);
	let mut ledger = load_publication_ledger(&publication_path)?;
	let manifest_path = project.program_dir.join(MANIFEST_PATH);
	let manifest = load_manifest(&manifest_path)?;

	if let Some(existing) = &ledger.pending {
		let manifest = manifest.as_ref().ok_or_else(|| {
			MigrationError::InvalidHistory(
				"migration manifest disappeared during publication".to_owned(),
			)
		})?;
		validate_ledger_for_manifest(&ledger, manifest)?;
		let artifact_digest = hash_regular_file(artifact)?;
		if artifact_digest != expected_artifact_digest {
			return Err(MigrationError::PublicationArtifactChanged {
				path: artifact.to_path_buf(),
			});
		}
		if existing.cluster == cluster
			&& existing.rpc_url == rpc_url
			&& existing.program_id == program_id
			&& existing.executable_sha256 == hex_digest(artifact_digest)
		{
			return Ok(Some(existing.clone()));
		}
		return Err(MigrationError::PublicationPending {
			program_id: existing.program_id.clone(),
			cluster: existing.cluster.clone(),
		});
	}

	let statuses = check_project_migrations(&project)?;
	if statuses.is_empty() {
		return Ok(None);
	}
	let manifest = manifest
		.expect("a successful migration check with contracts requires the already-loaded manifest");
	if manifest.program_id != program_id {
		return Err(MigrationError::PublicationProgramMismatch {
			deployed: program_id.to_owned(),
			manifest: manifest.program_id,
		});
	}
	validate_ledger_for_manifest(&ledger, &manifest)?;
	let artifact_digest = hash_regular_file(artifact)?;
	if artifact_digest != expected_artifact_digest {
		return Err(MigrationError::PublicationArtifactChanged {
			path: artifact.to_path_buf(),
		});
	}
	let versions = statuses
		.into_iter()
		.map(|status| {
			let history = manifest
				.contracts
				.get(&status.identity)
				.unwrap_or_else(|| panic!("migration status must name a manifest contract"));
			let pinned = history
				.versions
				.iter()
				.map(|version| {
					PublishedSchema {
						schema_sha256: version.schema_sha256.clone(),
						transition_sha256: version
							.transition
							.as_ref()
							.and_then(|transition| transition.implementation_sha256.clone()),
					}
				})
				.collect();
			(
				status.identity,
				PublishedContract {
					version: status.current_version,
					history: pinned,
				},
			)
		})
		.collect();
	let pending = PendingPublication {
		cluster: cluster.to_owned(),
		rpc_url: rpc_url.to_owned(),
		program_id: program_id.to_owned(),
		executable_sha256: hex_digest(artifact_digest),
		manifest_sha256: manifest.sha256(),
		versions,
		previous_receipt_sha256: ledger.receipts.last().map(PublicationReceipt::sha256),
	};
	ledger.pending = Some(pending.clone());
	validate_ledger_for_manifest(&ledger, &manifest)?;
	write_json_atomic(&publication_path, &ledger)?;
	Ok(Some(pending))
}

/// Convert the matching pending deployment into an immutable receipt.
///
/// The pending record remains intact on every error so a remotely successful
/// deployment cannot become a mutable local draft.
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
	let pending = ledger
		.pending
		.clone()
		.ok_or(MigrationError::MissingPendingPublication)?;
	if pending.cluster != cluster
		|| pending.rpc_url != rpc_url
		|| pending.program_id != deployed_program_id
		|| pending.executable_sha256 != hex_digest(artifact_digest)
	{
		return Err(MigrationError::MissingPendingPublication);
	}
	let sequence = u64::try_from(ledger.receipts.len())
		.map_err(|_| MigrationError::PublicationSequenceExhausted)?;
	let receipt = PublicationReceipt {
		sequence,
		cluster: pending.cluster,
		rpc_url: pending.rpc_url,
		program_id: pending.program_id,
		executable_sha256: pending.executable_sha256,
		manifest_sha256: pending.manifest_sha256,
		versions: pending.versions,
		previous_receipt_sha256: pending.previous_receipt_sha256,
		abandoned: false,
	};
	ledger.receipts.push(receipt.clone());
	ledger.pending = None;
	validate_ledger_for_manifest(&ledger, &manifest)?;
	write_json_atomic(&publication_path, &ledger)?;
	Ok(Some(receipt))
}

/// Outcome of reconciling a pending deployment.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ReconcileOutput {
	/// No deployment was pending, so there was nothing to reconcile.
	pub no_pending: bool,
	/// The pending deployment was converted into an abandoned receipt.
	pub abandoned: bool,
	/// Cluster of the pending deployment.
	pub cluster: Option<String>,
	/// RPC URL of the pending deployment.
	pub rpc_url: Option<String>,
	/// Program identity of the pending deployment.
	pub program_id: Option<String>,
	/// Executable digest the pending deployment planned to ship.
	pub executable_sha256: Option<String>,
}

/// Inspect or abandon an ambiguous pending deployment.
///
/// Without `abandon`, this reports the exact deployment that must be resumed:
/// rerunning the same cluster, RPC URL, program, and artifact reconciles the
/// pending record into a receipt. With `abandon`, the operator asserts the
/// deployment never went live or accepts the risk; the pending record becomes
/// an abandoned receipt that still freezes its pinned versions because the
/// remote outcome cannot be proven from the local ledger.
pub fn reconcile_publication(
	start: &Path,
	abandon: bool,
) -> Result<ReconcileOutput, MigrationError> {
	let project = Project::discover(start)?;
	let publication_path = project.program_dir.join(PUBLICATIONS_PATH);
	let manifest_path = project.program_dir.join(MANIFEST_PATH);
	let manifest = load_manifest(&manifest_path)?.ok_or_else(|| {
		MigrationError::InvalidHistory(
			"migration manifest disappeared during reconciliation".to_owned(),
		)
	})?;
	let ledger = load_publication_ledger_for_manifest(&publication_path, &manifest)?;
	let Some(pending) = ledger.pending.clone() else {
		return Ok(ReconcileOutput {
			no_pending: true,
			abandoned: false,
			cluster: None,
			rpc_url: None,
			program_id: None,
			executable_sha256: None,
		});
	};

	if !abandon {
		return Ok(ReconcileOutput {
			no_pending: false,
			abandoned: false,
			cluster: Some(pending.cluster),
			rpc_url: Some(pending.rpc_url),
			program_id: Some(pending.program_id),
			executable_sha256: Some(pending.executable_sha256),
		});
	}

	let mut ledger = ledger;
	let _lock = acquire_migration_lock(&project.program_dir)?;
	let sequence = u64::try_from(ledger.receipts.len())
		.map_err(|_| MigrationError::PublicationSequenceExhausted)?;
	let receipt = PublicationReceipt {
		sequence,
		cluster: pending.cluster,
		rpc_url: pending.rpc_url,
		program_id: pending.program_id,
		executable_sha256: pending.executable_sha256,
		manifest_sha256: pending.manifest_sha256,
		versions: pending.versions,
		previous_receipt_sha256: pending.previous_receipt_sha256,
		abandoned: true,
	};
	ledger.receipts.push(receipt);
	ledger.pending = None;
	validate_ledger_for_manifest(&ledger, &manifest)?;
	write_json_atomic(&publication_path, &ledger)?;
	Ok(ReconcileOutput {
		no_pending: false,
		abandoned: true,
		cluster: None,
		rpc_url: None,
		program_id: None,
		executable_sha256: None,
	})
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
			validate_published_contract(
				&format!("publication receipt {}", receipt.sequence),
				key,
				published,
				manifest,
			)?;
		}
	}
	if let Some(pending) = &ledger.pending {
		if pending.program_id != manifest.program_id {
			return Err(MigrationError::InvalidHistory(format!(
				"pending publication belongs to program {}, expected {}",
				pending.program_id, manifest.program_id
			)));
		}
		for (key, candidate) in &pending.versions {
			validate_published_contract("pending publication", key, candidate, manifest)?;
		}
	}
	Ok(())
}

/// Verify one receipt or pending record against the checked-in manifest.
///
/// Every pinned schema and transition must still be present with exactly the
/// recorded hash. A published schema rewritten with consistently recomputed
/// internal hashes therefore still fails the check instead of silently
/// changing how historical account bytes are interpreted.
fn validate_published_contract(
	source: &str,
	key: &str,
	published: &PublishedContract,
	manifest: &MigrationManifest,
) -> Result<(), MigrationError> {
	let history = manifest.contracts.get(key).ok_or_else(|| {
		MigrationError::InvalidHistory(format!("{source} names unknown contract `{key}`"))
	})?;
	let current = history.current().ok_or_else(|| {
		MigrationError::InvalidHistory(format!("contract `{key}` has no versions"))
	})?;
	if published.version > current.version {
		return Err(MigrationError::InvalidHistory(format!(
			"{source} claims future version {} for `{key}`",
			published.version
		)));
	}
	if published.history.is_empty() {
		return Ok(());
	}
	for (index, pin) in published.history.iter().enumerate() {
		let version = &history.versions[index];
		if pin.schema_sha256 != version.schema_sha256 {
			return Err(MigrationError::InvalidHistory(format!(
				"{source} pinned schema {} for `{key}` version {index}, but the manifest now 				 \
				 records {}",
				pin.schema_sha256, version.schema_sha256
			)));
		}
		let implementation = version
			.transition
			.as_ref()
			.and_then(|transition| transition.implementation_sha256.as_deref());
		if pin.transition_sha256.as_deref() != implementation {
			return Err(MigrationError::InvalidHistory(format!(
				"{source} pinned a different transition implementation for `{key}` version 				 \
				 {index}"
			)));
		}
	}
	Ok(())
}

struct CurrentProgram {
	program_id: String,
	contracts: Vec<CurrentContract>,
}

fn scan_current_contracts(project: &Project) -> Result<CurrentProgram, MigrationError> {
	let (ir, files) =
		parse::parse_program_with_sources(&project.program_dir, Some(&project.library_name))?;
	let mut discriminators = Vec::new();
	let mut events = Vec::new();
	for resolved in &files {
		// The shared parser has already validated discriminator declarations in
		// these exact syntax trees while assembling `ir`.
		discriminators.extend(parse::discriminator::extract_discriminator_enums(
			&resolved.file,
		)?);
		events.extend(parse::event_data::extract_migratable_events(
			&resolved.file,
		)?);
	}
	let discriminator_map = parse::build_discriminator_map(&discriminators);
	let mut contracts = BTreeMap::new();

	for account in ir.accounts.iter().filter(|account| account.is_migratable()) {
		let discriminator = &account.discriminator;
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
			.iter()
			.map(|field| {
				FieldSchema {
					name: field.name.clone(),
					rust_type: field.rust_type.clone(),
				}
			})
			.collect();
		let schema = DataSchema::try_new(layout, fields).map_err(MigrationError::InvalidHistory)?;
		insert_current(
			&mut contracts,
			CurrentContract {
				identity,
				rust_name: account.name.clone(),
				schema,
				process: None,
			},
		)?;
	}

	for instruction in ir
		.instructions
		.iter()
		.filter(|instruction| instruction.is_migratable())
	{
		let discriminator = &instruction.discriminator;
		let identity = ContractIdentity::try_new(
			ContractKind::Instruction,
			discriminator.repr_size,
			discriminator.value,
		)
		.map_err(MigrationError::InvalidHistory)?;
		let fields = instruction
			.arguments
			.iter()
			.map(|field| {
				FieldSchema {
					name: field.name.clone(),
					rust_type: field.rust_type.clone(),
				}
			})
			.collect();
		let schema = DataSchema::try_new(LayoutKind::Fixed, fields)
			.map_err(MigrationError::InvalidHistory)?;
		insert_current(
			&mut contracts,
			CurrentContract {
				identity,
				rust_name: instruction.name.clone(),
				schema,
				process: Some(process_contract(instruction)),
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

/// Load the ledger and fail closed when advanced versions lost their pins.
///
/// Versions beyond zero only exist after a publication, so a manifest with
/// advanced versions and no ledger file means the publication evidence was
/// deleted or never committed. Treating that state as drafts would let
/// `pina migrations make` rewrite published history in place.
fn load_publication_ledger_for_manifest(
	path: &Path,
	manifest: &MigrationManifest,
) -> Result<PublicationLedger, MigrationError> {
	let ledger = load_publication_ledger(path)?;
	if !path.exists()
		&& manifest
			.contracts
			.values()
			.any(|history| history.versions.len() > 1)
	{
		return Err(MigrationError::InvalidHistory(
			"the manifest records advanced versions but the publication ledger is missing; 			 \
			 restore migrations/publications.json from version control because published 			 \
			 history must stay pinned"
				.to_owned(),
		));
	}
	Ok(ledger)
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
	write_all(&mut file, bytes, path)?;
	file.commit().map_err(|source| {
		MigrationError::Write {
			path: path.to_path_buf(),
			source,
		}
	})
}

fn write_all(
	mut writer: impl std::io::Write,
	bytes: &[u8],
	path: &Path,
) -> Result<(), MigrationError> {
	writer.write_all(bytes).map_err(|source| {
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
	hash_reader(path, &mut file)
}

fn hash_reader(path: &Path, mut reader: impl std::io::Read) -> Result<[u8; 32], MigrationError> {
	let mut digest = Sha256::new();
	let mut buffer = vec![0_u8; 64 * 1024];
	loop {
		let read = reader.read(&mut buffer).map_err(|source| {
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
			)?
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
		let &(destination_offset, size) = destination_offsets
			.get(&field.name)
			.expect("fixed layout offsets contain every destination field");
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
) -> Result<String, MigrationError> {
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
			let migrate = if dynamic {
				// Variable-length migrations size the destination through the
				// functions above; the executor resizes before calling.
				"pub(crate) fn migrate(data: &mut [u8]) {\n\tlet _ = data;\n}\n"
			} else {
				"pub(crate) fn migrate(data: &mut [u8]) {\n\tif data.len() < WORKING_SIZE \
				 {\n\t\treturn;\n\t}\n\tlet _ = data;\n}\n"
			};
			(
				"the source shape is preflighted; this conversion must be total and fully \
				 initialize destination",
				sizing,
				migrate,
			)
		}
		ContractKind::Instruction | ContractKind::Event => {
			let source_size = source_size.ok_or_else(|| {
				MigrationError::InvalidHistory(format!(
					"instruction and event histories must use fixed layouts; `{}` does not",
					identity.key()
				))
			})?;
			let destination_size = destination_size.ok_or_else(|| {
				MigrationError::InvalidHistory(format!(
					"instruction and event histories must use fixed layouts; `{}` does not",
					identity.key()
				))
			})?;
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
	Ok(format!(
		"// Manual adjacent ABI migration generated by `pina migrations make`.\n// Source \
		 version: {} ({source_description} bytes)\n// Destination version: {destination_version} \
		 ({destination_description} bytes)\n// TODO(pina-manual-migration): \
		 {requirement}.\n{sizing}\n{migrate}",
		source.version,
	))
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
			if ledger.version_is_frozen(key, version.version) {
				return Err(MigrationError::FrozenImplementationChanged { path });
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
	let latest = history
		.versions
		.last_mut()
		.expect("decoded migration histories always contain a current version");
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
	if ledger.version_is_frozen(key, latest.version) {
		return Err(MigrationError::FrozenImplementationChanged { path });
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
		assert!(
			process_transition(&identity, "Transfer", Some(&source), Some(&destination),).is_ok()
		);

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
		let account =
			ContractIdentity::try_new(ContractKind::Account, 1, 1).expect("valid identity");
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
		let crlf = hash_transition_file(&source)
			.unwrap_or_else(|error| panic!("hash CRLF source: {error}"));

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
		check_migrations(&fixture.root)
			.unwrap_or_else(|error| panic!("check replaced draft: {error}"));

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
			include_str!("../../../examples/migrations_program/src/lib.rs"),
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
			5
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
					"use pina::*;\ndeclare_id!(\"{}\");\n#[discriminator]\nenum EventKind {{ \
					 Value = 1 }}\n{body}\n",
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
				"#[event(discriminator = Missing::Value, migrations)] struct ValueEvent { value: \
				 u64 }"
			),
			Err(MigrationError::InvalidHistory(_))
		));
		assert!(matches!(
			scan(
				"#[event(discriminator = EventKind::Value, migrations)] struct First { value: u64 \
				 }\n#[event(discriminator = EventKind::Value, migrations)] struct Second { value: \
				 u64 }"
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
				"use pina::*;\ndeclare_id!(\"{}\");\n#[discriminator]\nenum Kind {{ State = 1, \
				 Other = 2 }}\n#[account(discriminator = Kind::State, migrations)]\nstruct State \
				 {{ value: u64 }}\n#[account(discriminator = Kind::Other, migrations)]\nstruct \
				 Other {{ value: u8 }}\n",
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
				 }}\n#[account(discriminator = Kind::State, migrations)]\nstruct State {{ value: \
				 u64, enabled: bool }}\n",
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
		tampered.validate().unwrap_or_else(|error| {
			panic!("coherent tamper must pass manifest validation: {error}")
		});

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
				"use pina::*;\ndeclare_id!(\"{PROGRAM_ID}\");\n#[discriminator]\nenum Kind {{ \
				 State = 1 }}\n#[account(discriminator = Kind::State, migrations)]\nstruct State \
				 {{ value: u64 }}\n"
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
		let fixture = migration_fixture();
		let identity = ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap();
		let schema = schema(LayoutKind::Fixed, &[("value", "u64")]);
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
				.unwrap_or_else(|error| panic!("serialize manifest: {error}")),
		)
		.unwrap_or_else(|error| panic!("write manifest: {error}"));
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
				 }}\n#[account(discriminator = Kind::State, migrations)]\nstruct State {{ \
				 {fields} }}\n",
				fixture.program_id
			),
		)
		.unwrap_or_else(|error| panic!("write State source: {error}"));
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
