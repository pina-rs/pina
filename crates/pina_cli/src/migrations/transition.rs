//! Generation and verification of migration transition sources.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::path::PathBuf;

use pina_abi::ContractHistory;
use pina_abi::ContractIdentity;
use pina_abi::ContractKind;
use pina_abi::DataSchema;
use pina_abi::LayoutKind;
use pina_abi::MigrationVersionType;
use pina_abi::ProcessContract;
use pina_abi::ProcessTransition;
use pina_abi::PublicationLedger;
use pina_abi::SchemaVersion;
use pina_abi::Transition;
use pina_abi::TransitionMode;

use super::CreateMigrationsOutput;
use super::MigrationError;
use super::diff::MoveDirection;
use super::diff::automatic_direction;
use super::diff::transition_mode;
use super::remedy::ACCOUNT_GROWTH_REMEDY;
use super::remedy::LAMPORT_BUDGET_REMEDY;
use super::storage::write_atomic;
use crate::project::Project;

#[derive(Clone)]
pub(super) struct TransitionRequest<'a> {
	pub(super) identity: &'a ContractIdentity,
	pub(super) rust_name: &'a str,
	pub(super) source: &'a SchemaVersion,
	/// Version number of `source`: the index of the version this transition
	/// leaves.
	pub(super) source_version: u32,
	/// Every version a stale account may still hold while migrating inline to
	/// this destination, oldest first; the adjacent source is the last entry.
	/// Each entry carries its own version number, which the version index no
	/// longer stores.
	pub(super) stale_ladder: &'a [(u32, &'a SchemaVersion)],
	pub(super) renames: Vec<pina_abi::RenameMapping>,
	pub(super) destination_version: u32,
	pub(super) destination: &'a DataSchema,
	pub(super) destination_process: Option<&'a ProcessContract>,
	pub(super) preserve_manual: bool,
}

