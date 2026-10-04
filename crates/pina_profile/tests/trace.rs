//! End-to-end trace analysis against recordings of `examples/counter_program`.
//!
//! `tests/fixtures/trace/` holds the program built with DWARF line tables
//! (`counter_program.so` stripped, as the tests loaded it, and
//! `counter_program.so.debug` unstripped) plus one `initialize` and one
//! `increment` recording from its Mollusk tests. Regenerate them with
//! `node scripts/regenerate-trace-fixtures.ts` after changing the program or
//! the toolchain.

use std::path::Path;
use std::path::PathBuf;

use pina_profile::trace::read_trace_dir;
use pina_profile::trace_output::write_folded;
use pina_profile::trace_output::write_trace_json;
use pina_profile::trace_output::write_trace_text;
use pina_profile::trace_report::ObservedDiscriminator;
use pina_profile::trace_report::TraceReport;
use pina_profile::trace_report::TracedProgram;
use pina_profile::trace_report::analyze_traces;

fn fixture(name: &str) -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("tests/fixtures/trace")
		.join(name)
}

fn program() -> TracedProgram {
	TracedProgram::load(
		&fixture("counter_program.so"),
		&fixture("counter_program.so.debug"),
	)
	.unwrap_or_else(|error| panic!("loading the traced fixture failed: {error}"))
}

/// The counter program's discriminators, as the CLI resolves them from its IR.
fn counter_instruction(discriminator: &ObservedDiscriminator) -> Option<String> {
	match (discriminator.width, discriminator.value) {
		(1, 0) => Some("initialize".to_owned()),
		(1, 1) => Some("increment".to_owned()),
		_ => None,
	}
}

fn report() -> TraceReport {
	let traces = read_trace_dir(&fixture("traces"))
		.unwrap_or_else(|error| panic!("reading fixture traces failed: {error}"));
	let mut report = analyze_traces(&traces, &program());
	report.assign_names(counter_instruction);
	report
}

fn render(
	write: impl Fn(&TraceReport, &mut Vec<u8>) -> Result<(), pina_profile::output::OutputError>,
) -> String {
	let mut buffer = Vec::new();
	write(&report(), &mut buffer).unwrap_or_else(|error| panic!("render failed: {error}"));
	String::from_utf8(buffer).unwrap_or_else(|error| panic!("output is not UTF-8: {error}"))
}

#[test]
fn counter_traces_attribute_every_instruction() {
	let report = report();
	let names: Vec<&str> = report
		.instructions
		.iter()
		.map(|profile| profile.name.as_str())
		.collect();

	assert_eq!(names, ["increment", "initialize"]);
	assert_eq!(report.recorded_traces, 2);
	assert_eq!(report.skipped_traces, 0);
	assert!(report.line_info);

	for profile in &report.instructions {
		let line_total: u64 = profile
			.lines
			.iter()
			.map(|line| line.executed_instructions)
			.sum();
		let stack_total: u64 = profile
			.stacks
			.iter()
			.map(|stack| stack.executed_instructions)
			.sum();

		assert_eq!(
			line_total + profile.unattributed_instructions,
			profile.executed_instructions
		);
		assert_eq!(stack_total, profile.executed_instructions);
		assert!(
			profile
				.stacks
				.iter()
				.all(|stack| stack.frames.first().map(String::as_str) == Some("entrypoint")),
			"every stack starts at the entrypoint: {:?}",
			profile.stacks
		);
	}
}

#[test]
fn counter_initialize_names_its_syscalls() {
	let report = report();
	let initialize = report
		.instructions
		.iter()
		.find(|profile| profile.name == "initialize")
		.unwrap_or_else(|| panic!("initialize profile missing"));
	let syscalls: Vec<&str> = initialize
		.syscalls
		.iter()
		.map(|syscall| syscall.name.as_str())
		.collect();

	assert!(syscalls.contains(&"sol_invoke_signed_c"), "{syscalls:?}");
	assert!(
		syscalls.iter().all(|name| !name.starts_with("syscall 0x")),
		"every syscall key resolves to a name: {syscalls:?}"
	);
}

