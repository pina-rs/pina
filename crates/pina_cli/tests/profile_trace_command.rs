//! Integration tests for `pina profile trace`.
//!
//! The analysis tests run against `examples/counter_program` with
//! `--trace-dir`, using the traced build and recordings checked in under
//! `crates/pina_profile/tests/fixtures/trace/`, so instruction names come from
//! the real program's IR. The workflow tests replace `cargo` with a script
//! that publishes those fixtures, which exercises the build and test wiring
//! without the SBF toolchain.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;

use tempfile::TempDir;

fn workspace_root() -> &'static Path {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.and_then(Path::parent)
		.unwrap_or_else(|| Path::new("."))
}

fn fixtures() -> PathBuf {
	workspace_root().join("crates/pina_profile/tests/fixtures/trace")
}

fn counter_program() -> PathBuf {
	workspace_root().join("examples/counter_program")
}

fn copy(from: &Path, to: &Path) {
	fs::create_dir_all(to.parent().unwrap_or(to))
		.unwrap_or_else(|error| panic!("create {}: {error}", to.display()));
	fs::copy(from, to)
		.unwrap_or_else(|error| panic!("copy {} to {}: {error}", from.display(), to.display()));
}

/// What the comparison release build looks like.
#[derive(Clone, Copy)]
enum Release {
	Missing,
	Identical,
	Different,
	Unreadable,
}

/// A Cargo target directory holding a previous run's traced build.
fn traced_target(release: Release) -> TempDir {
	let target = TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
	let trace = target.path().join("pina/trace");
	copy(
		&fixtures().join("counter_program.so"),
		&trace.join("build/counter_program.so"),
	);
	copy(
		&fixtures().join("counter_program.so.debug"),
		&trace.join("counter_program.so.debug"),
	);

	match release {
		Release::Missing => {}
		Release::Identical => {
			copy(
				&fixtures().join("counter_program.so"),
				&trace.join("release/counter_program.so"),
			);
		}
		Release::Unreadable => {
			let release_program = trace.join("release/counter_program.so");
			fs::create_dir_all(release_program.parent().unwrap_or(&trace))
				.unwrap_or_else(|error| panic!("create release dir: {error}"));
			fs::write(release_program, b"not an elf")
				.unwrap_or_else(|error| panic!("write release: {error}"));
		}
		Release::Different => {
			let mut bytes = fs::read(fixtures().join("counter_program.so"))
				.unwrap_or_else(|error| panic!("read fixture: {error}"));
			// The first `.text` instruction starts at file offset 0x120.
			bytes[0x120] ^= 0xff;
			let release_program = trace.join("release/counter_program.so");
			fs::create_dir_all(release_program.parent().unwrap_or(&trace))
				.unwrap_or_else(|error| panic!("create release dir: {error}"));
			fs::write(release_program, bytes)
				.unwrap_or_else(|error| panic!("write release: {error}"));
		}
	}

	target
}

fn trace_command(target: &Path, trace_dir: &Path) -> Command {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command
		.args(["profile", "trace", "--project"])
		.arg(counter_program())
		.arg("--trace-dir")
		.arg(trace_dir)
		.env("CARGO_TARGET_DIR", target)
		.env("NO_COLOR", "1");
	command
}

fn run(command: &mut Command) -> Output {
	command
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina: {error}"))
}

