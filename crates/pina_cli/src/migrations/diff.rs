//! Schema diffing, rename resolution, and transition-mode decisions.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use pina_abi::DataSchema;
use pina_abi::FieldSchema;
use pina_abi::LayoutKind;
use pina_abi::SchemaVersion;
use pina_abi::TransitionMode;

use super::MigrationError;
use super::prompt::DisambiguationQuestion;
use super::prompt::MigrationAnswers;
use super::prompt::PromptIo;
use super::prompt::RenameAnswer;

/// Resolve every ambiguous field change for one contract against the answers.
///
/// Previously recorded transition renames count as answers so repeated `make`
/// runs over a draft stay stable. Unanswered candidates become interactive
/// prompts on a terminal, or a structured error that names the exact flags
/// that answer them.
pub(super) fn resolve_field_changes(
	contract: &str,
	source: &DataSchema,
	destination: &DataSchema,
	previous_renames: &[pina_abi::RenameMapping],
	answers: &MigrationAnswers,
	warnings: &mut Vec<String>,
	prompts: &mut PromptIo<'_>,
) -> Result<(Vec<pina_abi::RenameMapping>, BTreeSet<String>), MigrationError> {
	// Only renames whose source field still exists apply to this hop;
	// earlier hops' renames are already baked into the stored schema.
	let mut renames: Vec<pina_abi::RenameMapping> = previous_renames
		.iter()
		.filter(|mapping| source.fields.iter().any(|field| field.name == mapping.from))
		.cloned()
		.collect();
	let mut dropped = answers.removed.clone();

	for mapping in &renames {
		if let Some(to) = answers.renames.get(&mapping.from)
			&& *to != mapping.to
		{
			return Err(MigrationError::InvalidHistory(format!(
				"contract `{contract}` previously recorded the rename `{}:{}`; `--rename {}:{}` \
				 contradicts it",
				mapping.from, mapping.to, mapping.from, to
			)));
		}
	}

	let source_names = source
		.fields
		.iter()
		.map(|field| field.name.as_str())
		.collect::<BTreeSet<_>>();
	let destination_names = destination
		.fields
		.iter()
		.map(|field| field.name.as_str())
		.collect::<BTreeSet<_>>();

	let removed_fields: Vec<&FieldSchema> = source
		.fields
		.iter()
		.filter(|field| !destination_names.contains(field.name.as_str()))
		.collect();
	let added: Vec<&FieldSchema> = destination
		.fields
		.iter()
		.filter(|field| !source_names.contains(field.name.as_str()))
		.collect();
	let added_types = added
		.iter()
		.map(|field| (field.name.as_str(), field.rust_type.as_str()))
		.collect::<BTreeMap<_, _>>();

	// Validate every answer against this diff before it can influence a
	// transition: `--assume-removed` naming a retained field would silently
	// zero its live data through the effective schema, and `--rename` naming
	// anything other than a removed-to-added pair would be silently ignored.
	let removed_types = removed_fields
		.iter()
		.map(|field| (field.name.as_str(), field.rust_type.as_str()))
		.collect::<BTreeMap<_, _>>();
	for field in &answers.removed {
		if !removed_types.contains_key(field.as_str()) {
			return Err(MigrationError::InvalidHistory(format!(
				"contract `{contract}`: `--assume-removed {field}` names a field that was not \
				 removed by this change",
			)));
		}
		if answers.renames.contains_key(field) {
			return Err(MigrationError::InvalidHistory(format!(
				"contract `{contract}`: field `{field}` is answered with both `--rename` and \
				 `--assume-removed`; pick one",
			)));
		}
	}
	for (from, to) in &answers.renames {
		if !removed_types.contains_key(from.as_str()) {
			return Err(MigrationError::InvalidHistory(format!(
				"contract `{contract}`: `--rename {from}:{to}` names a field that was not removed \
				 by this change",
			)));
		}
		// The target must be an added field: a retained name would let one
		// `--rename` overwrite another field's live bytes through the
		// effective schema.
		let Some(&to_type) = added_types.get(to.as_str()) else {
			return Err(MigrationError::InvalidHistory(format!(
				"contract `{contract}`: `--rename {from}:{to}` targets `{to}`, which is not an \
				 added field",
			)));
		};
		let &from_type = removed_types
			.get(from.as_str())
			.expect("removed_types was just checked for `from`");
		if from_type != to_type {
			return Err(MigrationError::InvalidHistory(format!(
				"contract `{contract}`: `--rename {from}:{to}` changes the field type \
				 (`{from_type}` to `{to_type}`); write a manual transition instead",
			)));
		}
	}

	// Two renames onto one added field would duplicate that name in the
	// effective schema and overwrite its bytes twice.
	let mut claimed_targets: BTreeSet<String> =
		renames.iter().map(|mapping| mapping.to.clone()).collect();
	for target in answers.renames.values() {
		if !claimed_targets.insert(target.clone()) {
			return Err(MigrationError::InvalidHistory(format!(
				"contract `{contract}`: two renames target `{target}`; each added field can \
				 receive at most one renamed source field",
			)));
		}
	}

	let mut questions = Vec::new();
	for removed_field in &removed_fields {
		if renames
			.iter()
			.any(|mapping| mapping.from == removed_field.name)
		{
			continue;
		}
		// An explicit `--rename from:to` selects the target itself, so a
		// developer can correct the same-type pairing the heuristic proposes.
		// Validation above guarantees `to` is an added field of the same type.
		if let Some(to) = answers.renames.get(removed_field.name.as_str()) {
			claimed_targets.insert(to.clone());
			renames.push(pina_abi::RenameMapping {
				from: removed_field.name.clone(),
				to: to.clone(),
			});
			continue;
		}
		// Pair the removal with the first unclaimed addition of the same
		// type: that is the rename this change most plausibly represents.
		// Claimed targets — by recorded or answered renames, by earlier
		// questions, or by earlier removals — never pair twice.
		let candidate = added.iter().find(|added_field| {
			added_field.rust_type == removed_field.rust_type
				&& !claimed_targets.contains(added_field.name.as_str())
		});
		if dropped.contains(removed_field.name.as_str()) {
			// The developer answered the rename question with an explicit
			// removal: the old data is discarded and the paired new field, if
			// any, starts zeroed.
			if let Some(candidate) = candidate {
				claimed_targets.insert(candidate.name.clone());
				warnings.push(format!(
					"contract `{contract}`: field `{}` is removed by this migration and its \
					 stored data is discarded; `{}` starts zeroed",
					removed_field.name, candidate.name
				));
			} else {
				warnings.push(format!(
					"contract `{contract}`: field `{}` (type `{}`) is removed by this migration \
					 and its stored data is discarded",
					removed_field.name, removed_field.rust_type
				));
			}
			continue;
		}
		let Some(candidate) = candidate else {
			// An unpaired removal discards stored data, so it needs the same
			// explicit acknowledgement as a declined rename. A field whose
			// name reappears with a different type is not removed at all —
			// the mode classifier sends that diff to a manual transition.
			let question = DisambiguationQuestion {
				contract: contract.to_owned(),
				from: removed_field.name.clone(),
				to: String::new(),
				rust_type: removed_field.rust_type.clone(),
			};
			if answers.no_interactive || !prompts.interactive() {
				questions.push(question);
				continue;
			}
			match prompts.prompt_removal(&question) {
				RenameAnswer::Remove => {
					dropped.insert(removed_field.name.clone());
					warnings.push(format!(
						"contract `{contract}`: field `{}` (type `{}`) is removed by this \
						 migration and its stored data is discarded",
						removed_field.name, removed_field.rust_type
					));
				}
				RenameAnswer::Rename | RenameAnswer::Abort => {
					questions.push(question);
				}
			}
			continue;
		};
		let question = DisambiguationQuestion {
			contract: contract.to_owned(),
			from: removed_field.name.clone(),
			to: candidate.name.clone(),
			rust_type: removed_field.rust_type.clone(),
		};
		// The pairing is claimed either way: an answered rename moves the
		// bytes, and an acknowledged removal or a pending question explains
		// the new field without leaving it free for a later iteration.
		claimed_targets.insert(candidate.name.clone());
		if answers.no_interactive || !prompts.interactive() {
			questions.push(question);
			continue;
		}
		match prompts.prompt_rename(&question) {
			RenameAnswer::Rename => {
				renames.push(pina_abi::RenameMapping {
					from: removed_field.name.clone(),
					to: candidate.name.clone(),
				});
			}
			RenameAnswer::Remove => {
				dropped.insert(removed_field.name.clone());
				warnings.push(format!(
					"contract `{contract}`: field `{}` is removed by this migration and its \
					 stored data is discarded; `{}` starts zeroed",
					removed_field.name, candidate.name
				));
			}
			RenameAnswer::Abort => {
				questions.push(question);
			}
		}
	}

	if !questions.is_empty() {
		return Err(MigrationError::DisambiguationRequired { questions });
	}

	renames.sort();
	Ok((renames, dropped))
}