#[test]
fn counter_report_text_snapshot() {
	insta::assert_snapshot!(render(|report, buffer| write_trace_text(report, buffer)));
}

#[test]
fn counter_report_json_snapshot() {
	// Keep the snapshot reviewable: the shape and the hottest entries of each
	// list pin the contract; the invariants above cover the rest.
	insta::assert_snapshot!(render(|report, buffer| {
		let mut trimmed = report.clone();
		for profile in &mut trimmed.instructions {
			profile.lines.truncate(3);
			profile.functions.truncate(3);
			profile.stacks.truncate(3);
		}
		write_trace_json(&trimmed, buffer)
	}));
}

#[test]
fn counter_report_folded_snapshot() {
	insta::assert_snapshot!(render(|report, buffer| write_folded(report, buffer)));
}

#[test]
fn recordings_of_other_builds_are_skipped() {
	let mut traces = read_trace_dir(&fixture("traces"))
		.unwrap_or_else(|error| panic!("reading fixture traces failed: {error}"));
	traces[0].executable_sha256 = Some("0".repeat(64));
	traces[1].executable_sha256 = None;

	let report = analyze_traces(&traces, &program());

	assert_eq!(report.recorded_traces, 2);
	assert_eq!(report.skipped_traces, 2);
	assert!(report.instructions.is_empty());
}

#[test]
fn identical_recordings_are_reported_once() {
	let traces = read_trace_dir(&fixture("traces"))
		.unwrap_or_else(|error| panic!("reading fixture traces failed: {error}"));
	let mut repeated = traces[0].clone();
	repeated.id = "ffffffffffffffff".to_owned();
	let all = [traces.clone(), vec![repeated]].concat();

	let report = analyze_traces(&all, &program());
	let repeated_profile = report
		.instructions
		.iter()
		.find(|profile| profile.trace_ids.len() == 2)
		.unwrap_or_else(|| panic!("the repeated recording was not grouped"));

	assert_eq!(report.instructions.len(), 2);
	assert_eq!(
		repeated_profile.trace_ids,
		[traces[0].id.clone(), "ffffffffffffffff".to_owned()]
	);
}

#[test]
fn release_comparison_reads_both_builds() {
	let identical = pina_profile::trace_report::ReleaseComparison::between(
		&fixture("counter_program.so"),
		&fixture("counter_program.so.debug"),
	)
	.unwrap_or_else(|error| panic!("comparison failed: {error}"));

	assert!(identical.identical_text);
	assert_eq!(identical.differing_instructions, 0);
	assert!(
		pina_profile::trace_report::ReleaseComparison::between(
			&fixture("counter_program.so"),
			&fixture("missing.so"),
		)
		.is_err()
	);
}

#[test]
fn mismatched_debug_build_is_rejected() {
	let other = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
	let not_elf = other.path().join("not-elf.so.debug");
	std::fs::write(&not_elf, b"not an elf").unwrap_or_else(|error| panic!("write: {error}"));
	let error = TracedProgram::load(&fixture("counter_program.so"), &not_elf)
		.expect_err("a non-ELF debug build must fail");
	assert!(error.to_string().contains("Failed to parse ELF"), "{error}");

	let mut altered = std::fs::read(fixture("counter_program.so.debug"))
		.unwrap_or_else(|error| panic!("read debug fixture failed: {error}"));
	// The first `.text` instruction starts at file offset 0x120.
	altered[0x120] ^= 0xff;
	let altered_path = other.path().join("altered.so.debug");
	std::fs::write(&altered_path, altered)
		.unwrap_or_else(|error| panic!("write altered build failed: {error}"));

	let mismatch = TracedProgram::load(&fixture("counter_program.so"), &altered_path)
		.expect_err("a different .text must be rejected");
	assert!(
		mismatch.to_string().contains("their .text sections differ"),
		"{mismatch}"
	);

	let missing = TracedProgram::load(&fixture("missing.so"), &altered_path)
		.expect_err("a missing executable must fail");
	assert!(matches!(missing, pina_profile::ProfileError::Io { .. }));
}