fn text(bytes: &[u8]) -> String {
	String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn text_summary_names_instructions_and_writes_the_html_report() {
	let target = traced_target(Release::Identical);
	let output = run(&mut trace_command(
		target.path(),
		&fixtures().join("traces"),
	));
	let stdout = text(&output.stdout);
	let stderr = text(&output.stderr);

	assert!(output.status.success(), "stderr: {stderr}");
	assert!(!stderr.contains("Warning"), "stderr: {stderr}");

	let report = target
		.path()
		.join("pina")
		.join("trace")
		.join("counter_program.html");
	let html = fs::read_to_string(&report).unwrap_or_else(|error| panic!("read report: {error}"));
	assert!(html.contains("\"name\":\"increment\""));
	assert!(!html.contains("/*PINA_TRACE_DATA*/"));
	// The program's own source resolves and is embedded for the annotated view.
	assert!(html.contains("\"examples/counter_program/src/lib.rs\":{\"displayPath\""));

	// The CLI prints the native path, so match it exactly and show it in one
	// portable form; nothing else in the summary depends on the platform.
	let native_report = report.display().to_string();
	insta::assert_snapshot!(
		stdout.replace(&native_report, "[TARGET]/pina/trace/counter_program.html")
	);
}

#[test]
fn json_output_is_versioned_and_skips_the_html_report() {
	let target = traced_target(Release::Missing);
	let output = run(trace_command(target.path(), &fixtures().join("traces")).arg("--json"));

	assert!(output.status.success(), "stderr: {}", text(&output.stderr));
	let json: serde_json::Value = serde_json::from_slice(&output.stdout)
		.unwrap_or_else(|error| panic!("invalid JSON: {error}"));
	let names: Vec<&str> = json["instructions"]
		.as_array()
		.unwrap_or_else(|| panic!("instructions missing: {json}"))
		.iter()
		.filter_map(|profile| profile["name"].as_str())
		.collect();

	assert_eq!(json["schemaVersion"], 1);
	assert_eq!(json["releaseBuild"], serde_json::Value::Null);
	assert_eq!(names, ["increment", "initialize"]);
	assert!(
		!target
			.path()
			.join("pina/trace/counter_program.html")
			.exists()
	);
}

#[test]
fn folded_output_roots_stacks_at_each_instruction() {
	let target = traced_target(Release::Missing);
	let output = run(trace_command(target.path(), &fixtures().join("traces")).arg("--folded"));
	let stdout = text(&output.stdout);

	assert!(output.status.success(), "stderr: {}", text(&output.stderr));
	assert!(stdout.lines().count() > 10, "{stdout}");
	assert!(
		stdout.lines().all(|line| {
			(line.starts_with("increment;entrypoint") || line.starts_with("initialize;entrypoint"))
				&& line
					.rsplit_once(' ')
					.is_some_and(|(_, count)| count.parse::<u64>().is_ok())
		}),
		"{stdout}"
	);
}

#[test]
fn instruction_filter_accepts_any_case_and_rejects_unknown_names() {
	let target = traced_target(Release::Missing);
	let filtered = run(
		trace_command(target.path(), &fixtures().join("traces")).args([
			"--json",
			"--instruction",
			"Increment",
		]),
	);
	let json: serde_json::Value = serde_json::from_slice(&filtered.stdout)
		.unwrap_or_else(|error| panic!("invalid JSON: {error}"));

	assert_eq!(json["instructions"].as_array().map(Vec::len), Some(1));
	assert_eq!(json["instructions"][0]["instruction"], "increment");

	let unknown = run(trace_command(target.path(), &fixtures().join("traces"))
		.args(["--instruction", "transfer"]));

	assert_eq!(unknown.status.code(), Some(1));
	assert!(
		text(&unknown.stderr)
			.contains("no traced instruction is named \"transfer\"; traced: increment, initialize"),
		"{}",
		text(&unknown.stderr)
	);
}

#[test]
fn changed_code_generation_is_reported() {
	let target = traced_target(Release::Different);
	let output = run(trace_command(target.path(), &fixtures().join("traces")).arg("--json"));
	let json: serde_json::Value = serde_json::from_slice(&output.stdout)
		.unwrap_or_else(|error| panic!("invalid JSON: {error}"));

	assert!(output.status.success(), "stderr: {}", text(&output.stderr));
	assert_eq!(json["releaseBuild"]["identicalText"], false);
	assert_eq!(json["releaseBuild"]["differingInstructions"], 1);
	assert!(
		text(&output.stderr).contains(
			"debug information changed code generation: 1 of 1334 instruction slots differ"
		),
		"{}",
		text(&output.stderr)
	);
}

#[test]
fn unreadable_release_build_fails_the_comparison() {
	let target = traced_target(Release::Unreadable);
	let output = run(&mut trace_command(
		target.path(),
		&fixtures().join("traces"),
	));

	assert_eq!(output.status.code(), Some(1));
	assert!(
		text(&output.stderr).contains("Failed to parse ELF"),
		"{}",
		text(&output.stderr)
	);
}

#[test]
fn output_file_receives_the_summary() {
	let target = traced_target(Release::Missing);
	let summary = target.path().join("summary.txt");
	let output = run(trace_command(target.path(), &fixtures().join("traces"))
		.arg("--output")
		.arg(&summary));

	assert!(output.status.success(), "stderr: {}", text(&output.stderr));
	assert!(output.stdout.is_empty());
	assert!(text(&output.stderr).contains("HTML report: "));
	let written =
		fs::read_to_string(&summary).unwrap_or_else(|error| panic!("read summary: {error}"));
	assert!(
		written.starts_with("counter_program: 2 traced paths"),
		"{written}"
	);
}

#[test]
fn trace_dir_requires_a_previous_traced_build() {
	let target = TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
	let output = run(&mut trace_command(
		target.path(),
		&fixtures().join("traces"),
	));

	assert_eq!(output.status.code(), Some(1));
	assert!(
		text(&output.stderr).contains("no traced build at"),
		"{}",
		text(&output.stderr)
	);
}

#[test]
fn empty_trace_dir_explains_how_to_enable_tracing() {
	let target = traced_target(Release::Missing);
	let empty = target.path().join("empty");
	fs::create_dir_all(&empty).unwrap_or_else(|error| panic!("create empty: {error}"));
	let output = run(&mut trace_command(target.path(), &empty));
	let stderr = text(&output.stderr);

	assert_eq!(output.status.code(), Some(1));
	assert!(
		stderr.contains("no SBF register traces were recorded"),
		"{stderr}"
	);
	assert!(
		stderr.contains("mollusk-svm = { workspace = true, features = [\"register-tracing\"] }"),
		"{stderr}"
	);
}

#[test]
fn traces_from_another_build_are_rejected() {
	let target = traced_target(Release::Missing);
	let traces = target.path().join("other-build");

	for entry in fs::read_dir(fixtures().join("traces"))
		.unwrap_or_else(|error| panic!("read fixtures: {error}"))
	{
		let path = entry
			.unwrap_or_else(|error| panic!("fixture entry: {error}"))
			.path();
		copy(&path, &traces.join(path.file_name().unwrap_or_default()));
	}
	for entry in fs::read_dir(&traces).unwrap_or_else(|error| panic!("read traces: {error}")) {
		let path = entry
			.unwrap_or_else(|error| panic!("entry: {error}"))
			.path();
		if path.to_string_lossy().ends_with(".exec.sha256") {
			fs::write(&path, "0".repeat(64)).unwrap_or_else(|error| panic!("rewrite: {error}"));
		}
	}

	let output = run(&mut trace_command(target.path(), &traces));
	let stderr = text(&output.stderr);

	assert_eq!(output.status.code(), Some(1));
	assert!(
		stderr.contains("2 recorded traces in") && stderr.contains("none executed this build"),
		"{stderr}"
	);
}

#[test]
fn filter_conflicts_with_an_existing_trace_dir() {
	let target = traced_target(Release::Missing);
	let output =
		run(trace_command(target.path(), &fixtures().join("traces"))
			.args(["--filter", "increment"]));

	assert_eq!(output.status.code(), Some(2));
}

/// A scaffolded `counter_program` project whose `cargo` publishes the
/// fixture build and recordings.
#[cfg(unix)]
struct FakeWorkflow {
	_dir: TempDir,
	project: PathBuf,
	target: PathBuf,
	log: PathBuf,
}

#[cfg(unix)]
fn fake_workflow() -> FakeWorkflow {
	use std::os::unix::fs::PermissionsExt;

	let dir = TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
	// Cargo reports canonical paths, so the fixture uses them too.
	let root = dir
		.path()
		.canonicalize()
		.unwrap_or_else(|error| panic!("canonicalize: {error}"));
	let project = root.join("counter_program");
	pina_cli::init_project(&project, "counter_program", false)
		.unwrap_or_else(|error| panic!("scaffold failed: {error}"));
	let manifest = project.join("Cargo.toml");
	let cargo_toml =
		fs::read_to_string(&manifest).unwrap_or_else(|error| panic!("read manifest: {error}"));
	fs::write(&manifest, format!("{cargo_toml}\n[workspace]\n"))
		.unwrap_or_else(|error| panic!("isolate manifest: {error}"));

	let script = project.join("fake-cargo.sh");
	fs::write(
		&script,
		r#"#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
	metadata)
		# Removing the script makes the next cargo invocation fail to start.
		if [[ "${PINA_FAKE_MODE:-}" == "vanish-before-build" ]]; then rm -f "$0"; fi
		exec cargo "$@"
		;;
	build-sbf)
		out=""
		previous=""
		for argument in "$@"; do
			if [[ "$previous" == "--sbf-out-dir" ]]; then out="$argument"; fi
			previous="$argument"
		done
		printf 'build-sbf debug=%s strip=%s out=%s\n' "${CARGO_PROFILE_RELEASE_DEBUG:-}" \
			"${CARGO_PROFILE_RELEASE_STRIP:-}" "$out" >> "$PINA_FAKE_LOG"
		if [[ "${PINA_FAKE_MODE:-}" == "build-fails" ]]; then exit 9; fi
		if [[ "${PINA_FAKE_MODE:-}" == "no-artifact" ]]; then exit 0; fi
		mkdir -p "$out"
		cp "$PINA_FAKE_FIXTURES/counter_program.so" "$out/counter_program.so"
		if [[ -n "${CARGO_PROFILE_RELEASE_DEBUG:-}" && "${PINA_FAKE_MODE:-}" != "no-linker-output" ]]; then
			mkdir -p "$CARGO_TARGET_DIR/sbpf-solana-solana/release"
			cp "$PINA_FAKE_FIXTURES/counter_program.so.debug" \
				"$CARGO_TARGET_DIR/sbpf-solana-solana/release/counter_program.so"
		elif [[ "${PINA_FAKE_MODE:-}" == "vanish-before-tests" ]]; then
			rm -f "$0"
		fi
		;;
	test)
		printf 'test sbf_out=%s trace=%s bpf=%s args=%s\n' "$SBF_OUT_DIR" "$SBF_TRACE_DIR" \
			"${BPF_OUT_DIR:-unset}" "$*" >> "$PINA_FAKE_LOG"
		test -f "$SBF_OUT_DIR/counter_program.so"
		case "${PINA_FAKE_MODE:-}" in
			tests-fail) exit 7 ;;
			no-traces) ;;
			*) cp "$PINA_FAKE_FIXTURES/traces/"* "$SBF_TRACE_DIR/" ;;
		esac
		;;
	*) exit 91 ;;
