//! Schema diffing, rename resolution, and transition-mode decisions.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use pina_abi::DataSchema;
use pina_abi::FieldSchema;
use pina_abi::LayoutKind;
#[cfg(test)]
use pina_abi::TransitionMode;

use super::MigrationError;
use super::prompt::DisambiguationQuestion;
use super::prompt::MigrationAnswers;
use super::prompt::PromptIo;
use super::prompt::RenameAnswer;

/// Resolve every ambiguous field change for one contract against the answers.
///
/// Previously recorded transition renames count as answers so repeated `create`
/// runs over a draft stay stable. Unanswered candidates become interactive
/// prompts on a terminal, or a structured error that names the exact flags
/// that answer them.
///
/// A rename that would otherwise be written automatically can instead be
/// answered with a manual conversion when the developer needs to transform the
/// bytes rather than move them, which is what `--manual <field>` records.
pub(super) fn resolve_field_changes(
	contract: &str,
	source: &DataSchema,
	destination: &DataSchema,
	previous: &SourceIntent,
	answers: &MigrationAnswers,
	warnings: &mut Vec<String>,
	prompts: &mut PromptIo<'_>,
) -> Result<SourceIntent, MigrationError> {
	// Only renames whose source field still exists apply to this hop;
	// earlier hops' renames are already baked into the stored schema.
	let mut renames: Vec<pina_abi::RenameMapping> = previous
		.renames
		.iter()
		.filter(|mapping| source.fields.iter().any(|field| field.name == mapping.from))
		.cloned()
		.collect();
	let mut dropped = answers.removed.clone();
	// A manual answer names a destination field; the stored field it replaces
	// carries the bytes the developer converts by hand.
	let manual = answers.manual.clone();

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

	// A manual answer has to name an added field. Naming a retained field would
	// ask for a hand-written value on a field the destination already reads
	// verbatim, and the answer would be silently dropped.
	for field in &manual {
		if !added_types.contains_key(field.as_str()) {
			return Err(MigrationError::InvalidHistory(format!(
				"contract `{contract}`: `--manual {field}` names a field that this change did not \
				 add; a manual conversion replaces an added field's value",
			)));
		}
	}
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
		// A generated rename copies bytes verbatim, so the two types must match
		// exactly. `--manual` is the escape hatch that makes a differing type
		// legal: the developer writes the conversion, so the width and meaning
		// of the destination are theirs to decide.
		if from_type != to_type && !manual.contains(to.as_str()) {
			return Err(MigrationError::InvalidHistory(format!(
				"contract `{contract}`: `--rename {from}:{to}` changes the field type \
				 (`{from_type}` to `{to_type}`); add `--manual {to}` to write that conversion by \
				 hand, or pick a field of the same type",
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
		// A developer who asked for a manual conversion already owns this
		// field's value, so the pairing question is settled by naming the
		// destination: the stored bytes are the input to their conversion.
		if manual.contains(candidate.name.as_str()) {
			claimed_targets.insert(candidate.name.clone());
			renames.push(pina_abi::RenameMapping {
				from: removed_field.name.clone(),
				to: candidate.name.clone(),
			});
			continue;
		}
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
	Ok(SourceIntent {
		renames,
		dropped,
		manual: manual.clone(),
		// A `--manual` answer this run is as durable as a recorded one: it keeps
		// the developer's body across later refreshes of the same draft.
		force_manual: previous.force_manual || !manual.is_empty(),
	})
}

/// The developer's resolved reading of one schema change.
///
/// Field *pairing* follows these logical names, but every byte offset in a
/// generated transition comes from the stored schema: a renamed field keeps its
/// original name on the wire, and a dropped field keeps occupying its bytes.
#[derive(Clone, Debug, Default)]
pub(super) struct SourceIntent {
	/// Confirmed renames, pairing an original field name with its new one.
	pub(super) renames: Vec<pina_abi::RenameMapping>,
	/// Fields whose stored bytes the developer acknowledged discarding.
	pub(super) dropped: BTreeSet<String>,
	/// Fields whose conversion the developer writes by hand.
	pub(super) manual: BTreeSet<String>,
	/// Whether the developer owns this conversion outright.
	///
	/// Distinct from a non-empty `manual`: a recorded manual transition is a
	/// standing instruction even when it names no field, which is the case when
	/// the change was unprovable (a type change, say) rather than answered. It
	/// is what a draft refresh replays so a developer-authored body survives.
	pub(super) force_manual: bool,
}

impl SourceIntent {
	/// The stored field name whose bytes a destination field inherits.
	fn renamed_from(&self, destination: &str) -> Option<&str> {
		self.renames
			.iter()
			.find(|mapping| mapping.to == destination)
			.map(|mapping| mapping.from.as_str())
	}
}

/// Byte-level proof that one adjacent transition is expressible as in-place
/// copies plus zero fills.
///
/// Offsets are payload-relative, so the generator and its tests reason about a
/// transition without knowing the discriminator or version width.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MovePlan {
	/// Stored payload size the plan reads from.
	pub(super) source_size: usize,
	/// Destination payload size the plan writes.
	pub(super) destination_size: usize,
	/// `(stored offset, destination offset, size)` for each field whose bytes
	/// survive the change.
	pub(super) moves: Vec<(usize, usize, usize)>,
	/// `(destination offset, size)` for each field with no stored counterpart.
	pub(super) zero_fills: Vec<(usize, usize)>,
	/// Order that keeps every copy from overwriting a source a later copy still
	/// has to read.
	pub(super) direction: MoveDirection,
}

impl MovePlan {
	/// Surviving-field copies in an order that cannot clobber an unread source.
	///
	/// Copying a field overwrites its destination, which may still hold bytes a
	/// later copy reads. When every field moves to a higher offset, the highest
	/// source must be read first; when none does, ascending source order is
	/// safe because each write lands at or below a source already consumed.
	pub(super) fn ordered_moves(&self) -> Vec<(usize, usize, usize)> {
		let mut moves = self.moves.clone();
		match self.direction {
			MoveDirection::Forward => moves.sort_by_key(|(source, ..)| *source),
			MoveDirection::Backward => moves.sort_by_key(|(source, ..)| std::cmp::Reverse(*source)),
		}
		moves
	}
}

/// How a surviving field's bytes travel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MoveDirection {
	/// No field moves to a higher offset, so copies run in ascending order.
	Forward,
	/// Every field moves to a higher offset, so copies run in descending order.
	Backward,
}

/// Prove and plan the byte movement for an automatic transition.
///
/// `None` means the change cannot be expressed as ordered in-place copies plus
/// zero fills, and the developer owns a manual transition. The proof is the
/// single source of truth for both the mode decision and the generated bytes,
/// so a transition can never be labelled automatic and then emit offsets that
/// contradict its own plan.
pub(super) fn automatic_move_plan(
	stored: &DataSchema,
	intent: &SourceIntent,
	destination: &DataSchema,
) -> Option<MovePlan> {
	// A manual conversion is developer-owned by definition: the generated bytes
	// for that field would be a guess, so the change is not expressible as
	// copies and zero fills.
	if intent.force_manual || !intent.manual.is_empty() {
		return None;
	}
	if stored.layout != LayoutKind::Fixed || destination.layout != LayoutKind::Fixed {
		return None;
	}
	let source_offsets = stored.fixed_field_offsets()?;
	let destination_offsets = destination.fixed_field_offsets()?;
	let mut moves = Vec::with_capacity(destination.fields.len());
	let mut zero_fills = Vec::new();
	let mut matched = BTreeSet::new();
	for field in &destination.fields {
		let &(destination_offset, destination_size) = destination_offsets.get(&field.name)?;
		let stored_name = intent
			.renamed_from(field.name.as_str())
			.unwrap_or(field.name.as_str());
		let Some(&(source_offset, source_size)) = source_offsets.get(stored_name) else {
			// No stored counterpart: a genuinely new field, which starts zeroed
			// instead of copying. A rename that lands on a retained name is
			// rejected by `resolve_field_changes` before it reaches this proof.
			zero_fills.push((destination_offset, destination_size));
			continue;
		};
		// The stored type must match exactly. Equal width is not enough: reading
		// `u64` bytes as `i64`, or `u32` as `f32`, silently reinterprets a live
		// value, so those changes stay manual.
		let stored_type = stored
			.fields
			.iter()
			.find(|candidate| candidate.name == stored_name)
			.map(|candidate| candidate.rust_type.as_str());
		if stored_type != Some(field.rust_type.as_str()) {
			return None;
		}
		// Two destination fields may not read one stored field: the second copy
		// would duplicate bytes the developer meant to place once.
		if !matched.insert(stored_name) {
			return None;
		}
		moves.push((source_offset, destination_offset, source_size));
	}
	// Every stored field must be accounted for: either its bytes move to a
	// destination field or the developer explicitly discarded them. A retained
	// name whose type changed fails here, because nothing pairs it.
	let accounted = stored
		.fields
		.iter()
		.all(|field| matched.contains(field.name.as_str()) || intent.dropped.contains(&field.name));
	if !accounted {
		return None;
	}
	let direction = move_direction(&moves)?;
	Some(MovePlan {
		source_size: stored.fixed_payload_size()?,
		destination_size: destination.fixed_payload_size()?,
		moves,
		zero_fills,
		direction,
	})
}

/// Order the copies must run in, or `None` when no single order is safe.
///
/// A field moving right can overwrite a source that a field moving left has yet
/// to read, so a change that moves one field each way is not expressible as a
/// sequence of copies and needs the developer.
fn move_direction(moves: &[(usize, usize, usize)]) -> Option<MoveDirection> {
	let mut moves_left = false;
	let mut moves_right = false;
	for (source, destination, _) in moves {
		moves_left |= destination < source;
		moves_right |= destination > source;
	}
	match (moves_left, moves_right) {
		(true, true) => None,
		(false, true) => Some(MoveDirection::Backward),
		(true | false, false) => Some(MoveDirection::Forward),
	}
}

/// The mode one adjacent transition is recorded with.
///
/// Kept as a named entry point so tests can classify a change without
/// generating it. It is exactly the plan's own verdict, which is what keeps a
/// recorded mode from disagreeing with the bytes `make` writes.
#[cfg(test)]
pub(super) fn transition_mode(
	stored: &DataSchema,
	intent: &SourceIntent,
	destination: &DataSchema,
) -> TransitionMode {
	if automatic_move_plan(stored, intent, destination).is_some() {
		TransitionMode::Automatic
	} else {
		TransitionMode::Manual
	}
}
