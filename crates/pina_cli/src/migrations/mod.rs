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
use std::collections::BTreeSet;
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
use diff::SourceIntent;
use diff::resolve_field_changes;
pub use ledger::PublicationAttempt;
pub use ledger::ReconcileOutput;
pub use ledger::begin_publication;
pub use ledger::begin_publication_attempt;
pub use ledger::discard_unsent_publication;
use ledger::load_manifest;
use ledger::load_publication_ledger_for_manifest;
pub use ledger::pin_legacy_publications;
pub use ledger::reconcile_publication;
pub use ledger::record_publication;
use ledger::validate_ledger_for_manifest;
use pina_abi::ContractHistory;
use pina_abi::ContractKind;
use pina_abi::MANIFEST_PATH;
use pina_abi::MigrationAuto;
use pina_abi::MigrationManifest;
use pina_abi::MigrationVersionType;
use pina_abi::PUBLICATIONS_PATH;
use pina_abi::PublicationLedger;
use pina_abi::SchemaVersion;
use pina_abi::Transition;
use pina_abi::TransitionMode;
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
pub struct CreateMigrationsOutput {
	pub manifest: PathBuf,
	pub created_contracts: Vec<String>,
	pub advanced_versions: Vec<String>,
	pub updated_drafts: Vec<String>,
	/// Published instruction snapshots whose account list gained appended
	/// optional slots in place. Old clients omit those slots, so the change
	/// needs no new version.
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub extended_processes: Vec<String>,
	/// Instruction snapshots recorded without an envelope that the source no
	/// longer asks to record (`migrations = false`, or an auto policy that
	/// stopped covering instructions). Nothing on the wire changes.
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub released_snapshots: Vec<String>,
	pub unchanged_contracts: Vec<String>,
	pub manual_transitions: Vec<PathBuf>,
	/// Data-loss warnings for removals the developer explicitly accepted.
	pub data_warnings: Vec<String>,
	/// Kinds the recorded auto policy covers after this run.
	pub auto: Vec<String>,
	/// The version envelope width recorded after this run.
	pub version_type: MigrationVersionType,
	/// The width this run replaced, when `--version-type` changed it.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub previous_version_type: Option<MigrationVersionType>,
	/// Build-script action taken for the recorded auto policy.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub build_script: Option<BuildScriptStatus>,
	/// Program ID the unpublished history was bound to before this run moved
	/// it to the current `declare_id!`.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub rebound_from_program_id: Option<String>,
	/// Hand-written transition bodies that no longer match their draft's
	/// layouts, moved aside so the regenerated stub can replace them.
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub stale_manual_transitions: Vec<PathBuf>,
	/// The history is bound to the shared `pina init` placeholder address.
	#[serde(skip_serializing_if = "std::ops::Not::not")]
	pub placeholder_program_id: bool,
}

