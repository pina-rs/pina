//! Migrations CLI: schema diffing, transition generation, publication
//! bookkeeping, and the disambiguation flow that ties them together.

mod abi_layout;
mod build_script;
mod cost;
mod diff;
mod ledger;
mod prompt;
mod remedy;
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

pub use abi_layout::ABI_LAYOUT_TEST_PATH;
use abi_layout::generate as generate_abi_layout;
pub use build_script::BuildScriptStatus;
use build_script::ensure_build_script;
use build_script::verify_build_script;
pub use cost::AccountCostPreview;
pub use cost::InstructionCostPreview;
pub use cost::InstructionLadder;
pub use cost::LadderCost;
pub use cost::MigrationCostPreview;
pub use cost::MostExpensiveTransaction;
pub use cost::StaticCuEstimate;
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
use pina_abi::MigrationAuto;
use pina_abi::MigrationManifest;
use pina_abi::MigrationVersionType;
use pina_abi::PUBLICATIONS_PATH;
use pina_abi::ProcessContract;
use pina_abi::PublicationLedger;
use pina_abi::SchemaVersion;
pub use prompt::DisambiguationQuestion;
pub use prompt::MigrationAnswers;
use prompt::PromptIo;
use scan::CurrentContract;
use scan::CurrentOptOut;
use scan::next_migration_version;
use scan::scan_current_contracts;
use scan::validate_program_configuration;
use serde::Serialize;
use storage::acquire_migration_lock;
use storage::write_atomic;
use storage::write_json_atomic;
use transition::TransitionRequest;
use transition::create_transition;
use transition::refresh_draft_transition_hash;
use transition::supported_stale_ladder;
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
	/// Kinds recorded as automatically enveloped by this run.
	pub auto: Vec<String>,
	/// Build-script action taken for the recorded auto policy.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub build_script: Option<BuildScriptStatus>,
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
	/// Versions this contract can still consume before its width is exhausted.
	///
	/// Every contract owns an independent history, so this is a per-contract
	/// budget. The width freezes at the first publication, which is why running
	/// out is a planning problem rather than something a later release fixes.
	pub versions_remaining: u32,
}

