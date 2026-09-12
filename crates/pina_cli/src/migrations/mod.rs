//! Migrations CLI: schema diffing, transition generation, publication
//! bookkeeping, and the disambiguation flow that ties them together.

mod diff;
mod ledger;
mod prompt;
mod scan;
mod storage;
mod transition;

pub mod inspect;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::io::IsTerminal as _;
use std::path::Path;
use std::path::PathBuf;

use diff::effective_source_schema;
use diff::resolve_field_changes;
pub use ledger::ReconcileOutput;
pub use ledger::begin_publication;
use ledger::load_manifest;
use ledger::load_publication_ledger_for_manifest;
pub use ledger::reconcile_publication;
pub use ledger::record_publication;
use ledger::validate_ledger_for_manifest;
use pina_abi::ContractHistory;
use pina_abi::MANIFEST_PATH;
use pina_abi::MigrationManifest;
use pina_abi::MigrationVersionType;
use pina_abi::PUBLICATIONS_PATH;
use pina_abi::ProcessContract;
use pina_abi::SchemaVersion;
pub use prompt::DisambiguationQuestion;
pub use prompt::MigrationAnswers;
use prompt::PromptIo;
use scan::next_migration_version;
use scan::scan_current_contracts;
use scan::validate_program_configuration;
use serde::Serialize;
use storage::acquire_migration_lock;
use storage::write_json_atomic;
use transition::TransitionRequest;
use transition::create_transition;
use transition::refresh_draft_transition_hash;
use transition::verify_transition_files;

use crate::error::IdlError;
use crate::project::Project;
use crate::project::ProjectError;

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
	/// Data-loss warnings for removals the developer explicitly accepted.
	pub data_warnings: Vec<String>,
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

	#[error("{}", DisambiguationQuestion::render_all(questions))]
	DisambiguationRequired {
		questions: Vec<DisambiguationQuestion>,
	},
}

/// Create the initial ABI database or refresh its latest draft versions.
///
/// An unfrozen latest version is mutable. A published or pending latest
/// version is immutable and a source change appends one adjacent version.
pub fn make_migrations(start: &Path) -> Result<MakeMigrationsOutput, MigrationError> {
	make_migrations_with_answers(start, &MigrationAnswers::default())
}

/// [`make_migrations`] with explicit disambiguation answers.
pub fn make_migrations_with_answers(
	start: &Path,
	answers: &MigrationAnswers,
) -> Result<MakeMigrationsOutput, MigrationError> {
	let project = Project::discover(start)?;
	let _lock = acquire_migration_lock(&project.program_dir)?;
	// Attach the process terminal once: with a tty the disambiguation
	// questions below can be answered inline, and a piped stdin simply makes
	// the prompts report themselves unavailable.
	let stdin = std::io::stdin();
	let stdout = std::io::stdout();
	let interactive = stdin.is_terminal();
	let mut stdin_lock = stdin.lock();
	let mut stdout_lock = stdout.lock();
	let mut prompts = PromptIo::new(&mut stdin_lock, &mut stdout_lock, interactive);
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
					let (renames, dropped) = resolve_field_changes(
						&key,
						&latest.schema,
						&source.schema,
						latest
							.transition
							.as_ref()
							.map_or(&[], |transition| &transition.renames),
						answers,
						&mut output.data_warnings,
						&mut prompts,
					)?;
					let effective = effective_source_schema(latest, &renames, &dropped)?;
					let transition = create_transition(
						&project,
						TransitionRequest {
							identity: &history.identity,
							rust_name: &history.rust_name,
							source: &effective,
							renames,
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
						// The draft's own transition carries the disambiguation
						// answers recorded when it was created; reusing them
						// keeps repeated `make` runs over the draft stable
						// instead of re-asking settled questions.
						let (renames, dropped) = resolve_field_changes(
							&key,
							&previous.schema,
							&source.schema,
							latest
								.transition
								.as_ref()
								.map_or(&[], |transition| &transition.renames),
							answers,
							&mut output.data_warnings,
							&mut prompts,
						)?;
						let effective = effective_source_schema(previous, &renames, &dropped)?;
						let transition = create_transition(
							&project,
							TransitionRequest {
								identity: &history.identity,
								rust_name: &history.rust_name,
								source: &effective,
								renames,
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

/// Machine-actionable failure envelope printed when a migration command
/// fails under `--json`.
///
/// `error` carries the human-readable message and `questions` the exact
/// disambiguation state, so CI logs and agents can answer with flags
/// without re-parsing prose.
#[derive(Debug, Serialize)]
pub struct JsonErrorEnvelope<'a> {
	/// The rendered failure message.
	pub error: String,
	/// The unanswered disambiguation questions, when the failure is
	/// [`MigrationError::DisambiguationRequired`].
	#[serde(skip_serializing_if = "Option::is_none")]
	pub questions: Option<&'a [DisambiguationQuestion]>,
}