esac
"#,
	)
	.unwrap_or_else(|error| panic!("write fake cargo: {error}"));
	let mut permissions = fs::metadata(&script)
		.unwrap_or_else(|error| panic!("stat fake cargo: {error}"))
		.permissions();
	permissions.set_mode(0o755);
	fs::set_permissions(&script, permissions)
		.unwrap_or_else(|error| panic!("chmod fake cargo: {error}"));

	FakeWorkflow {
		target: root.join("target"),
		log: root.join("commands.log"),
		project,
		_dir: dir,
	}
}

#[cfg(unix)]
fn run_workflow(workflow: &FakeWorkflow, mode: &str, args: &[&str]) -> Output {
	run(Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["profile", "trace", "--project"])
		.arg(&workflow.project)
		.args(args)
		.env("CARGO", workflow.project.join("fake-cargo.sh"))
		.env("CARGO_TARGET_DIR", &workflow.target)
		.env("BPF_OUT_DIR", "/must/not/be/searched")
		.env("NO_COLOR", "1")
		.env("PINA_FAKE_FIXTURES", fixtures())
		.env("PINA_FAKE_LOG", &workflow.log)
		.env("PINA_FAKE_MODE", mode))
}

#[cfg(unix)]
#[test]
fn workflow_builds_twice_runs_traced_tests_and_reports() {
	let workflow = fake_workflow();
	let output = run_workflow(&workflow, "", &["--filter", "increment", "--json"]);
	let log = fs::read_to_string(&workflow.log).unwrap_or_else(|error| panic!("read log: {error}"));
	let trace = workflow.target.join("pina/trace");
	let lines: Vec<&str> = log.lines().collect();

	assert!(output.status.success(), "stderr: {}", text(&output.stderr));
	assert_eq!(
		lines,
		[
			format!(
				"build-sbf debug=line-tables-only strip=none out={}",
				trace.join("build").display()
			),
			format!(
				"build-sbf debug= strip= out={}",
				trace.join("release").display()
			),
			format!(
				"test sbf_out={} trace={} bpf=unset args=test --manifest-path {} increment",
				trace.join("build").display(),
				trace.join("traces").display(),
				workflow.project.join("Cargo.toml").display()
			),
		]
	);
	assert!(trace.join("counter_program.so.debug").is_file());

	let json: serde_json::Value = serde_json::from_slice(&output.stdout)
		.unwrap_or_else(|error| panic!("invalid JSON: {error}"));
	assert_eq!(json["releaseBuild"]["identicalText"], true);
	assert_eq!(json["recordedTraces"], 2);
}