/// Current status of one migration-aware wire contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationStatus {
	pub identity: String,
	pub kind: String,
	pub rust_name: String,
	/// Whether the contract carries the version envelope. An instruction
	/// recorded by an auto policy without the `migrations` token does not: its
	/// single snapshot only gates wire-breaking changes.
	pub envelope: bool,
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
	/// Every earlier schema of each event, oldest first, keyed like
	/// `current_versions`. Log records emitted before the current version are
	/// decoded with these, so the IDL lists each one as its own event.
	pub historical_events: BTreeMap<String, Vec<pina_abi::DataSchema>>,
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
		 `pina migrations create` to regenerate it."
	)]
	AbiLayoutTestStale { path: PathBuf },

	#[error(
		"The generated ABI layout test {path} is missing. Run `pina migrations create` to create \
		 it."
	)]
	AbiLayoutTestMissing { path: PathBuf },

	#[error(
		"These contracts would enter the migration version envelope while their contract kind is \
		 already published: {contracts}. For a contract that was itself published, every byte \
		 after the discriminator shifts, so regenerate clients, update fixtures and hand-written \
		 decoders, and commit the regenerated layout test. For a new contract, or one whose \
		 discriminator changed, nothing live shifts, but existing decoders of that kind must \
		 expect the envelope. Re-run with `--envelope-ack` to record the change."
	)]
	EnvelopeAcknowledgementRequired { contracts: String },

	#[error(
		"Migration history belongs to program {found}, but current source declares {expected}. If \
		 this history was never deployed, run `pina migrations create` to rebind it to the \
		 declared program; published history cannot move to a new program ID, so restore the \
		 original `declare_id!` instead"
	)]
	ProgramIdentityChanged { expected: String, found: String },

	#[error(
		"Migration version encoding is frozen as {recorded} because a deployment published it, \
		 so it cannot become {requested}"
	)]
	VersionTypeFrozen { recorded: String, requested: String },

	#[error(
		"{kind} `{name}` ({identity}) is recorded in the migration manifest but is no longer \
		 migration-aware. Removing an envelope is a wire-format change that `pina migrations \
		 create` must record deliberately; restore its migration coverage (a `migrations` token \
		 or an auto policy covering its kind, set with `pina migrations create --auto`) or \
		 retire the contract deliberately."
	)]
	EnvelopeRemoval {
		kind: String,
		name: String,
		identity: String,
	},

	#[error(
		"Instruction `{name}` ({identity}) is published without a version envelope, so it cannot \
		 gain one: every existing client sends its payload with no version byte. Declare a new \
		 discriminator for the migration-aware instruction."
	)]
	EnvelopeAddition { name: String, identity: String },

	#[error(
		"Instruction `{name}` ({identity}) is published without a version envelope, so its \
		 payload cannot change: no byte tells the program which layout a client sent. Declare a \
		 new discriminator for the new payload, or restore the published fields."
	)]
	PublishedPayloadChanged { name: String, identity: String },

	#[error(
		"Instruction `{name}` ({identity}) no longer asks to be recorded, but the migration \
		 manifest still holds its snapshot. Run `pina migrations create` to release it."
	)]
	StaleSnapshot { name: String, identity: String },

	#[error(
		"Migration auto policy requires the exact line `{directive}` in {path}. Run `pina \
		 migrations create` to scaffold a missing script, or add that line to the existing build \
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
		"Migration-aware {kind} `{name}` has no checked-in snapshot. Run `pina migrations create`."
	)]
	MissingSnapshot { kind: String, name: String },

	#[error(
		"Migration-aware {kind} `{name}` differs from version {version}. Its data schema or \
		 instruction process ABI changed. Run `pina migrations create` and review the transition."
	)]
	SchemaDrift {
		kind: String,
		name: String,
		version: u32,
	},

	#[error(
		"Migration implementation {path} differs from the recorded hash for {kind} `{name}` \
		 version {version}. Run `pina migrations create` and review the transition."
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
		 and run `pina migrations create`."
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
/// Rebuild the developer's recorded intent for the version a transition sits on.
///
/// A recorded manual transition has no automatic proof, so its recorded renames
/// name exactly the fields whose bytes the developer converts by hand. Deriving
/// the manual set from `mode` and `renames` keeps repeated `create` runs stable
/// without a second copy of the same fact in the manifest.
fn recorded_intent(transition: Option<&Transition>) -> SourceIntent {
	let Some(transition) = transition else {
		return SourceIntent::default();
	};
	// Only a manual transition's renames name hand-written conversions. An
	// automatic transition's renames are ordinary byte moves, so populating
	// `manual` from them would turn every rename into a manual draft.
	let manual = match transition.mode {
		TransitionMode::Manual => {
			transition
				.renames
				.iter()
				.map(|mapping| mapping.to.clone())
				.collect()
		}
		TransitionMode::Automatic => BTreeSet::new(),
	};
	SourceIntent {
		renames: transition.renames.clone(),
		dropped: BTreeSet::new(),
		manual,
		// A recorded manual transition keeps the developer's body regardless of
		// what the current diff would otherwise prove, so a refreshed draft
		// cannot silently replace it with generated bytes.
		force_manual: transition.mode == TransitionMode::Manual,
	}
}

/// Program ID recorded by the project's migration manifest, when one exists
/// and can be decoded.
pub(crate) fn recorded_program_id(program_dir: &Path) -> Option<String> {
	load_manifest(&program_dir.join(MANIFEST_PATH))
		.ok()
		.flatten()
		.map(|manifest| manifest.program_id)
}

