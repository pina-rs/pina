//! Interactive disambiguation questions and the answers that resolve them.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io::BufRead;
use std::io::Write;

use serde::Serialize;

/// One ambiguous field change that only the developer can resolve.
///
/// A source field disappeared and a destination field of the same type
/// appeared: either the field was renamed (its bytes must move) or the old
/// field was removed and a new one added (old data is discarded and the new
/// field starts zeroed). `pina migrations make` refuses to guess.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DisambiguationQuestion {
	/// Contract identity key the question belongs to.
	pub contract: String,
	/// Field name in the previous schema.
	pub from: String,
	/// Field name in the destination schema.
	pub to: String,
	/// Field type shared by both names.
	pub rust_type: String,
}

impl DisambiguationQuestion {
	pub(super) fn render_all(questions: &[DisambiguationQuestion]) -> String {
		let question_lines = questions
			.iter()
			.map(|question| {
				if question.to.is_empty() {
					format!(
						"\n  - `{}`: field `{}` (type `{}`) is removed and its stored data is \
						 discarded. Answer with `--assume-removed {}` to acknowledge the loss",
						question.contract, question.from, question.rust_type, question.from
					)
				} else {
					format!(
						"\n  - `{}`: was `{}` renamed to `{}` (type `{}`)? Answer with `--rename \
						 {}:{}` to preserve its data, or `--assume-removed {}` to discard the old \
						 field and zero-initialize `{}`",
						question.contract,
						question.from,
						question.to,
						question.rust_type,
						question.from,
						question.to,
						question.from,
						question.to
					)
				}
			})
			.collect::<String>();
		format!(
			"ambiguous field changes need an explicit answer before this migration can be \
			 generated{question_lines}\nRun `pina migrations make` again with those flags, or \
			 answer the prompts interactively on a terminal"
		)
	}
}

/// Developer answers for ambiguous schema changes.
#[derive(Clone, Debug, Default)]
pub struct MigrationAnswers {
	/// Renames the developer confirmed, keyed by source field name.
	pub(super) renames: BTreeMap<String, String>,
	/// Removals the developer explicitly acknowledged as data-dropping.
	pub(super) removed: BTreeSet<String>,
	/// Disable interactive prompts even when stdin is a terminal.
	pub(super) no_interactive: bool,
}

impl MigrationAnswers {
	/// Collect answers from repeatable CLI arguments.
	pub fn from_flags(
		renames: &[String],
		removed: &[String],
		no_interactive: bool,
	) -> Result<Self, String> {
		Self::parse_answers(renames, removed, no_interactive)
	}

	/// Layer CLI arguments over the persisted `[migrations.answers]` table.
	///
	/// Flags override the persisted answer for the same field. A removal
	/// that contradicts a persisted rename fails closed — the two answers
	/// disagree about whether the field's bytes survive — so a stale
	/// `pina.toml` cannot silently drop data.
	pub fn from_layers(
		persisted: &crate::project::MigrationsAnswersConfig,
		renames: &[String],
		removed: &[String],
		no_interactive: bool,
	) -> Result<Self, String> {
		let mut answers =
			Self::parse_answers(&persisted.rename, &persisted.assume_removed, no_interactive)?;
		let flag_answers = Self::parse_answers(renames, removed, no_interactive)?;
		for field in &flag_answers.removed {
			if let Some(persisted_to) = answers.renames.get(field) {
				return Err(format!(
					"`--assume-removed {field}` contradicts the persisted rename \
					 `{field}:{persisted_to}` in pina.toml; update one of them"
				));
			}
		}
		// Flag renames replace persisted answers for the same field, and a
		// flag rename moves a field out of the persisted removal set.
		for field in flag_answers.renames.keys() {
			answers.removed.remove(field.as_str());
		}
		answers.renames.extend(flag_answers.renames);
		answers.removed.extend(flag_answers.removed);
		Ok(answers)
	}