/// `pina migrations status` output: per-contract state plus the cost preview.
///
/// The `statuses` list keeps the exact `MigrationStatus` shape `check --json`
/// emits, and `costPreview` is the additive pre-deploy cost section.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationStatusReport {
	pub statuses: Vec<MigrationStatus>,
	pub cost_preview: MigrationCostPreview,
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

	#[error(
		"The generated ABI layout test {path} is stale and no longer matches the manifest. Run \
		 `pina migrations make` to regenerate it."
	)]
	AbiLayoutTestStale { path: PathBuf },

	#[error(
		"The generated ABI layout test {path} is missing. Run `pina migrations make` to create it."
	)]
	AbiLayoutTestMissing { path: PathBuf },

	#[error(
		"Opting these already-published contracts into the migration version envelope changes \
		 their wire format: {contracts}. Every byte after the discriminator shifts, so regenerate \
		 clients, update fixtures and hand-written decoders, and commit the regenerated layout \
		 test. Re-run with `--envelope-ack` to record the change."
	)]
	EnvelopeAcknowledgementRequired { contracts: String },

	#[error("Migration history belongs to program {found}, but current source declares {expected}")]
	ProgramIdentityChanged { expected: String, found: String },

	#[error("Migration version encoding is frozen as {found}, but pina.toml configures {expected}")]
	VersionTypeChanged { expected: String, found: String },

	#[error(
		"Migration auto policy is recorded as {found}, but pina.toml configures {expected}. Run \
		 `pina migrations make` to record the policy flip."
	)]
	AutoPolicyChanged { expected: String, found: String },

	#[error(
		"{kind} `{name}` ({identity}) is recorded in the migration manifest but is no longer \
		 migration-aware. Removing an envelope is a wire-format change that `pina migrations \
		 make` must record deliberately; restore its migration coverage (a `migrations` token or \
		 the matching `[migrations].auto` kind) or retire the contract deliberately."
	)]
	EnvelopeRemoval {
		kind: String,
		name: String,
		identity: String,
	},

	#[error(
		"Migration auto policy requires the exact line `{directive}` in {path}. Run `pina \
		 migrations make` to scaffold a missing script, or add that line to the existing build \
		 script."
	)]
	BuildScriptRerunMissing {
		path: PathBuf,
		directive: &'static str,
	},

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

	#[error(
		"Configured {version_type} migration versions are exhausted for `{identity}`. Versions \
		 are counted per contract, so this is the ceiling for this one contract, not for the \
		 program. The width is program-wide and freezes at the first publication, so it cannot be \
		 widened now: adopt a successor contract with a new discriminator and a fresh version-0 \
		 history, and add a bridge instruction that reads the exhausted account through the \
		 current loaders and writes the successor. Account history cannot be pruned, because a \
		 program cannot enumerate its own accounts without an external completeness proof."
	)]
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
	// `make` records the policy configured in `pina.toml`; every later reader
	// (macros, build, IDL) trusts only the manifest.
	let auto = project.migration_auto.clone();
	let current = scan_current_contracts(&project, &auto)?;
	let manifest_path = project.program_dir.join(MANIFEST_PATH);
	let publication_path = project.program_dir.join(PUBLICATIONS_PATH);
	let mut manifest = load_manifest(&manifest_path)?.unwrap_or_else(|| {
		MigrationManifest::new(current.program_id.clone(), project.migration_version_type)
	});
	reject_opt_outs(&manifest, &current.opt_outs)?;
	let ledger = load_publication_ledger_for_manifest(&publication_path, &manifest)?;
	validate_program_configuration(&project, &current.program_id, &manifest)?;
	validate_ledger_for_manifest(&ledger, &manifest)?;

	let mut output = MakeMigrationsOutput {
		manifest: manifest_path.clone(),
		..MakeMigrationsOutput::default()
	};
	let mut seen = BTreeMap::new();

	// Opting a contract into the version envelope inserts a byte after its
	// discriminator, shifting every byte that follows. When the contract was
	// already published, that reaches every generated client, fixture, and
	// hand-written decoder for it, so the command must say so and require an
	// explicit acknowledgement instead of recording the change quietly.
	if !answers.envelope_ack {
		let newly_enveloped = first_time_envelopes(&current.contracts, &manifest, &ledger);
		if !newly_enveloped.is_empty() {
			return Err(MigrationError::EnvelopeAcknowledgementRequired {
				contracts: newly_enveloped.join(", "),
			});
		}
	}

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
					let stale_ladder = supported_stale_ladder(history, next);
					let transition = create_transition(
						&project,
						TransitionRequest {
							identity: &history.identity,
							rust_name: &history.rust_name,
							source: &effective,
							stale_ladder: &stale_ladder,
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
						let stale_ladder = supported_stale_ladder(history, latest_version);
						let transition = create_transition(
							&project,
							TransitionRequest {
								identity: &history.identity,
								rust_name: &history.rust_name,
								source: &effective,
								stale_ladder: &stale_ladder,
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

	// A contract whose kind the policy no longer covers is an envelope removal,
	// not a silent source deletion.
	let dropped = auto.removed_since(&manifest.auto);
	for (key, history) in &manifest.contracts {
		if !seen.contains_key(key) {
			if dropped.contains(&history.identity.kind) {
				return Err(envelope_removal(history));
			}
			return Err(MigrationError::ContractRemoved {
				kind: history.identity.kind.to_string(),
				name: history.rust_name.clone(),
				identity: key.clone(),
			});
		}
	}

	manifest.auto = auto;
	manifest
		.validate()
		.map_err(MigrationError::InvalidHistory)?;
	write_json_atomic(&manifest_path, &manifest)?;
	write_json_atomic(&publication_path, &ledger)?;
	write_abi_layout_test(&project.program_dir, &manifest)?;
	output.auto = manifest
		.auto
		.iter()
		.map(|kind| kind.config_name().to_owned())
		.collect();
	if !manifest.auto.is_empty() {
		output.build_script = Some(ensure_build_script(&project.program_dir)?);
	}
	Ok(output)
}

/// Write the machine-checked ABI layout test.
///
/// The file is a deterministic function of the manifest, so an unchanged
/// program regenerates identical bytes and [`check_project_migrations`] can
/// compare content instead of tracking a hash.
fn write_abi_layout_test(
	program_dir: &Path,
	manifest: &MigrationManifest,
) -> Result<(), MigrationError> {
	let path = program_dir.join(ABI_LAYOUT_TEST_PATH);
	let generated = generate_abi_layout(manifest);
	if abi_layout::read_existing(program_dir).map_err(|source| {
		MigrationError::Read {
			path: path.clone(),
			source,
		}
	})? == Some(generated.clone())
	{
		return Ok(());
	}
	if let Some(parent) = path.parent() {
		std::fs::create_dir_all(parent).map_err(|source| {
			MigrationError::CreateDirectory {
				path: parent.to_path_buf(),
				source,
			}
		})?;
	}
	// `write_atomic` also enforces the safe-path check, matching how the
	// manifest and publication ledger are written.
	write_atomic(&path, generated.as_bytes())
}

/// Fail when the checked-in ABI layout test no longer matches the manifest.
///
/// A stale file means a schema change shipped without regenerating the guard,
/// which is exactly the case that let a hand-maintained offset drift.
fn verify_abi_layout_test(
	program_dir: &Path,
	manifest: &MigrationManifest,
) -> Result<(), MigrationError> {
	let path = program_dir.join(ABI_LAYOUT_TEST_PATH);
	let expected = generate_abi_layout(manifest);
	match abi_layout::read_existing(program_dir).map_err(|source| {
		MigrationError::Read {
			path: path.clone(),
			source,
		}
	})? {
		Some(existing) if layout_tests_agree(&existing, &expected) => Ok(()),
		Some(_) => Err(MigrationError::AbiLayoutTestStale { path }),
		None => Err(MigrationError::AbiLayoutTestMissing { path }),
	}
}

/// Whether a checked-in ABI layout test carries the same content as generated
/// output, ignoring incidental line wrapping.
///
/// `rustfmt` re-wraps the long `SCHEMA_SHA256` constants and collapses
/// single-element `FIELDS` arrays, so a formatted guard file is never
/// byte-identical to generator output. Comparing normalized whitespace keeps
/// `pina migrations make` and `fix:format` from fighting: the guard still fails
/// closed on any real change (a value, a constant name, a module), which is what
/// it exists to catch.
fn layout_tests_agree(existing: &str, generated: &str) -> bool {
	fn normalized(source: &str) -> String {
		source
			.split_whitespace()
			.collect::<Vec<_>>()
			.join(" ")
			.trim()
			.to_owned()
	}
	normalized(existing) == normalized(generated)
}

/// Return the contracts this run envelopes for the first time on a program
/// that already has published deployments.
///
/// Ledger validation guarantees every published contract is also in the
/// manifest, so "published but missing" cannot happen. The reachable case is a
/// program that is already live where `[migrations].auto` widens — accounts
/// only today, accounts and events tomorrow — so contracts that carried no
/// envelope gain one. Their wire format changes even though nothing was
/// removed: every byte after the discriminator shifts, and every generated
/// client and hand-written decoder for that contract sees it.
///
/// A program with no receipts at all has nothing live to break, so a
/// first-time envelope there is free.
fn first_time_envelopes(
	contracts: &[CurrentContract],
	manifest: &MigrationManifest,
	ledger: &PublicationLedger,
) -> Vec<String> {
	let program_is_live = !ledger.receipts.is_empty() || ledger.pending.is_some();
	if !program_is_live {
		return Vec::new();
	}
	let mut names = Vec::new();
	for source in contracts {
		let key = source.identity.key();
		if manifest.contracts.contains_key(&key) {
			continue;
		}
		names.push(format!("{} ({})", source.rust_name, key));
	}
	names
}

/// Reject an explicit `migrations = false` on a contract already recorded.
fn reject_opt_outs(
	manifest: &MigrationManifest,
	opt_outs: &[CurrentOptOut],
) -> Result<(), MigrationError> {
	for opt_out in opt_outs {
		let Ok(history) = manifest.contract_for_source(opt_out.kind, &opt_out.rust_name) else {
			continue;
		};
		return Err(envelope_removal(history));
	}
	Ok(())
}

fn envelope_removal(history: &ContractHistory) -> MigrationError {
	MigrationError::EnvelopeRemoval {
		kind: history.identity.kind.to_string(),
		name: history.rust_name.clone(),
		identity: history.identity.key(),
	}
}

/// Fail when the checked-in policy differs from `pina.toml`.
fn validate_auto_policy(
	project: &Project,
	manifest: &MigrationManifest,
) -> Result<(), MigrationError> {
	if manifest.auto == project.migration_auto {
		return Ok(());
	}

	Err(MigrationError::AutoPolicyChanged {
		expected: project.migration_auto.to_string(),
		found: manifest.auto.to_string(),
	})
}

/// Verify source, snapshots, process contracts, and frozen transition code.
pub fn check_migrations(start: &Path) -> Result<Vec<MigrationStatus>, MigrationError> {
	let project = Project::discover(start)?;
	check_project_migrations(&project)
}

/// Verify the explicit `pina migrations check` gate, including the generated
/// ABI layout test.
///
/// `check` is the CI entry point, so it requires the guard file. The plain
/// `check_migrations` used by `build`, `status`, and publication stays
/// unchanged: those run during ordinary work on an existing project, where
/// demanding a regenerated guard would block them for an unrelated reason.
pub fn check_migrations_with_abi_layout(
	start: &Path,
) -> Result<Vec<MigrationStatus>, MigrationError> {
	let project = Project::discover(start)?;
	let (statuses, manifest) = check_project_migrations_with_manifest(&project)?;
	if let Some(manifest) = &manifest {
		verify_abi_layout_test(&project.program_dir, manifest)?;
	}
	Ok(statuses)
}

pub(crate) fn check_project_migrations(
	project: &Project,
) -> Result<Vec<MigrationStatus>, MigrationError> {
	Ok(check_project_migrations_with_manifest(project)?.0)
}

/// [`check_project_migrations`] keeping the validated manifest for the cost
/// preview, so a status report does not load and validate history twice.
fn check_project_migrations_with_manifest(
	project: &Project,
) -> Result<(Vec<MigrationStatus>, Option<MigrationManifest>), MigrationError> {
	let manifest_path = project.program_dir.join(MANIFEST_PATH);
	let manifest = load_manifest(&manifest_path)?;
	// Verification follows the recorded policy because that is what macros
	// expanded against; a policy difference is reported below as a stale
	// manifest that only `make` may refresh.
	let auto = manifest.as_ref().map_or_else(
		|| project.migration_auto.clone(),
		|manifest| manifest.auto.clone(),
	);
	let current = scan_current_contracts(project, &auto)?;
	if current.contracts.is_empty() && manifest.is_none() {
		return Ok((Vec::new(), None));
	}
	let manifest = manifest.ok_or_else(|| {
		let first = &current.contracts[0];
		MigrationError::MissingSnapshot {
			kind: first.identity.kind.to_string(),
			name: first.rust_name.clone(),
		}
	})?;
	reject_opt_outs(&manifest, &current.opt_outs)?;
	let publication_path = project.program_dir.join(PUBLICATIONS_PATH);
	let ledger = load_publication_ledger_for_manifest(&publication_path, &manifest)?;
	validate_program_configuration(project, &current.program_id, &manifest)?;
	validate_auto_policy(project, &manifest)?;
	if !manifest.auto.is_empty() {
		verify_build_script(&project.program_dir)?;
	}
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
			versions_remaining: manifest
				.version_type
				.max_version()
				.saturating_sub(latest.version),
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
	Ok((statuses, Some(manifest)))
}

/// Return migration status after applying every build-time compatibility check.
pub fn migration_status(start: &Path) -> Result<Vec<MigrationStatus>, MigrationError> {
	check_migrations(start)
}

/// Return migration status plus the pre-deploy cost preview.
///
/// The preview derives from the same validated history and, when the compiled
/// SBF artifact exists, from `pina profile`'s static per-function estimates.
/// A missing or unparsable artifact turns every CU figure into an explicit
/// `unavailable` reason rather than a zero.
pub fn migration_status_report(start: &Path) -> Result<MigrationStatusReport, MigrationError> {
	let project = Project::discover(start)?;
	let (statuses, manifest) = check_project_migrations_with_manifest(&project)?;
	let source = cost::load_profile_source(&project, manifest.as_ref());
	let cost_preview = cost::build_cost_preview(manifest.as_ref(), &source);

	Ok(MigrationStatusReport {
		statuses,
		cost_preview,
	})
}

/// Read the recorded auto policy without running compatibility checks.
///
/// IDL extraction needs the policy before it can assemble the IR, and the full
/// migration check runs afterwards. An unreadable or invalid manifest returns
/// no policy here; the check that follows reports the real failure.
pub(crate) fn manifest_auto_policy(program_dir: &Path) -> MigrationAuto {
	load_manifest(&program_dir.join(MANIFEST_PATH))
		.ok()
		.flatten()
		.map_or_else(MigrationAuto::none, |manifest| manifest.auto)
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