/// Apply confirmed renames and acknowledged removals to a copy of the source
/// schema so the ordinary diff, mode classifier, and generator see the
/// developer's intent instead of the ambiguous raw diff.
pub(super) fn effective_source_schema(
	source: &SchemaVersion,
	renames: &[pina_abi::RenameMapping],
	dropped: &BTreeSet<String>,
) -> Result<SchemaVersion, MigrationError> {
	let mut fields = source.schema.fields.clone();
	for field in &mut fields {
		if let Some(mapping) = renames.iter().find(|mapping| mapping.from == field.name) {
			field.name = mapping.to.clone();
		}
	}
	fields.retain(|field| !dropped.contains(field.name.as_str()));
	let rebuilt = DataSchema::try_new(source.schema.layout, fields).map_err(|reason| {
		MigrationError::InvalidHistory(format!(
			"resolved field changes produce an invalid schema: {reason}"
		))
	})?;
	let mut effective = source.clone();
	effective.schema = rebuilt;
	Ok(effective)
}

pub(super) fn transition_mode(source: &DataSchema, destination: &DataSchema) -> TransitionMode {
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
pub(super) enum MoveDirection {
	Forward,
	Backward,
}

pub(super) fn automatic_direction(
	source: &DataSchema,
	destination: &DataSchema,
) -> Option<MoveDirection> {
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
