//! Text, JSON, and folded-stack renderings of a [`TraceReport`].

use std::io::Write;

use crate::output::OutputError;
use crate::trace_report::InstructionProfile;
use crate::trace_report::TraceReport;

/// Source lines listed per instruction in the text summary.
const TOP_LINES: usize = 10;
/// Functions listed per instruction in the text summary.
const TOP_FUNCTIONS: usize = 8;
/// Width of the location and function columns in the text summary.
const NAME_COLUMN: usize = 52;
/// Cells in a share bar.
const BAR_CELLS: usize = 10;

/// Write the report as a stable, pretty-printed JSON document.
///
/// # Errors
///
/// Returns an error when serialization or writing fails.
pub fn write_trace_json(report: &TraceReport, writer: &mut dyn Write) -> Result<(), OutputError> {
	let json = serde_json::to_string_pretty(report).map_err(OutputError::Json)?;
	writeln!(writer, "{json}")?;
	Ok(())
}

/// Write Brendan Gregg folded stacks: one `frame;frame;frame count` line per
/// stack, rooted at the instruction's name.
///
/// The output loads in speedscope, inferno, and `flamegraph.pl`. Frame names
/// cannot contain the `;` separator, so a `;` inside a name (for example in
/// `[u8; 32]`) is written as `:`.
///
/// # Errors
///
/// Returns an error when writing fails.
pub fn write_folded(report: &TraceReport, writer: &mut dyn Write) -> Result<(), OutputError> {
	for profile in &report.instructions {
		for stack in &profile.stacks {
			let frames = std::iter::once(profile.name.as_str())
				.chain(stack.frames.iter().map(String::as_str))
				.map(folded_frame)
				.collect::<Vec<_>>()
				.join(";");
			writeln!(writer, "{frames} {}", stack.executed_instructions)?;
		}
	}

	Ok(())
}

fn folded_frame(name: &str) -> String {
	name.chars()
		.map(|character| {
			match character {
				';' => ':',
				character if character.is_control() => ' ',
				character => character,
			}
		})
		.collect()
}

/// Write a concise human-readable summary of every traced instruction.
///
/// # Errors
///
/// Returns an error when writing fails.
pub fn write_trace_text(report: &TraceReport, writer: &mut dyn Write) -> Result<(), OutputError> {
	writer.write_all(render_text(report).as_bytes())?;
	Ok(())
}

/// Render the summary [`write_trace_text`] writes.
#[must_use]
pub fn render_text(report: &TraceReport) -> String {
	let mut lines = vec![format!(
		"{}: {} traced {} from {} recorded {}",
		report.program,
		report.instructions.len(),
		plural(report.instructions.len(), "path", "paths"),
		report.recorded_traces,
		plural(report.recorded_traces, "invocation", "invocations"),
	)];

	if report.skipped_traces > 0 {
		lines.push(format!(
			"  {} {} executed other programs or builds and {} skipped.",
			report.skipped_traces,
			plural(report.skipped_traces, "recording", "recordings"),
			plural(report.skipped_traces, "was", "were"),
		));
	}

	lines.push(
		"  Each executed SBF instruction costs 1 CU; syscall charges are not included.".to_owned(),
	);

	if !report.line_info {
		lines.push(
			"  The traced build has no DWARF line table, so lines are unavailable.".to_owned(),
		);
	}

	for profile in &report.instructions {
		lines.push(String::new());
		render_instruction(profile, &mut lines);
	}

	lines.push(String::new());
	lines.join("\n")
}