#[cfg(unix)]
#[test]
fn workflow_propagates_test_failures() {
	let workflow = fake_workflow();
	let output = run_workflow(&workflow, "tests-fail", &[]);
	let stderr = text(&output.stderr);

	assert_eq!(output.status.code(), Some(7), "stderr: {stderr}");
	assert!(
		stderr.contains("`cargo test` exited unsuccessfully"),
		"{stderr}"
	);
	assert!(
		stderr.contains("pina profile trace --trace-dir"),
		"{stderr}"
	);
}

#[cfg(unix)]
#[test]
fn workflow_reports_tests_that_record_nothing() {
	let workflow = fake_workflow();
	let output = run_workflow(&workflow, "no-traces", &[]);
	let stderr = text(&output.stderr);

	assert_eq!(output.status.code(), Some(1));
	assert!(
		stderr.contains("mollusk-svm = { version = \"0.15\", features = [\"register-tracing\"] }"),
		"{stderr}"
	);
}

#[cfg(unix)]
#[test]
fn workflow_reports_build_problems() {
	let failing = fake_workflow();
	let output = run_workflow(&failing, "build-fails", &[]);
	assert_eq!(output.status.code(), Some(1));
	assert!(
		text(&output.stderr).contains("failed (exit status: 9)"),
		"{}",
		text(&output.stderr)
	);

	let missing = fake_workflow();
	let output = run_workflow(&missing, "no-linker-output", &[]);
	assert_eq!(output.status.code(), Some(1));
	assert!(
		text(&output.stderr).contains("cargo build-sbf left no unstripped program"),
		"{}",
		text(&output.stderr)
	);
}