/// Apply a `--version-type` request, returning the width it replaced.
///
/// The width sizes every enveloped contract's version field, so it may change
/// only while nothing is published: before then every history is a single
/// draft that simply re-expands with the new width.
fn change_version_type(
	manifest: &mut MigrationManifest,
	ledger: &PublicationLedger,
	requested: Option<MigrationVersionType>,
) -> Result<Option<MigrationVersionType>, MigrationError> {
	let Some(requested) = requested.filter(|requested| *requested != manifest.version_type) else {
		return Ok(None);
	};
	if !ledger.receipts.is_empty() || ledger.pending.is_some() {
		return Err(MigrationError::VersionTypeFrozen {
			recorded: manifest.version_type.to_string(),
			requested: requested.to_string(),
		});
	}
	Ok(Some(std::mem::replace(
		&mut manifest.version_type,
		requested,
	)))
}

/// Move a never-published history to the program ID the source now declares.
///
/// Returns the previous program ID when the manifest was rebound.
fn rebind_unpublished_history(
	manifest: &mut MigrationManifest,
	ledger: &PublicationLedger,
	current: &scan::CurrentProgram,
) -> Option<String> {
	let unpublished = ledger.receipts.is_empty()
		&& ledger.pending.is_none()
		&& manifest
			.contracts
			.values()
			.all(|history| history.versions.len() <= 1);
	if !unpublished || manifest.program_id == current.program_id {
		return None;
	}
	Some(std::mem::replace(
		&mut manifest.program_id,
		current.program_id.clone(),
	))
}

pub fn create_migrations(start: &Path) -> Result<CreateMigrationsOutput, MigrationError> {
	create_migrations_with_answers(start, &MigrationAnswers::default())
}

