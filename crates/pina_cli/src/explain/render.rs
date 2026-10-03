//! Terminal rendering of an explanation.

use std::fmt::Write;

use super::Candidate;
use super::Confidence;
use super::ErrorKind;
use super::ExplainReport;
use super::FailureReport;
use crate::doctor::escape_controls;

/// How many construction sites the text report lists.
const ERROR_SITE_LINES: usize = 8;

impl ExplainReport {
	/// Render stable, color-free terminal output.
	///
	/// Every value read from the transaction or the source, including program
	/// logs, has its control characters escaped so a crafted log line cannot
	/// rewrite the terminal.
	#[must_use]
	pub fn render_text(&self) -> String {
		let mut output = String::new();
		let signature = escape_controls(&self.signature);
		let source = escape_controls(&self.source);

		let Some(failure) = &self.failure else {
			let _ = writeln!(
				output,
				"Transaction {signature} succeeded at slot {} (from {source}); there is nothing \
				 to explain.",
				self.slot
			);
			return output;
		};

		let _ = writeln!(
			output,
			"Transaction {signature} (slot {}, from {source})",
			self.slot
		);
		self.render_failure(&mut output, failure);
		self.render_candidates(&mut output);
		self.render_error_sites(&mut output);
		self.render_accounts(&mut output);

		if !self.logs.is_empty() {
			let _ = writeln!(output, "\nProgram logs (last {} lines):", self.logs.len());
			for line in &self.logs {
				let _ = writeln!(output, "  {}", escape_controls(line));
			}
		}

		if !self.notes.is_empty() {
			let _ = writeln!(output, "\nNotes:");
			for note in &self.notes {
				let _ = writeln!(output, "  - {}", escape_controls(note));
			}
		}

		output
	}

	fn render_failure(&self, output: &mut String, failure: &FailureReport) {
		match (failure.instruction_index, &failure.program_id) {
			(Some(index), Some(program_id)) if !failure.targets_program => {
				let _ = writeln!(
					output,
					"Instruction #{index} failed in {}, not in {}",
					escape_controls(program_id),
					escape_controls(&self.program.name)
				);
			}
			(Some(index), Some(_)) => {
				let instruction = failure.instruction.as_ref().map_or_else(
					|| "unknown instruction".to_owned(),
					|instruction| {
						let accounts = instruction
							.accounts_struct
							.as_deref()
							.map_or_else(String::new, |name| format!(" ({name})"));
						format!("{}{accounts}", instruction.name)
					},
				);
				let _ = writeln!(
					output,
					"Instruction #{index} failed: {} in {}",
					escape_controls(&instruction),
					escape_controls(&self.program.name)
				);
			}
			_ => {
				let _ = writeln!(output, "The transaction failed outside its instructions.");
			}
		}

		let error = &failure.error;
		let code = match (error.kind, error.code) {
			(ErrorKind::Pina, Some(code)) => format!(" (custom error {code:#X})"),
			(_, Some(code)) => format!(" (custom error {code})"),
			_ => String::new(),
		};
		let _ = writeln!(output, "Error: {}{code}", escape_controls(&error.name));
		if let Some(description) = &error.description {
			let _ = writeln!(output, "  {}", escape_controls(description));
		}

		if let Some(cpi) = &failure.cpi {
			let _ = writeln!(
				output,
				"Failed inside a CPI to {} (depth {}): {}",
				escape_controls(&cpi.program_id),
				cpi.depth,
				escape_controls(&cpi.message)
			);
		}
	}

	fn render_candidates(&self, output: &mut String) {
		let Some((first, rest)) = self.candidates.split_first() else {
			return;
		};

		if first.confidence == Confidence::Possible && !first.log_confirmed {
			let _ = writeln!(
				output,
				"\nPossible causes (they need runtime values to check):"
			);
			for candidate in &self.candidates {
				render_candidate(output, candidate);
			}
			return;
		}

		let _ = writeln!(output, "\nMost likely cause:");
		render_candidate(output, first);

		if !rest.is_empty() {
			let _ = writeln!(output, "\nOther candidates:");
			for candidate in rest {
				render_candidate(output, candidate);
			}
		}
	}

	fn render_error_sites(&self, output: &mut String) {
		if self.error_sites.is_empty() {
			return;
		}

		let in_instruction = self
			.error_sites
			.iter()
			.any(|site| site.in_failing_instruction);
		let heading = if in_instruction {
			"Where this instruction returns the error:"
		} else {
			"Where the program returns the error:"
		};
		let _ = writeln!(output, "\n{heading}");

		for site in self
			.error_sites
			.iter()
			.filter(|site| site.in_failing_instruction == in_instruction)
			.take(ERROR_SITE_LINES)
		{
			let _ = writeln!(
				output,
				"  {} in {}",
				escape_controls(&site.location),
				escape_controls(&site.context)
			);
		}
	}

	fn render_accounts(&self, output: &mut String) {
		if self.accounts.is_empty() {
			return;
		}

		let width = self
			.accounts
			.iter()
			.map(|row| row.field.as_deref().map_or(1, str::len) + if row.absent { 9 } else { 0 })
			.max()
			.unwrap_or_default()
			.max("field".len());

		let _ = writeln!(output, "\nAccounts:");
		let _ = writeln!(
			output,
			"  #   {:width$}  signer  writable  address",
			"field"
		);
		for row in &self.accounts {
			let field = row.field.as_deref().unwrap_or("-");
			let field = if row.absent {
				format!("{field} (absent)")
			} else {
				field.to_owned()
			};
			let _ = writeln!(
				output,
				"  {:<3} {:width$}  {:<6}  {:<8}  {}",
				row.index,
				escape_controls(&field),
				yes_no(row.signer),
				yes_no(row.writable),
				escape_controls(&row.address)
			);
		}
	}
}

fn render_candidate(output: &mut String, candidate: &Candidate) {
	let subject = candidate.field.as_deref().unwrap_or("instruction");
	let confidence = match candidate.confidence {
		Confidence::Confirmed => "confirmed",
		Confidence::CheckedAgainstCurrentState => "checked against current state",
		Confidence::Possible => "possible",
	};
	let log = if candidate.log_confirmed {
		", matches the program log"
	} else {
		""
	};
	let location = candidate
		.location
		.as_deref()
		.map_or_else(String::new, |location| format!(" at {location}"));

	let _ = writeln!(
		output,
		"  {}: {} [{confidence}{log}]{}",
		escape_controls(subject),
		escape_controls(&candidate.rule),
		escape_controls(&location)
	);
	let _ = writeln!(output, "    {}", escape_controls(&candidate.reason));
}

const fn yes_no(value: bool) -> &'static str {
	if value { "yes" } else { "no" }
}