pub(super) fn create_transition(
	project: &Project,
	request: TransitionRequest<'_>,
	output: &mut CreateMigrationsOutput,
) -> Result<Transition, MigrationError> {
	let TransitionRequest {
		identity,
		rust_name,
		source,
		source_version,
		stale_ladder,
		renames,
		destination_version,
		destination,
		destination_process,
		preserve_manual,
	} = request;
	// The account-list proof is derived from the neighbouring versions on load
	// rather than stored, so this call only has to fail closed here.
	process_transition(
		identity,
		rust_name,
		source.process.as_ref(),
		destination_process,
	)?;
	let mode = transition_mode(&source.schema, destination);
	let path = transition_path(project, identity, source_version, destination_version);
	let generated = match mode {
		TransitionMode::Automatic => {
			automatic_transition_source(
				identity,
				project.migration_version_type,
				source,
				source_version,
				destination_version,
				destination,
			)
		}
		TransitionMode::Manual => {
			manual_transition_source(
				identity,
				project.migration_version_type,
				source,
				source_version,
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
	warn_about_account_growth(
		identity,
		rust_name,
		(source_version, source),
		stale_ladder,
		destination,
		project.migration_version_type.bytes(),
		output,
	);
	let implementation_sha256 = Some(hash_transition_file(&path)?);
	Ok(Transition {
		mode,
		renames,
		implementation_sha256,
	})
}

/// Approximate rent-exemption cost of one byte of account data.
///
/// Solana charges `LAMPORTS_PER_BYTE_YEAR * EXEMPTION_THRESHOLD`
/// (3,480 * 2) lamports per byte for a rent-exempt account; the constants
/// have been fixed since genesis, so this is a planning figure, not a quote.
pub(crate) const RENT_EXEMPT_LAMPORTS_PER_BYTE: u64 = 6_960;

/// Maximum account growth the Solana runtime permits one top-level
/// instruction to allocate, mirroring
/// `pina::MAX_PERMITTED_DATA_INCREASE` / `pinocchio::account::MAX_PERMITTED_DATA_INCREASE`.
///
/// A transition growing an account by more than this cannot migrate inline;
/// the executor rejects it with `MigrationAccountGrowthExceeded`.
pub(crate) const MAX_PERMITTED_DATA_INCREASE: usize = 10 * 1024;

/// Maximum adjacent transitions one inline ladder may walk, mirroring the
/// `pina_macros`-generated `MAX_INLINE_STEPS` (`current.min(8)`).
///
/// A stale account further behind its program's current version cannot
/// migrate inline and reports `MigrationUnavailable`; the ladder is the
/// window of versions this warning must cover.
pub(crate) const MAX_INLINE_STEPS: u32 = 8;

/// Every version a stale account may still hold while migrating inline to
/// `destination_version`, oldest first.
///
/// The generated `MAX_INLINE_STEPS` bounds the ladder at
/// `destination_version.min(8)` adjacent steps, and the executor captures the
/// account size before the first step, so each of these versions is a distinct
/// starting point for the runtime growth check. Each entry carries its own
/// version number because a history stores positions, not numbers; versions
/// missing from the history are skipped so a malformed manifest produces fewer
/// warnings instead of a panic.
pub(super) fn supported_stale_ladder(
	history: &ContractHistory,
	destination_version: u32,
) -> Vec<(u32, &SchemaVersion)> {
	let steps = destination_version.min(MAX_INLINE_STEPS);
	let first = destination_version.saturating_sub(steps);

	(first..destination_version)
		.filter_map(|version| {
			let entry = history.versions.get(version as usize)?;
			Some((version, entry))
		})
		.collect()
}

/// Largest allocation an inline ladder reaches from one stale version, and
/// the stale version it starts from.
struct WorstLadderGrowth {
	from_version: u32,
	from_size: usize,
	peak: usize,
}

/// Warn when a growing account must fund rent or can exceed the runtime's
/// per-instruction allocation cap.
///
/// Rent is funded per adjacent step from the migration payer, so the deficit
/// quote stays on the adjacent transition; an undersized `max_lamports` fails
/// the migration with `MigrationLamportBudgetExceeded` until the budget is
/// raised. The runtime cap, however, is measured against the account size
/// captured before the whole ladder: `MAX_INLINE_STEPS` lets a stale account
/// walk several adjacent transitions in one instruction, so the warning
/// evaluates the largest allocation reachable from every supported stale
/// version, not only the adjacent hop. Growth beyond that cap fails with
/// `MigrationAccountGrowthExceeded` however large the lamport budget is.
/// Fixed layouts quote exact byte growth; compact layouts quote the exact
/// worst-case growth the declared capacities imply. `adjacent` pairs the source
/// of the new transition with its version number; `stale_ladder` lists the
/// supported stale versions it walks from, oldest first.
pub(super) fn warn_about_account_growth(
	identity: &ContractIdentity,
	rust_name: &str,
	adjacent: (u32, &SchemaVersion),
	stale_ladder: &[(u32, &SchemaVersion)],
	destination: &DataSchema,
	version_bytes: usize,
	output: &mut CreateMigrationsOutput,
) {
	let (adjacent_version, adjacent) = adjacent;
	if identity.kind != ContractKind::Account {
		return;
	}
	let header = usize::from(identity.discriminator_bytes) + version_bytes;
	let destination_size = header.saturating_add(payload_size(destination));
	let adjacent_size = header.saturating_add(payload_size(&adjacent.schema));
	let compact = stale_ladder
		.iter()
		.any(|(_, source)| source.schema.layout == LayoutKind::Compact)
		|| destination.layout == LayoutKind::Compact;

	if destination_size > adjacent_size {
		let growth = destination_size - adjacent_size;
		let rent =
			RENT_EXEMPT_LAMPORTS_PER_BYTE.saturating_mul(u64::try_from(growth).unwrap_or(u64::MAX));
		let sizes = growth_descriptor(
			compact,
			adjacent_size,
			destination_size,
			&format!(
				"in transition v{adjacent_version} to v{}",
				adjacent_version + 1
			),
		);
		output.data_warnings.push(format!(
			"account `{rust_name}` {sizes}: a stale account funds roughly {rent} lamports of rent \
			 exemption from the migration payer, so pass a signer or PDA payer and \
			 {LAMPORT_BUDGET_REMEDY}; an undersized budget fails the migration with \
			 `MigrationLamportBudgetExceeded`",
		));
	}

	let worst = worst_ladder_growth(
		header,
		adjacent,
		adjacent_version,
		stale_ladder,
		destination,
	);
	if worst.peak.saturating_sub(worst.from_size) > MAX_PERMITTED_DATA_INCREASE {
		// A worst case starting at the adjacent version is the single
		// transition itself; anything older only exceeds the cap cumulatively,
		// and intermediates reset it only across separate transactions.
		let sizes = if worst.from_version == adjacent_version {
			growth_descriptor(
				compact,
				worst.from_size,
				worst.peak,
				&format!(
					"in transition v{adjacent_version} to v{}",
					adjacent_version + 1
				),
			)
		} else {
			let steps = adjacent_version + 1 - worst.from_version;
			growth_descriptor(
				compact,
				worst.from_size,
				worst.peak,
				&format!(
					"across the {steps}-step inline ladder v{} to v{} (an intermediate version \
					 only resets the runtime cap when it migrates in a separate transaction)",
					worst.from_version,
					adjacent_version + 1,
				),
			)
		};
		output.data_warnings.push(format!(
			"account `{rust_name}` {sizes}: that exceeds the runtime's \
			 `MAX_PERMITTED_DATA_INCREASE` ({MAX_PERMITTED_DATA_INCREASE} bytes) for a single \
			 instruction, so {ACCOUNT_GROWTH_REMEDY}; the runtime fails the migration with \
			 `MigrationAccountGrowthExceeded`",
		));
	}
}

/// Worst-case payload size for a schema.
///
/// Validated schemas always expose a size; an unmeasurable one saturates so an
/// unexpected layout over-warns instead of silently under-warning.
fn payload_size(schema: &DataSchema) -> usize {
	schema.maximum_payload_size().unwrap_or(usize::MAX)
}

/// Largest allocation reachable from a supported stale version, walking to the
/// destination. The executor compares each step's allocation against the size
/// captured before the ladder, so a middle version can be the worst starting
/// point even when the adjacent hop stays under the cap. The adjacent source
/// seeds the search so a ladder without stored entries still checks its own
/// transition.
fn worst_ladder_growth(
	header: usize,
	adjacent: &SchemaVersion,
	adjacent_version: u32,
	stale_ladder: &[(u32, &SchemaVersion)],
	destination: &DataSchema,
) -> WorstLadderGrowth {
	let destination_size = header.saturating_add(payload_size(destination));
	let adjacent_size = header.saturating_add(payload_size(&adjacent.schema));
	let mut worst = WorstLadderGrowth {
		from_version: adjacent_version,
		from_size: adjacent_size,
		peak: adjacent_size.max(destination_size),
	};
	for (index, (from_version, source)) in stale_ladder.iter().enumerate() {
		let from_size = header.saturating_add(payload_size(&source.schema));
		let mut peak = from_size;
		for (_, later) in &stale_ladder[index + 1..] {
			peak = peak.max(header.saturating_add(payload_size(&later.schema)));
		}
		peak = peak.max(destination_size);
		let growth = peak.saturating_sub(from_size);
		if growth > worst.peak.saturating_sub(worst.from_size) {
			worst = WorstLadderGrowth {
				from_version: *from_version,
				from_size,
				peak,
			};
		}
	}
	worst
}

/// Format the byte-growth clause shared by the rent and runtime-cap warnings.
fn growth_descriptor(compact: bool, from: usize, to: usize, transition: &str) -> String {
	if compact {
		format!("worst-case size grows from {from} to {to} bytes (compact capacity) {transition}")
	} else {
		format!("grows from {from} to {to} bytes {transition}")
	}
}

pub(super) fn process_transition(
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

pub(super) fn automatic_transition_source(
	identity: &ContractIdentity,
	version_type: MigrationVersionType,
	source: &SchemaVersion,
	source_version: u32,
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
		"// @generated by `pina migrations create`; do not edit an automatic \
		 transition.\npub(crate) const FROM_VERSION: u32 = {source_version};\npub(crate) const \
		 TO_VERSION: u32 = {destination_version};\npub(crate) const SOURCE_SIZE: usize = \
		 {source_size};\npub(crate) const DESTINATION_SIZE: usize = \
		 {destination_size};\n\npub(crate) const WORKING_SIZE: usize = \
		 {working_size};\n\npub(crate) fn migrate(data: &mut [u8]) {{\n\tif data.len() < \
		 WORKING_SIZE {{\n\t\treturn;\n\t}}\n{moves}{zeroes}}}\n",
	)
}

pub(super) fn manual_transition_source(
	identity: &ContractIdentity,
	version_type: MigrationVersionType,
	source: &SchemaVersion,
	source_version: u32,
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
		"// Manual adjacent ABI migration generated by `pina migrations create`.\n// Source \
		 version: {source_version} ({source_description} bytes)\n// Destination version: \
		 {destination_version} ({destination_description} bytes)\n// TODO(pina-manual-migration): \
		 {requirement}.\n{sizing}\n{migrate}",
	))
}

pub(super) fn transition_path(
	project: &Project,
	identity: &ContractIdentity,
	from: u32,
	to: u32,
) -> PathBuf {
	project
		.program_dir
		.join(pina_abi::transition_path(identity, from, to))
}

pub(super) fn hash_transition_file(path: &Path) -> Result<String, MigrationError> {
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

pub(super) fn verify_transition_files(
	project: &Project,
	ledger: &PublicationLedger,
	key: &str,
	history: &ContractHistory,
) -> Result<(), MigrationError> {
	// A transition sits on the version it converts into, so the entering
	// transition of version `number` is the step from `number - 1`.
	for (number, version) in history.versions.iter().enumerate().skip(1) {
		let number = u32::try_from(number).map_err(|_| {
			MigrationError::InvalidHistory(format!(
				"contract `{key}` has more versions than u32 can index"
			))
		})?;
		let transition = version.transition.as_ref().ok_or_else(|| {
			MigrationError::InvalidHistory(format!(
				"contract `{key}` version {number} has no transition"
			))
		})?;
		let path = transition_path(project, &history.identity, number - 1, number);
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
			if ledger.version_is_frozen(key, number) {
				return Err(MigrationError::FrozenImplementationChanged { path });
			}
			return Err(MigrationError::TransitionDrift {
				kind: history.identity.kind.to_string(),
				name: history.rust_name.clone(),
				version: number,
				path,
			});
		}
	}
	Ok(())
}

pub(super) fn refresh_draft_transition_hash(
	project: &Project,
	ledger: &PublicationLedger,
	key: &str,
	history: &mut ContractHistory,
	output: &mut CreateMigrationsOutput,
) -> Result<(), MigrationError> {
	let latest_version = history
		.current_version()
		.unwrap_or_else(|| panic!("decoded migration histories always contain a current version"));
	let latest = history
		.versions
		.last_mut()
		.expect("decoded migration histories always contain a current version");
	let Some(transition) = latest.transition.as_mut() else {
		return Ok(());
	};
	let path = transition_path(
		project,
		&history.identity,
		latest_version - 1,
		latest_version,
	);
	if !path.is_file() {
		return Err(MigrationError::MissingTransition { path });
	}
	let hash = hash_transition_file(&path)?;
	if transition.implementation_sha256.as_deref() == Some(hash.as_str()) {
		return Ok(());
	}
	if ledger.version_is_frozen(key, latest_version) {
		return Err(MigrationError::FrozenImplementationChanged { path });
	}
	transition.implementation_sha256 = Some(hash);
	output
		.updated_drafts
		.push(format!("{key}@{latest_version}"));
	Ok(())
}