#[cfg(unix)]
#[test]
fn workflow_reports_missing_artifacts_and_executables() {
	let cases = [
		("no-artifact", "was not created"),
		("vanish-before-build", "Failed to run `"),
		("vanish-before-tests", "failed to run "),
	];

	for (mode, expected) in cases {
		let workflow = fake_workflow();
		let output = run_workflow(&workflow, mode, &[]);
		let stderr = text(&output.stderr);

		assert_eq!(output.status.code(), Some(1), "{mode}: {stderr}");
		assert!(stderr.contains(expected), "{mode}: {stderr}");
	}
}

#[cfg(unix)]
#[test]
fn workflow_replaces_stale_traces_and_reports_an_unusable_trace_dir() {
	let stale = fake_workflow();
	let traces = stale.target.join("pina/trace/traces");
	fs::create_dir_all(&traces).unwrap_or_else(|error| panic!("create traces: {error}"));
	// A stale, unreadable recording would fail the analysis if it survived.
	fs::write(traces.join("ffffffffffffffff.regs"), b"stale")
		.unwrap_or_else(|error| panic!("write stale trace: {error}"));

	let output = run_workflow(&stale, "", &["--json"]);
	assert!(output.status.success(), "stderr: {}", text(&output.stderr));
	assert!(!traces.join("ffffffffffffffff.regs").exists());

	let blocked = fake_workflow();
	let traces = blocked.target.join("pina/trace/traces");
	fs::create_dir_all(traces.parent().unwrap_or(&blocked.target))
		.unwrap_or_else(|error| panic!("create trace root: {error}"));
	fs::write(&traces, b"not a directory").unwrap_or_else(|error| panic!("write: {error}"));

	let output = run_workflow(&blocked, "", &[]);
	assert_eq!(output.status.code(), Some(1));
	assert!(
		text(&output.stderr).contains("failed to prepare"),
		"{}",
		text(&output.stderr)
	);
}

#[cfg(unix)]
#[test]
fn workflow_labels_traces_by_id_when_the_program_cannot_be_parsed() {
	let workflow = fake_workflow();
	fs::write(workflow.project.join("src/lib.rs"), "fn {")
		.unwrap_or_else(|error| panic!("break source: {error}"));

	let output = run_workflow(&workflow, "", &["--json"]);
	let json: serde_json::Value = serde_json::from_slice(&output.stdout)
		.unwrap_or_else(|error| panic!("invalid JSON: {error}"));

	assert!(output.status.success(), "stderr: {}", text(&output.stderr));
	assert!(
		text(&output.stderr).contains("instruction names are unavailable"),
		"{}",
		text(&output.stderr)
	);
	assert!(
		json["instructions"]
			.as_array()
			.unwrap_or_else(|| panic!("instructions missing: {json}"))
			.iter()
			.all(|profile| {
				profile["name"]
					.as_str()
					.is_some_and(|name| name.starts_with("trace "))
			}),
		"{json}"
	);
}
