//! Interactive disambiguation questions and the answers that resolve them.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

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

pub(super) enum RenameAnswer {
	Rename,
	Remove,
	Abort,
}

/// Prompt once on an interactive terminal for one rename question.
pub(super) fn prompt_rename(question: &DisambiguationQuestion) -> RenameAnswer {
	use std::io::BufRead as _;
	use std::io::Write as _;

	for _ in 0..3 {
		println!(
			"Field `{}` was removed and `{}` (same type `{}`) was added for `{}`.",
			question.from, question.to, question.rust_type, question.contract
		);
		println!(
			"Is this a rename? [y] rename and preserve data, [n] remove and zero `{}`",
			question.to
		);
		print!("Answer (y/n): ");
		if std::io::stdout().flush().is_err() {
			return RenameAnswer::Abort;
		}
		let mut line = String::new();
		if std::io::stdin().lock().read_line(&mut line).is_err() {
			return RenameAnswer::Abort;
		}
		match line.trim().to_ascii_lowercase().as_str() {
			"y" | "yes" | "rename" => return RenameAnswer::Rename,
			"n" | "no" | "remove" => return RenameAnswer::Remove,
			_ => {
				println!("Answer `y` or `n`.");
			}
		}
	}
	RenameAnswer::Abort
}

/// Confirm one data-dropping removal on an interactive terminal.
pub(super) fn prompt_removal(question: &DisambiguationQuestion) -> RenameAnswer {
	use std::io::BufRead as _;
	use std::io::Write as _;

	for _ in 0..3 {
		println!(
			"Field `{}` (type `{}`) is removed for `{}` and its stored data is discarded.",
			question.from, question.rust_type, question.contract
		);
		print!("Acknowledge the removal? (y/n): ");
		if std::io::stdout().flush().is_err() {
			return RenameAnswer::Abort;
		}
		let mut line = String::new();
		if std::io::stdin().lock().read_line(&mut line).is_err() {
			return RenameAnswer::Abort;
		}
		match line.trim().to_ascii_lowercase().as_str() {
			"y" | "yes" | "remove" => return RenameAnswer::Remove,
			"n" | "no" | "abort" => return RenameAnswer::Abort,
			_ => {
				println!("Answer `y` or `n`.");
			}
		}
	}
	RenameAnswer::Abort
}