	fn parse_answers(
		renames: &[String],
		removed: &[String],
		no_interactive: bool,
	) -> Result<Self, String> {
		let mut answers = Self {
			renames: BTreeMap::new(),
			removed: removed.iter().cloned().collect(),
			no_interactive,
		};
		for rename in renames {
			let (from, to) = rename.split_once(':').ok_or_else(|| {
				format!("`--rename {rename}` must be written as `--rename from:to`")
			})?;
			if from.is_empty() || to.is_empty() {
				return Err(format!("`--rename {rename}` must name both fields"));
			}
			answers.renames.insert(from.to_owned(), to.to_owned());
		}
		Ok(answers)
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RenameAnswer {
	Rename,
	Remove,
	Abort,
}

/// Where prompts are written, where answers are read, and whether a human is
/// attached to answer them.
///
/// The make flow depends on this struct instead of the process stdio so tests
/// resolve disambiguations with scripted streams and every prompt branch runs
/// deterministically. Production attaches the terminal; a piped stdin simply
/// reports `interactive: false` and the question degrades to a flag error.
pub(super) struct PromptIo<'io> {
	input: &'io mut dyn BufRead,
	output: &'io mut dyn Write,
	interactive: bool,
}

impl<'io> PromptIo<'io> {
	pub(super) fn new(
		input: &'io mut dyn BufRead,
		output: &'io mut dyn Write,
		interactive: bool,
	) -> Self {
		Self {
			input,
			output,
			interactive,
		}
	}

	/// Whether prompts may be asked at all.
	pub(super) fn interactive(&self) -> bool {
		self.interactive
	}

	/// Prompt once for one plausible rename.
	///
	/// The loop tolerates two invalid answers; every third failure aborts so a
	/// wedged terminal cannot stall `pina migrations make` forever.
	pub(super) fn prompt_rename(&mut self, question: &DisambiguationQuestion) -> RenameAnswer {
		for _ in 0..3 {
			let _ = writeln!(
				self.output,
				"Field `{}` was removed and `{}` (same type `{}`) was added for `{}`.",
				question.from, question.to, question.rust_type, question.contract
			);
			let _ = writeln!(
				self.output,
				"Is this a rename? [y] rename and preserve data, [n] remove and zero `{}`",
				question.to
			);
			let _ = write!(self.output, "Answer (y/n): ");
			if self.output.flush().is_err() {
				return RenameAnswer::Abort;
			}
			let mut line = String::new();
			if self.input.read_line(&mut line).is_err() {
				return RenameAnswer::Abort;
			}
			match line.trim().to_ascii_lowercase().as_str() {
				"y" | "yes" | "rename" => return RenameAnswer::Rename,
				"n" | "no" | "remove" => return RenameAnswer::Remove,
				_ => {
					let _ = writeln!(self.output, "Answer `y` or `n`.");
				}
			}
		}
		RenameAnswer::Abort
	}

	/// Confirm one data-dropping removal.
	pub(super) fn prompt_removal(&mut self, question: &DisambiguationQuestion) -> RenameAnswer {
		for _ in 0..3 {
			let _ = writeln!(
				self.output,
				"Field `{}` (type `{}`) is removed for `{}` and its stored data is discarded.",
				question.from, question.rust_type, question.contract
			);
			let _ = write!(self.output, "Acknowledge the removal? (y/n): ");
			if self.output.flush().is_err() {
				return RenameAnswer::Abort;
			}
			let mut line = String::new();
			if self.input.read_line(&mut line).is_err() {
				return RenameAnswer::Abort;
			}
			match line.trim().to_ascii_lowercase().as_str() {
				"y" | "yes" | "remove" => return RenameAnswer::Remove,
				"n" | "no" | "abort" => return RenameAnswer::Abort,
				_ => {
					let _ = writeln!(self.output, "Answer `y` or `n`.");
				}
			}
		}
		RenameAnswer::Abort
	}
}