/// [`create_migrations`] with explicit disambiguation answers.
pub fn create_migrations_with_answers(
	start: &Path,
	answers: &MigrationAnswers,
) -> Result<CreateMigrationsOutput, MigrationError> {
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
	// The manifest is the only home of the policy: `--auto` and
	// `--version-type` change it, and every reader (macros, build, IDL) trusts
	// what it records. Without a manifest the policy starts empty with `u8`
	// versions.
	let manifest_path = project.program_dir.join(MANIFEST_PATH);
	let publication_path = project.program_dir.join(PUBLICATIONS_PATH);
	let recorded = load_manifest(&manifest_path)?;
	let auto = answers.auto.clone().unwrap_or_else(|| {
		recorded
			.as_ref()
			.map_or_else(MigrationAuto::none, |manifest| manifest.auto.clone())
	});
	let current = scan_current_contracts(&project, &auto)?;
	let mut manifest = recorded.unwrap_or_else(|| {
		MigrationManifest::new(
			current.program_id.clone(),
			answers.version_type.unwrap_or_default(),
		)
	});
	reject_opt_outs(&manifest, &current.opt_outs)?;
	let ledger = load_publication_ledger_for_manifest(&publication_path, &manifest)?;
	// A history nothing was ever deployed from belongs to no program yet, so a
	// new identity (for example from `pina keys new`) simply rebinds it. Once a
	// receipt or pending deployment exists the identity is part of the
	// published record and the mismatch below stays a hard error.
	let rebound_from_program_id = rebind_unpublished_history(&mut manifest, &ledger, &current);
	validate_program_configuration(&current.program_id, &manifest)?;
	validate_ledger_for_manifest(&ledger, &manifest)?;
	let previous_version_type = change_version_type(&mut manifest, &ledger, answers.version_type)?;
	let version_type = manifest.version_type;

	let mut output = CreateMigrationsOutput {
		manifest: manifest_path.clone(),
		rebound_from_program_id,
		previous_version_type,
		placeholder_program_id: current.program_id == crate::init::PLACEHOLDER_PROGRAM_ID,
		..CreateMigrationsOutput::default()
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
		let Some(history) = manifest.contracts.get_mut(&key) else {
			manifest.contracts.insert(
				key.clone(),
				ContractHistory {
					identity: source.identity,
					rust_name: source.rust_name,
					envelope: source.envelope,
					versions: vec![SchemaVersion {
						schema: source.schema,
						process: source.process,
						transition: None,
					}],
				},
			);
			output.created_contracts.push(key);
			continue;
		};
		history.rust_name.clone_from(&source.rust_name);
		if history.envelope != source.envelope {
			change_envelope(&ledger, &key, history, source, &mut output)?;
			continue;
		}
		if !history.is_migrated() {
			record_unmigrated(
				manifest.version_type,
				&ledger,
				&key,
				history,
				source,
				&mut output,
			)?;
			continue;
		}
		let latest = history
			.current()
			.expect("decoded migration histories always contain a current version");
		// `--manual <field>` must turn an unchanged automatic draft into a
		// hand-written one; otherwise the answer would be silently dropped.
		let converts_to_manual = !answers.manual.is_empty()
			&& latest
				.transition
				.as_ref()
				.is_some_and(|transition| transition.mode == TransitionMode::Automatic)
			&& !ledger.version_is_frozen(&key, history.current_version().unwrap_or(0))
			&& source
				.schema
				.fields
				.iter()
				.any(|field| answers.manual.contains(&field.name));
		if latest.schema.same_wire(&source.schema)
			&& latest.process == source.process
			&& !converts_to_manual
		{
			refresh_draft_transition_hash(&project, &ledger, &key, history, &mut output)?;
			output.unchanged_contracts.push(key);
			continue;
		}

		let latest_version = history
			.current_version()
			.expect("decoded migration histories always contain a current version");
		if ledger.version_is_frozen(&key, latest_version) {
			if latest.schema.same_wire(&source.schema) {
				extend_published_process(&key, history, source, &mut output)?;
				continue;
			}
			let next = next_migration_version(&key, latest_version, manifest.version_type)?;
			// The frozen version's recorded transition describes the hop
			// that produced it, not this new adjacent one. Its renames
			// are already baked into the stored schema, and replaying its
			// manual mode here would brand every hop after a hand-written
			// transition manual forever.
			let intent = resolve_field_changes(
				&key,
				&latest.schema,
				&source.schema,
				&SourceIntent::default(),
				answers,
				&mut output.data_warnings,
				&mut prompts,
			)?;
			let stale_ladder = supported_stale_ladder(history, next);
			let transition = create_transition(
				&project,
				TransitionRequest {
					identity: &history.identity,
					rust_name: &history.rust_name,
					source: latest,
					source_version: latest_version,
					stale_ladder: &stale_ladder,
					intent,
					destination_version: next,
					destination: &source.schema,
					destination_process: source.process.as_ref(),
					preserve_manual: false,
					version_type,
				},
				&mut output,
			)?;
			history.versions.push(SchemaVersion {
				schema: source.schema,
				process: source.process,
				transition: Some(transition),
			});
			output.advanced_versions.push(format!("{key}@{next}"));
		} else {
			let replacement = if latest_version == 0 {
				SchemaVersion {
					schema: source.schema,
					process: source.process,
					transition: None,
				}
			} else {
				let previous = history
					.version(latest_version - 1)
					.expect("decoded histories contain every adjacent prior version");
				// The draft's own transition carries the disambiguation
				// answers recorded when it was created; reusing them
				// keeps repeated `create` runs over the draft stable
				// instead of re-asking settled questions.
				let previous_intent = recorded_intent(latest.transition.as_ref());
				let intent = resolve_field_changes(
					&key,
					&previous.schema,
					&source.schema,
					&previous_intent,
					answers,
					&mut output.data_warnings,
					&mut prompts,
				)?;
				let stale_ladder = supported_stale_ladder(history, latest_version);
				let transition = create_transition(
					&project,
					TransitionRequest {
						identity: &history.identity,
						rust_name: &history.rust_name,
						source: previous,
						source_version: latest_version - 1,
						stale_ladder: &stale_ladder,
						intent,
						destination_version: latest_version,
						destination: &source.schema,
						destination_process: source.process.as_ref(),
						// A hand-written body is only valid for the byte
						// layouts it was written against. A process-only
						// change keeps it; any change to the destination
						// schema or to the recorded intent regenerates it.
						preserve_manual: latest.schema.same_wire(&source.schema)
							&& !converts_to_manual,
						version_type,
					},
					&mut output,
				)?;
				SchemaVersion {
					schema: source.schema,
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

	// A contract the source no longer declares is removed history, except an
	// instruction snapshot the source explicitly stopped recording: it carries
	// no envelope, so releasing it changes nothing on the wire.
	let dropped = auto.removed_since(&manifest.auto);
	let mut released = Vec::new();
	for (key, history) in &manifest.contracts {
		if seen.contains_key(key) {
			continue;
		}
		if snapshot_released(history, &auto, &current.opt_outs) {
			released.push(key.clone());
			continue;
		}
		if dropped.contains(&history.identity.kind) {
			return Err(envelope_removal(history));
		}
		return Err(MigrationError::ContractRemoved {
			kind: history.identity.kind.to_string(),
			name: history.rust_name.clone(),
			identity: key.clone(),
		});
	}
	for key in released {
		manifest.contracts.remove(&key);
		output.released_snapshots.push(key);
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
	output.version_type = manifest.version_type;
	if !manifest.auto.is_empty() {
		output.build_script = Some(ensure_build_script(&project.program_dir)?);
	}
	Ok(output)
}

/// Move a contract between the enveloped and snapshot-only framings.
///
/// Only an instruction can change framing, and only while nothing is
/// published: once live, existing clients send exactly the bytes they were
/// generated with, so neither adding nor removing the version byte can be
/// reconciled under the same discriminator.
fn change_envelope(
	ledger: &PublicationLedger,
	key: &str,
	history: &mut ContractHistory,
	source: CurrentContract,
	output: &mut CreateMigrationsOutput,
) -> Result<(), MigrationError> {
	if ledger.version_is_frozen(key, 0) {
		return Err(if history.envelope {
			envelope_removal(history)
		} else {
			MigrationError::EnvelopeAddition {
				name: history.rust_name.clone(),
				identity: key.to_owned(),
			}
		});
	}
	// Unpublished history holds a single draft version, so the new framing
	// starts a fresh baseline.
	history.envelope = source.envelope;
	history.versions = vec![SchemaVersion {
		schema: source.schema,
		process: source.process,
		transition: None,
	}];
	output.updated_drafts.push(format!("{key}@0"));
	Ok(())
}

/// Record a change to a history that carries no transitions.
///
/// An event is decoded with the schema of the version that emitted it, so a
/// published event gains a new version with nothing to convert. An instruction
/// recorded without an envelope has a single snapshot: a draft is replaced,
/// and a published one may only append optional accounts.
fn record_unmigrated(
	version_type: MigrationVersionType,
	ledger: &PublicationLedger,
	key: &str,
	history: &mut ContractHistory,
	source: CurrentContract,
	output: &mut CreateMigrationsOutput,
) -> Result<(), MigrationError> {
	let latest_version = history
		.current_version()
		.expect("decoded migration histories always contain a current version");
	let latest = &history.versions[latest_version as usize];
	if latest.schema.same_wire(&source.schema) && latest.process == source.process {
		output.unchanged_contracts.push(key.to_owned());
		return Ok(());
	}
	if !ledger.version_is_frozen(key, latest_version) {
		history.versions[latest_version as usize] = SchemaVersion {
			schema: source.schema,
			process: source.process,
			transition: None,
		};
		output
			.updated_drafts
			.push(format!("{key}@{latest_version}"));
		return Ok(());
	}
	if history.identity.kind == ContractKind::Event {
		let next = next_migration_version(key, latest_version, version_type)?;
		history.versions.push(SchemaVersion {
			schema: source.schema,
			process: None,
			transition: None,
		});
		output.advanced_versions.push(format!("{key}@{next}"));
		return Ok(());
	}
	if !latest.schema.same_wire(&source.schema) {
		return Err(MigrationError::PublishedPayloadChanged {
			name: history.rust_name.clone(),
			identity: key.to_owned(),
		});
	}
	extend_published_process(key, history, source, output)
}

/// Append optional accounts to a published instruction's current version.
///
/// A request built for the shorter list still parses: the accounts parser
/// reads a missing trailing optional slot as absent. The published payload is
/// unchanged, so no version is consumed and no transition is written; any
/// other account-list change fails closed.
fn extend_published_process(
	key: &str,
	history: &mut ContractHistory,
	source: CurrentContract,
	output: &mut CreateMigrationsOutput,
) -> Result<(), MigrationError> {
	let latest_version = history
		.current_version()
		.expect("decoded migration histories always contain a current version");
	let latest = &mut history.versions[latest_version as usize];
	transition::process_transition(
		&history.identity,
		&history.rust_name,
		latest.process.as_ref(),
		source.process.as_ref(),
	)?;
	latest.process = source.process;
	output
		.extended_processes
		.push(format!("{key}@{latest_version}"));
	Ok(())
}

/// Whether an unseen history is an instruction snapshot the source released.
///
/// A snapshot without an envelope is released when its declaration opts out
/// with `migrations = false` or the auto policy stops covering instructions.
/// An enveloped contract is never released this way: dropping its envelope
/// is a wire-format change.
fn snapshot_released(
	history: &ContractHistory,
	auto: &MigrationAuto,
	opt_outs: &[CurrentOptOut],
) -> bool {
	!history.envelope
		&& (!auto.contains(ContractKind::Instruction)
			|| opt_outs.iter().any(|opt_out| {
				opt_out.kind == ContractKind::Instruction && opt_out.rust_name == history.rust_name
			}))
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
/// `pina migrations create` and `fix:format` from fighting: the guard still fails
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
/// program that is already live where the auto policy widens — accounts
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
		// A snapshot without an envelope adds no byte to the wire.
		if !source.envelope || manifest.contracts.contains_key(&key) {
			continue;
		}
		names.push(format!("{} ({})", source.rust_name, key));
	}
	names
}

/// Reject an explicit `migrations = false` on a contract recorded with an
/// envelope.
fn reject_opt_outs(
	manifest: &MigrationManifest,
	opt_outs: &[CurrentOptOut],
) -> Result<(), MigrationError> {
	for opt_out in opt_outs {
		let Ok(history) = manifest.contract_for_source(opt_out.kind, &opt_out.rust_name) else {
			continue;
		};
		if history.envelope {
			return Err(envelope_removal(history));
		}
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
	// expanded against; without a manifest there is no policy at all.
	let auto = manifest
		.as_ref()
		.map_or_else(MigrationAuto::none, |manifest| manifest.auto.clone());
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
	validate_program_configuration(&current.program_id, &manifest)?;
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
		let latest_version = history.current_version().unwrap_or_else(|| {
			panic!("validated migration histories always contain a current version")
		});
		if history.envelope && !source.envelope {
			return Err(envelope_removal(history));
		}
		if !latest.schema.same_wire(&source.schema)
			|| latest.process != source.process
			|| history.envelope != source.envelope
		{
			return Err(MigrationError::SchemaDrift {
				kind: source.identity.kind.to_string(),
				name: source.rust_name,
				version: latest_version,
			});
		}
		verify_transition_files(project, &ledger, &key, history)?;
		statuses.push(MigrationStatus {
			identity: key.clone(),
			kind: source.identity.kind.to_string(),
			rust_name: source.rust_name,
			envelope: history.envelope,
			current_version: latest_version,
			published: ledger.ever_published(&key, latest_version),
			publication_pending: ledger.pending.as_ref().is_some_and(|pending| {
				pending
					.versions
					.get(&key)
					.is_some_and(|published| published.pins(latest_version))
			}),
			schema_sha256: latest.schema_sha256(),
			// A snapshot without an envelope never consumes a version.
			versions_remaining: if history.envelope {
				manifest
					.version_type
					.max_version()
					.saturating_sub(latest_version)
			} else {
				0
			},
		});
	}
	for (key, history) in &manifest.contracts {
		if seen.contains_key(key) {
			continue;
		}
		if snapshot_released(history, &auto, &current.opt_outs) {
			return Err(MigrationError::StaleSnapshot {
				name: history.rust_name.clone(),
				identity: key.clone(),
			});
		}
		return Err(MigrationError::ContractRemoved {
			kind: history.identity.kind.to_string(),
			name: history.rust_name.clone(),
			identity: key.clone(),
		});
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

/// The checked-in manifest's version-envelope width, read as tolerantly as
/// [`manifest_auto_policy`]: callers use it only to present account bytes.
pub(crate) fn manifest_version_type(program_dir: &Path) -> Option<MigrationVersionType> {
	load_manifest(&program_dir.join(MANIFEST_PATH))
		.ok()
		.flatten()
		.map(|manifest| manifest.version_type)
}

/// Verify history and return only the current constants needed by IDL codegen.
pub(crate) fn idl_migration_metadata(
	start: &Path,
) -> Result<Option<IdlMigrationMetadata>, MigrationError> {
	let project = Project::discover(start)?;
	let (statuses, manifest) = check_project_migrations_with_manifest(&project)?;
	let Some(manifest) = manifest.filter(|_| !statuses.is_empty()) else {
		return Ok(None);
	};

	Ok(Some(IdlMigrationMetadata {
		version_type: manifest.version_type,
		current_versions: statuses
			.into_iter()
			.map(|status| (status.identity, status.current_version))
			.collect(),
		historical_events: manifest
			.contracts
			.into_iter()
			.filter(|(_, history)| history.identity.kind == ContractKind::Event)
			.map(|(key, mut history)| {
				history.versions.pop();
				let schemas = history
					.versions
					.into_iter()
					.map(|version| version.schema)
					.collect();
				(key, schemas)
			})
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