fn render_instruction(profile: &InstructionProfile, lines: &mut Vec<String>) {
	let total = profile.executed_instructions;
	let runs = profile.trace_ids.len();
	let runs_note = if runs > 1 {
		format!(" across {runs} identical runs")
	} else {
		String::new()
	};

	lines.push(format!(
		"{}: {total} CU executed{runs_note}, {} {}",
		profile.name,
		profile.syscall_invocations,
		plural(profile.syscall_invocations as usize, "syscall", "syscalls"),
	));

	let mut rows: Vec<(String, u64)> = profile
		.lines
		.iter()
		.map(|line| (location(&line.file, line.line), line.executed_instructions))
		.collect();

	if profile.unattributed_instructions > 0 {
		rows.push((
			"<no line information>".to_owned(),
			profile.unattributed_instructions,
		));
		rows.sort_by_key(|(_, executed)| std::cmp::Reverse(*executed));
	}

	if !rows.is_empty() {
		lines.push(format!(
			"  {:<NAME_COLUMN$} {:>7} {:>6}",
			"Line", "CU", "Share"
		));
	}

	for (location, executed) in rows.iter().take(TOP_LINES) {
		let share = share(*executed, total);
		lines.push(format!(
			"  {:<NAME_COLUMN$} {executed:>7} {:>5.1}% {}",
			truncate_start(location, NAME_COLUMN),
			share * 100.0,
			share_bar(share),
		));
	}

	if rows.len() > TOP_LINES {
		lines.push(format!("  ... {} more lines", rows.len() - TOP_LINES));
	}

	if !profile.functions.is_empty() {
		lines.push(format!(
			"  {:<NAME_COLUMN$} {:>7} {:>7}",
			"Function", "Self", "Total"
		));
	}

	for function in profile.functions.iter().take(TOP_FUNCTIONS) {
		lines.push(format!(
			"  {:<NAME_COLUMN$} {:>7} {:>7}",
			truncate_start(&function.name, NAME_COLUMN),
			function.self_instructions,
			function.inclusive_instructions,
		));
	}

	if !profile.syscalls.is_empty() {
		lines.push(format!("  {:<28} {:>5}  Called from", "Syscall", "Calls"));
	}

	for syscall in &profile.syscalls {
		let mut sites: Vec<String> = profile
			.lines
			.iter()
			.filter(|line| line.syscalls.iter().any(|call| call.name == syscall.name))
			.map(|line| location(&line.file, line.line))
			.collect();
		sites.sort_unstable();
		let sites = if sites.is_empty() {
			"<no line information>".to_owned()
		} else {
			sites.join(", ")
		};

		lines.push(format!(
			"  {:<28} {:>5}  {sites}",
			syscall.name, syscall.invocations
		));
	}
}

fn location(file: &str, line: u32) -> String {
	format!("{}:{line}", display_path(file))
}

fn plural<'a>(count: usize, one: &'a str, many: &'a str) -> &'a str {
	if count == 1 { one } else { many }
}

fn share(part: u64, total: u64) -> f64 {
	if total == 0 {
		return 0.0;
	}

	part as f64 / total as f64
}

/// A bar of [`BAR_CELLS`] cells drawn with eighth blocks.
fn share_bar(share: f64) -> String {
	const PARTIAL: [char; 8] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉'];

	let eighths = (share.clamp(0.0, 1.0) * (BAR_CELLS * 8) as f64).round() as usize;
	let mut bar = "█".repeat(eighths / 8);

	if !eighths.is_multiple_of(8) {
		bar.push(PARTIAL[eighths % 8]);
	}

	bar
}

/// Keep the end of a long name, where Rust paths carry the specific part.
fn truncate_start(name: &str, width: usize) -> String {
	let length = name.chars().count();

	if length <= width {
		return name.to_owned();
	}

	let tail: String = name.chars().skip(length - (width - 2)).collect();
	format!("..{tail}")
}

