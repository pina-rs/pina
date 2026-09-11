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

use super::MakeMigrationsOutput;
use super::MigrationError;
use super::diff::MoveDirection;
use super::diff::automatic_direction;
use super::diff::transition_mode;
use super::storage::write_atomic;
use crate::project::Project;

#[derive(Clone)]
pub(super) struct TransitionRequest<'a> {
	pub(super) identity: &'a ContractIdentity,
	pub(super) rust_name: &'a str,
	pub(super) source: &'a SchemaVersion,
	pub(super) renames: Vec<pina_abi::RenameMapping>,
	pub(super) destination_version: u32,
	pub(super) destination: &'a DataSchema,
	pub(super) destination_process: Option<&'a ProcessContract>,
	pub(super) preserve_manual: bool,
}

pub(super) fn create_transition(
	project: &Project,
	request: TransitionRequest<'_>,
	output: &mut MakeMigrationsOutput,
) -> Result<Transition, MigrationError> {
	let TransitionRequest {
		identity,
		rust_name,
		source,
		renames,
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
	warn_about_account_growth(
		identity,
		rust_name,
		source,
		destination,
		project.migration_version_type.bytes(),
		output,
	);
	let implementation_sha256 = Some(hash_transition_file(&path)?);
	Ok(Transition {
		from: source.version,
		to: destination_version,
		mode,
		renames,
		source_schema_sha256: source.schema_sha256.clone(),
		destination_schema_sha256: destination.sha256(),
		source_process_sha256: source.process_sha256.clone(),
		destination_process_sha256: destination_process.map(ProcessContract::sha256),
		process,
		implementation_sha256,
	})
}

/// Approximate rent-exemption cost of one byte of account data.
///
/// Solana charges `LAMPORTS_PER_BYTE_YEAR * EXEMPTION_THRESHOLD`
/// (3,480 * 2) lamports per byte for a rent-exempt account; the constants
/// have been fixed since genesis, so this is a planning figure, not a quote.
pub(super) const RENT_EXEMPT_LAMPORTS_PER_BYTE: u64 = 6_960;

/// Warn when a transition grows an account, because a stale account must
/// fund the rent deficit from the migration payer inside the touching
/// transaction. An undersized lamport budget makes those migrations fail
/// with `MigrationBudgetExceeded` until the budget is raised.
pub(super) fn warn_about_account_growth(
	identity: &ContractIdentity,
	rust_name: &str,
	source: &SchemaVersion,
	destination: &DataSchema,
	version_bytes: usize,
	output: &mut MakeMigrationsOutput,
) {
	if identity.kind != ContractKind::Account {
		return;
	}
	let header = usize::from(identity.discriminator_bytes) + version_bytes;
	if let (Some(from_payload), Some(to_payload)) = (
		source.schema.fixed_payload_size(),
		destination.fixed_payload_size(),
	) && to_payload > from_payload
	{
		let growth = to_payload - from_payload;
		let rent =
			RENT_EXEMPT_LAMPORTS_PER_BYTE.saturating_mul(u64::try_from(growth).unwrap_or(u64::MAX));
		output.data_warnings.push(format!(
			"account `{rust_name}` grows from {} to {} bytes in transition v{} to v{}: a stale \
			 account funds roughly {rent} lamports of rent exemption from the migration payer, so \
			 size the invoking instruction's lamport budget and pass a signer or PDA payer",
			header + from_payload,
			header + to_payload,
			source.version,
			source.version + 1,
		));
	} else if destination.layout == LayoutKind::Compact {
		output.data_warnings.push(format!(
			"account `{rust_name}` keeps a compact layout in transition v{} to v{}: capacity \
			 growth funds rent from the migration payer, so confirm the invoking instruction's \
			 lamport budget covers rent exemption at the new capacity",
			source.version,
			source.version + 1,
		));
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

pub(super) fn manual_transition_source(
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

pub(super) fn refresh_draft_transition_hash(
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