/// Shorten toolchain and registry paths for display.
///
/// Standard library sources are recorded under the platform-tools build
/// machine's checkout (`.../rust/library/core/src/...`) and registry crates
/// under `~/.cargo/registry/src/<index>/<crate>-<version>/`; both are shown
/// from the crate directory onward. Other paths are unchanged.
#[must_use]
pub fn display_path(path: &str) -> &str {
	if let Some((_, library)) = path.split_once("/library/") {
		return library;
	}

	if let Some((_, registry)) = path.split_once("/registry/src/") {
		return registry.split_once('/').map_or(registry, |(_, rest)| rest);
	}

	path
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn display_path_shortens_toolchain_and_registry_paths() {
		assert_eq!(
			display_path("/Users/runner/work/platform-tools/out/rust/library/core/src/ptr/mod.rs"),
			"core/src/ptr/mod.rs"
		);
		assert_eq!(
			display_path(
				"/home/me/.cargo/registry/src/index.crates.io-abc/pinocchio-0.11.2/src/lib.rs"
			),
			"pinocchio-0.11.2/src/lib.rs"
		);
		assert_eq!(display_path("/home/me/.cargo/registry/src/odd"), "odd");
		assert_eq!(
			display_path("crates/pina/src/lib.rs"),
			"crates/pina/src/lib.rs"
		);
	}

	#[test]
	fn share_bar_uses_eighth_blocks() {
		assert_eq!(share_bar(0.0), "");
		assert_eq!(share_bar(1.0), "██████████");
		assert_eq!(share_bar(0.25), "██▌");
		assert_eq!(share(1, 0), 0.0);
	}

	#[test]
	fn truncate_start_keeps_the_tail() {
		assert_eq!(truncate_start("short", 10), "short");
		assert_eq!(truncate_start("pina::cpi::invoke_signed", 10), "..e_signed");
	}

	#[test]
	fn render_text_reports_skips_missing_lines_repeated_runs_and_overflow() {
		use crate::trace_report::FunctionCost;
		use crate::trace_report::LineCost;
		use crate::trace_report::SyscallCount;
		use crate::trace_report::TRACE_SCHEMA_VERSION;

		let lines = (1..=12)
			.map(|line| {
				LineCost {
					file: "src/lib.rs".to_owned(),
					line,
					executed_instructions: 13 - u64::from(line),
					syscalls: Vec::new(),
				}
			})
			.collect();
		let report = TraceReport {
			schema_version: TRACE_SCHEMA_VERSION,
			program: "demo".to_owned(),
			executable_sha256: String::new(),
			line_info: false,
			recorded_traces: 3,
			skipped_traces: 1,
			release_build: None,
			instructions: vec![InstructionProfile {
				name: "transfer".to_owned(),
				instruction: Some("transfer".to_owned()),
				discriminator: None,
				trace_ids: vec!["a".to_owned(), "b".to_owned()],
				executed_instructions: 80,
				syscall_invocations: 1,
				unattributed_instructions: 2,
				lines,
				functions: vec![FunctionCost {
					name: "entrypoint".to_owned(),
					self_instructions: 80,
					inclusive_instructions: 80,
				}],
				syscalls: vec![SyscallCount {
					name: "sol_log_".to_owned(),
					invocations: 1,
				}],
				stacks: Vec::new(),
			}],
		};

		let text = render_text(&report);

		assert!(
			text.contains("1 recording executed other programs or builds and was skipped."),
			"{text}"
		);
		assert!(text.contains("no DWARF line table"), "{text}");
		assert!(text.contains("transfer: 80 CU executed across 2 identical runs, 1 syscall"));
		assert!(text.contains("... 3 more lines"), "{text}");
		assert!(text.contains("sol_log_                         1  <no line information>"));
	}

	#[test]
	fn render_text_omits_empty_sections() {
		let report = TraceReport {
			schema_version: crate::trace_report::TRACE_SCHEMA_VERSION,
			program: "demo".to_owned(),
			executable_sha256: String::new(),
			line_info: true,
			recorded_traces: 1,
			skipped_traces: 0,
			release_build: None,
			instructions: vec![InstructionProfile {
				name: "noop".to_owned(),
				instruction: None,
				discriminator: None,
				trace_ids: vec!["a".to_owned()],
				executed_instructions: 0,
				syscall_invocations: 0,
				unattributed_instructions: 0,
				lines: Vec::new(),
				functions: Vec::new(),
				syscalls: Vec::new(),
				stacks: Vec::new(),
			}],
		};

		assert_eq!(
			render_text(&report),
			"demo: 1 traced path from 1 recorded invocation\n  Each executed SBF instruction \
			 costs 1 CU; syscall charges are not included.\n\nnoop: 0 CU executed, 0 syscalls\n"
		);
	}

	#[test]
	fn folded_frames_cannot_break_the_format() {
		assert_eq!(folded_frame("copy<[u8; 32]>"), "copy<[u8: 32]>");
		assert_eq!(folded_frame("a\nb"), "a b");
	}
}
