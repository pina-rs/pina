//! Integration tests for the `pina profile` CLI command.
//!
//! Builds minimal synthetic SBF ELF binaries and exercises the CLI end-to-end.

use std::io::Write;
use std::path::Path;
use std::process::Command;

use object::Architecture;
use object::BinaryFormat;
use object::Endianness;
use object::SectionKind;
use object::SymbolFlags;
use object::SymbolKind;
use object::SymbolScope;
use object::write::Object;
use object::write::Symbol;
use object::write::SymbolSection;

/// Build a minimal SBF ELF binary with a `.text` section and optional symbols.
fn build_sbf_elf(text_size: usize, symbols: &[(&str, u64, u64)]) -> Vec<u8> {
	let mut obj = Object::new(BinaryFormat::Elf, Architecture::Sbf, Endianness::Little);

	let section = obj.add_section(Vec::new(), b".text".to_vec(), SectionKind::Text);

	let text_data = vec![0u8; text_size];
	obj.set_section_data(section, text_data, 8);

	for &(name, offset, size) in symbols {
		obj.add_symbol(Symbol {
			name: name.as_bytes().to_vec(),
			value: offset,
			size,
			kind: SymbolKind::Text,
			scope: SymbolScope::Dynamic,
			weak: false,
			section: SymbolSection::Section(section),
			flags: SymbolFlags::None,
		});
	}

	obj.write()
		.unwrap_or_else(|e| panic!("failed to write ELF: {e}"))
}

/// Write bytes to a temp file and return it.
fn write_temp_elf(data: &[u8]) -> tempfile::NamedTempFile {
	let mut file = tempfile::Builder::new()
		.suffix(".so")
		.tempfile()
		.unwrap_or_else(|e| panic!("failed to create temp file: {e}"));
	file.write_all(data)
		.unwrap_or_else(|e| panic!("failed to write temp ELF: {e}"));
	file.flush()
		.unwrap_or_else(|e| panic!("failed to flush temp ELF: {e}"));
	file
}

#[test]
fn cli_profile_text_output() {
	let elf_data = build_sbf_elf(160, &[("process_instruction", 0, 160)]);
	let file = write_temp_elf(&elf_data);

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["profile", file.path().to_str().unwrap()])
		.output()
		.unwrap_or_else(|e| panic!("failed to run pina profile: {e}"));

	assert!(
		output.status.success(),
		"pina profile failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let stdout = String::from_utf8_lossy(&output.stdout);
	assert!(
		stdout.contains("process_instruction"),
		"expected function name in output: {stdout}"
	);
	assert!(
		stdout.contains("Total estimated CU"),
		"expected CU summary in output: {stdout}"
	);
}

#[test]
fn cli_profile_json_output() {
	let elf_data = build_sbf_elf(160, &[("process_instruction", 0, 160)]);
	let file = write_temp_elf(&elf_data);

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["profile", file.path().to_str().unwrap(), "--json"])
		.output()
		.unwrap_or_else(|e| panic!("failed to run pina profile --json: {e}"));

	assert!(
		output.status.success(),
		"pina profile --json failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let stdout = String::from_utf8_lossy(&output.stdout);
	let parsed: serde_json::Value =
		serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("invalid JSON: {e}"));
	assert_eq!(parsed["total_instructions"], 20);
	assert!(parsed["functions"].is_array());
}

#[test]
fn cli_profile_output_to_file() {
	let elf_data = build_sbf_elf(80, &[("my_func", 0, 80)]);
	let elf_file = write_temp_elf(&elf_data);
	let output_file = tempfile::Builder::new()
		.suffix(".json")
		.tempfile()
		.unwrap_or_else(|e| panic!("temp: {e}"));
	let output_path = std::fs::canonicalize(output_file.path())
		.unwrap_or_else(|error| panic!("canonicalize output failed: {error}"));

	let result = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"profile",
			elf_file.path().to_str().unwrap(),
			"--json",
			"--output",
			output_path.to_str().unwrap(),
		])
		.output()
		.unwrap_or_else(|e| panic!("failed to run pina profile --output: {e}"));

	assert!(
		result.status.success(),
		"pina profile --output failed: {}",
		String::from_utf8_lossy(&result.stderr)
	);

	let content =
		std::fs::read_to_string(&output_path).unwrap_or_else(|e| panic!("read output file: {e}"));
	let parsed: serde_json::Value = serde_json::from_str(&content)
		.unwrap_or_else(|e| panic!("invalid JSON in output file: {e}"));
	assert!(parsed["functions"].is_array());
}

#[test]
fn cli_profile_nonexistent_file_fails() {
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["profile", "/nonexistent/path.so"])
		.output()
		.unwrap_or_else(|e| panic!("failed to run: {e}"));

	assert!(!output.status.success());
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(stderr.contains("Error"), "expected error message: {stderr}");
}

#[test]
fn cli_profile_non_elf_fails() {
	let mut file = tempfile::Builder::new()
		.suffix(".so")
		.tempfile()
		.unwrap_or_else(|e| panic!("temp: {e}"));
	file.write_all(b"not an elf").unwrap();
	file.flush().unwrap();

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["profile", file.path().to_str().unwrap()])
		.output()
		.unwrap_or_else(|e| panic!("failed to run: {e}"));

	assert!(!output.status.success());
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(stderr.contains("Error"), "expected error message: {stderr}");
}

#[test]
fn cli_profile_refuses_overwrite_input() {
	let elf_data = build_sbf_elf(80, &[]);
	let file = write_temp_elf(&elf_data);
	let canonical_path = std::fs::canonicalize(file.path())
		.unwrap_or_else(|error| panic!("canonicalize input failed: {error}"));
	let path = canonical_path.as_path();
	let path_str = path.to_str().unwrap();
	let output_path = path
		.parent()
		.expect("temporary file has a parent")
		.join(".")
		.join(path.file_name().expect("temporary file has a name"));
	let before = std::fs::read(path).unwrap_or_else(|error| panic!("input read failed: {error}"));

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"profile",
			path_str,
			"--output",
			output_path.to_str().expect("UTF-8 output path"),
		])
		.output()
		.unwrap_or_else(|e| panic!("failed to run: {e}"));

	assert!(!output.status.success());
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(
		stderr.contains("path resolves to the input program"),
		"expected overwrite guard: {stderr}"
	);
	assert_eq!(
		std::fs::read(path).unwrap_or_else(|error| panic!("input read failed: {error}")),
		before
	);
}

#[test]
fn cli_profile_refuses_hardlink_output_without_changing_input() {
	let elf_data = build_sbf_elf(80, &[]);
	let file = write_temp_elf(&elf_data);
	let input_path = std::fs::canonicalize(file.path())
		.unwrap_or_else(|error| panic!("canonicalize input failed: {error}"));
	let output_path = input_path.with_extension("hardlink.so");
	std::fs::hard_link(&input_path, &output_path)
		.unwrap_or_else(|error| panic!("hardlink failed: {error}"));

	let result = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"profile",
			input_path.to_str().expect("UTF-8 input path"),
			"--output",
			output_path.to_str().expect("UTF-8 output path"),
		])
		.output()
		.unwrap_or_else(|error| panic!("profile failed to launch: {error}"));

	assert!(!result.status.success());
	assert!(String::from_utf8_lossy(&result.stderr).contains("path resolves to the input program"));
	assert_eq!(
		std::fs::read(file.path()).unwrap_or_else(|error| panic!("input read failed: {error}")),
		elf_data
	);
}

#[cfg(unix)]
#[test]
fn cli_profile_refuses_symlinked_output_ancestor() {
	use std::os::unix::fs::symlink;

	let elf_data = build_sbf_elf(80, &[]);
	let file = write_temp_elf(&elf_data);
	let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
	let root = std::fs::canonicalize(temp.path())
		.unwrap_or_else(|error| panic!("canonicalize temp failed: {error}"));
	let target = root.join("target");
	let link = root.join("linked");
	std::fs::create_dir(&target).unwrap_or_else(|error| panic!("target create failed: {error}"));
	symlink(&target, &link).unwrap_or_else(|error| panic!("symlink failed: {error}"));
	let output_path = link.join("report.json");

	let result = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"profile",
			file.path().to_str().expect("UTF-8 input path"),
			"--output",
			output_path.to_str().expect("UTF-8 output path"),
		])
		.output()
		.unwrap_or_else(|error| panic!("profile failed to launch: {error}"));

	assert!(!result.status.success());
	assert!(
		String::from_utf8_lossy(&result.stderr)
			.contains("path contains a symbolic link or reparse point")
	);
	assert!(!target.join("report.json").exists());
}

#[test]
fn cli_profile_discovers_current_project_artifact() {
	let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
	let source_dir = temp.path().join("src");
	let artifact_dir = temp.path().join("target/deploy");
	std::fs::create_dir_all(&source_dir)
		.unwrap_or_else(|error| panic!("create source failed: {error}"));
	std::fs::create_dir_all(&artifact_dir)
		.unwrap_or_else(|error| panic!("create artifact failed: {error}"));
	std::fs::write(
		temp.path().join("Cargo.toml"),
		"[package]\nname = \"profile-demo\"\nversion = \"0.1.0\"\nedition = \
		 \"2024\"\n\n[lib]\ncrate-type = [\"cdylib\", \"lib\"]\n",
	)
	.unwrap_or_else(|error| panic!("manifest write failed: {error}"));
	std::fs::write(source_dir.join("lib.rs"), "pub fn placeholder() {}\n")
		.unwrap_or_else(|error| panic!("source write failed: {error}"));
	std::fs::write(
		artifact_dir.join("profile_demo.so"),
		build_sbf_elf(160, &[("process_instruction", 0, 160)]),
	)
	.unwrap_or_else(|error| panic!("artifact write failed: {error}"));

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(temp.path())
		.args(["profile", "--json"])
		.output()
		.unwrap_or_else(|error| panic!("failed to run discovered profile: {error}"));

	assert!(
		output.status.success(),
		"profile failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)
		.unwrap_or_else(|error| panic!("invalid profile JSON: {error}"));
	assert_eq!(parsed["total_instructions"], 20);
}

#[test]
fn cli_profile_reports_the_expected_missing_artifact() {
	let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
	std::fs::create_dir_all(temp.path().join("src"))
		.unwrap_or_else(|error| panic!("create source failed: {error}"));
	std::fs::write(
		temp.path().join("Cargo.toml"),
		"[package]\nname = \"missing-demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
	)
	.unwrap_or_else(|error| panic!("manifest write failed: {error}"));
	std::fs::write(temp.path().join("src/lib.rs"), "pub fn placeholder() {}\n")
		.unwrap_or_else(|error| panic!("source write failed: {error}"));

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(temp.path())
		.arg("profile")
		.output()
		.unwrap_or_else(|error| panic!("profile failed to launch: {error}"));
	let stderr = String::from_utf8_lossy(&output.stderr);
	let relative_artifact = Path::new("target").join("deploy").join("missing_demo.so");
	let quoted_artifact = format!("{relative_artifact:?}");
	let escaped_artifact = quoted_artifact
		.chars()
		.flat_map(char::escape_default)
		.collect::<String>();
	let expected_suffix = escaped_artifact
		.strip_prefix("\\\"")
		.unwrap_or_else(|| panic!("quoted artifact lacks an opening quote: {escaped_artifact}"));

	assert!(!output.status.success());
	assert!(stderr.contains(expected_suffix), "{stderr}");
	assert!(
		stderr.contains("build the program before profiling"),
		"{stderr}"
	);
}

#[test]
fn cli_profile_fails_closed_when_cargo_metadata_is_invalid() {
	let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
	let artifact_dir = temp.path().join("target/deploy");
	std::fs::create_dir_all(temp.path().join("src"))
		.unwrap_or_else(|error| panic!("source create failed: {error}"));
	std::fs::create_dir_all(&artifact_dir)
		.unwrap_or_else(|error| panic!("artifact create failed: {error}"));
	std::fs::write(
		temp.path().join("Cargo.toml"),
		"[package]\nname = \"fallback-demo\"\nthis is not valid TOML\n",
	)
	.unwrap_or_else(|error| panic!("manifest write failed: {error}"));
	std::fs::write(temp.path().join("src/lib.rs"), "pub fn placeholder() {}\n")
		.unwrap_or_else(|error| panic!("source write failed: {error}"));
	std::fs::write(
		artifact_dir.join("fallback_demo.so"),
		build_sbf_elf(80, &[("fallback", 0, 80)]),
	)
	.unwrap_or_else(|error| panic!("artifact write failed: {error}"));

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(temp.path())
		.args(["profile", "--json"])
		.output()
		.unwrap_or_else(|error| panic!("profile failed to launch: {error}"));

	assert!(!output.status.success());
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(
		stderr.contains("Cargo metadata discovery failed"),
		"{stderr}"
	);
}

/// Profile an ELF through the CLI and save the JSON report as a baseline file.
fn write_baseline(elf_path: &Path) -> tempfile::NamedTempFile {
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["profile", elf_path.to_str().unwrap(), "--json"])
		.output()
		.unwrap_or_else(|error| panic!("failed to profile baseline: {error}"));

	assert!(
		output.status.success(),
		"baseline profiling failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let file = tempfile::Builder::new()
		.suffix(".json")
		.tempfile()
		.unwrap_or_else(|error| panic!("temp failed: {error}"));
	std::fs::write(file.path(), &output.stdout)
		.unwrap_or_else(|error| panic!("baseline write failed: {error}"));
	file
}

fn run_compare(baseline: &Path, artifact: &Path, extra_args: &[&str]) -> std::process::Output {
	Command::new(env!("CARGO_BIN_EXE_pina"))
		.arg("profile")
		.arg("compare")
		.arg(baseline)
		.arg(artifact)
		.args(extra_args)
		.output()
		.unwrap_or_else(|error| panic!("compare failed to launch: {error}"))
}

#[test]
fn cli_profile_compare_identical_reports_exit_zero() {
	let elf_data = build_sbf_elf(160, &[("entry", 0, 160)]);
	let artifact = write_temp_elf(&elf_data);
	let baseline = write_baseline(artifact.path());

	let output = run_compare(baseline.path(), artifact.path(), &[]);

	assert!(
		output.status.success(),
		"identical comparison failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let stdout = String::from_utf8_lossy(&output.stdout);
	assert!(stdout.contains("No function-level changes"), "{stdout}");
	assert!(stdout.contains("No compute unit changes"), "{stdout}");
	assert!(stdout.contains("20 -> 20"), "{stdout}");
}

#[test]
fn cli_profile_compare_flags_regressions_beyond_the_threshold() {
	let baseline_elf = build_sbf_elf(160, &[("entry", 0, 160)]);
	let baseline_artifact = write_temp_elf(&baseline_elf);
	let baseline = write_baseline(baseline_artifact.path());
	let current_elf = build_sbf_elf(800, &[("entry", 0, 160), ("extra", 160, 640)]);
	let current_artifact = write_temp_elf(&current_elf);

	let output = run_compare(
		baseline.path(),
		current_artifact.path(),
		&["--fail-cu", "50", "--fail-percent", "10"],
	);

	assert_eq!(
		output.status.code(),
		Some(2),
		"regression beyond the threshold must exit 2: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let stdout = String::from_utf8_lossy(&output.stdout);
	assert!(stdout.contains("reaches the threshold"), "{stdout}");
	assert!(stdout.contains("+80 CU"), "{stdout}");
	// Only changed functions are listed; `entry` is unchanged.
	assert!(stdout.contains("extra"), "{stdout}");
	assert!(!stdout.contains("  entry"), "{stdout}");
}

#[test]
fn cli_profile_compare_small_regressions_below_the_threshold_exit_zero() {
	let baseline_elf = build_sbf_elf(800, &[("entry", 0, 160), ("extra", 160, 640)]);
	let baseline_artifact = write_temp_elf(&baseline_elf);
	let baseline = write_baseline(baseline_artifact.path());
	let current_elf = build_sbf_elf(880, &[("entry", 0, 160), ("extra", 160, 720)]);
	let current_artifact = write_temp_elf(&current_elf);

	let output = run_compare(baseline.path(), current_artifact.path(), &[]);

	assert!(
		output.status.success(),
		"small regressions must not fail: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let stdout = String::from_utf8_lossy(&output.stdout);
	assert!(stdout.contains("below the threshold"), "{stdout}");
	assert!(stdout.contains("+10 CU"), "{stdout}");
}

#[test]
fn cli_profile_compare_improvements_exit_zero_under_strict_thresholds() {
	let baseline_elf = build_sbf_elf(800, &[("entry", 0, 160), ("extra", 160, 640)]);
	let baseline_artifact = write_temp_elf(&baseline_elf);
	let baseline = write_baseline(baseline_artifact.path());
	let current_elf = build_sbf_elf(160, &[("entry", 0, 160)]);
	let current_artifact = write_temp_elf(&current_elf);

	let output = run_compare(
		baseline.path(),
		current_artifact.path(),
		&["--fail-cu", "5", "--fail-percent", "1"],
	);

	assert!(
		output.status.success(),
		"improvements must not fail: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let stdout = String::from_utf8_lossy(&output.stdout);
	assert!(stdout.contains("improved by 80"), "{stdout}");
	assert!(stdout.contains("gone"), "{stdout}");
}

#[test]
fn cli_profile_compare_reports_added_and_removed_functions() {
	let baseline_elf = build_sbf_elf(160, &[("first", 0, 160)]);
	let baseline_artifact = write_temp_elf(&baseline_elf);
	let baseline = write_baseline(baseline_artifact.path());
	let current_elf = build_sbf_elf(160, &[("second", 0, 160)]);
	let current_artifact = write_temp_elf(&current_elf);

	let output = run_compare(baseline.path(), current_artifact.path(), &[]);

	assert!(
		output.status.success(),
		"replaced functions must not fail: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let stdout = String::from_utf8_lossy(&output.stdout);
	assert!(stdout.contains("first"), "{stdout}");
	assert!(stdout.contains("second"), "{stdout}");
	assert!(stdout.contains("No compute unit changes"), "{stdout}");
}

#[test]
fn cli_profile_compare_json_is_stable_machine_readable_output() {
	let baseline_elf = build_sbf_elf(160, &[("entry", 0, 160)]);
	let baseline_artifact = write_temp_elf(&baseline_elf);
	let baseline = write_baseline(baseline_artifact.path());
	let current_elf = build_sbf_elf(800, &[("entry", 0, 160), ("extra", 160, 640)]);
	let current_artifact = write_temp_elf(&current_elf);

	let args = [
		baseline.path().to_str().unwrap(),
		current_artifact.path().to_str().unwrap(),
		"--fail-cu",
		"50",
		"--fail-percent",
		"10",
	];
	let first = Command::new(env!("CARGO_BIN_EXE_pina"))
		.arg("profile")
		.arg("compare")
		.args(args)
		.arg("--json")
		.output()
		.unwrap_or_else(|error| panic!("compare failed to launch: {error}"));
	let second = Command::new(env!("CARGO_BIN_EXE_pina"))
		.arg("profile")
		.arg("compare")
		.args(args)
		.arg("--json")
		.output()
		.unwrap_or_else(|error| panic!("compare failed to launch: {error}"));

	assert_eq!(
		first.status.code(),
		Some(2),
		"threshold regression must exit 2 in JSON mode"
	);
	assert_eq!(first.stdout, second.stdout, "JSON output must be stable");

	let parsed: serde_json::Value = serde_json::from_slice(&first.stdout)
		.unwrap_or_else(|error| panic!("invalid comparison JSON: {error}"));
	assert_eq!(parsed["schema_version"], 1);
	assert_eq!(parsed["status"], "threshold-regression");
	assert_eq!(parsed["exceeds_threshold"], true);
	assert_eq!(parsed["threshold"]["delta_cu"], 50);
	assert_eq!(parsed["totals"]["baseline"]["total_cu"], 20);
	assert_eq!(parsed["totals"]["current"]["total_cu"], 100);
	assert_eq!(parsed["totals"]["delta_cu"], 80);
	assert_eq!(parsed["totals"]["delta_percent"], 400.0);

	let functions = parsed["functions"]
		.as_array()
		.unwrap_or_else(|| panic!("functions must be an array"));
	let names: Vec<&str> = functions
		.iter()
		.map(|function| function["name"].as_str().unwrap_or_default())
		.collect();
	assert_eq!(names, vec!["extra", "entry"]);
	assert_eq!(functions[0]["change"], "added");
	assert_eq!(functions[0]["baseline_cu"], serde_json::Value::Null);
	assert_eq!(functions[1]["change"], "unchanged");
}

#[test]
fn cli_profile_compare_accepts_versioned_baseline_documents() {
	let elf_data = build_sbf_elf(160, &[("entry", 0, 160)]);
	let artifact = write_temp_elf(&elf_data);
	let bare = write_baseline(artifact.path());
	let profile_json = std::fs::read_to_string(bare.path())
		.unwrap_or_else(|error| panic!("baseline read failed: {error}"));
	let versioned = format!("{{\"schema_version\": 1, \"profile\": {profile_json}}}");
	let versioned_file = tempfile::Builder::new()
		.suffix(".json")
		.tempfile()
		.unwrap_or_else(|error| panic!("temp failed: {error}"));
	std::fs::write(versioned_file.path(), versioned)
		.unwrap_or_else(|error| panic!("versioned write failed: {error}"));

	let output = run_compare(versioned_file.path(), artifact.path(), &[]);

	assert!(
		output.status.success(),
		"versioned baseline rejected: {}",
		String::from_utf8_lossy(&output.stderr)
	);
}

#[test]
fn cli_profile_compare_rejects_invalid_json_with_a_clear_error() {
	let baseline_file = tempfile::Builder::new()
		.suffix(".json")
		.tempfile()
		.unwrap_or_else(|error| panic!("temp failed: {error}"));
	std::fs::write(baseline_file.path(), "not json at all")
		.unwrap_or_else(|error| panic!("write failed: {error}"));
	let elf_data = build_sbf_elf(160, &[("entry", 0, 160)]);
	let artifact = write_temp_elf(&elf_data);

	let output = run_compare(baseline_file.path(), artifact.path(), &[]);

	assert_eq!(output.status.code(), Some(1));
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(stderr.contains("is not valid JSON"), "{stderr}");
	assert!(stderr.contains("baseline"), "{stderr}");
}

#[test]
fn cli_profile_compare_rejects_non_profile_documents() {
	let baseline_file = tempfile::Builder::new()
		.suffix(".json")
		.tempfile()
		.unwrap_or_else(|error| panic!("temp failed: {error}"));
	std::fs::write(baseline_file.path(), "{\"hello\": \"world\"}")
		.unwrap_or_else(|error| panic!("write failed: {error}"));
	let elf_data = build_sbf_elf(160, &[("entry", 0, 160)]);
	let artifact = write_temp_elf(&elf_data);

	let output = run_compare(baseline_file.path(), artifact.path(), &[]);

	assert_eq!(output.status.code(), Some(1));
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(stderr.contains("is not a pina profile report"), "{stderr}");
}

#[test]
fn cli_profile_compare_rejects_future_schema_versions() {
	let baseline_file = tempfile::Builder::new()
		.suffix(".json")
		.tempfile()
		.unwrap_or_else(|error| panic!("temp failed: {error}"));
	std::fs::write(
		baseline_file.path(),
		"{\"schema_version\": 999, \"profile\": {}}",
	)
	.unwrap_or_else(|error| panic!("write failed: {error}"));
	let elf_data = build_sbf_elf(160, &[("entry", 0, 160)]);
	let artifact = write_temp_elf(&elf_data);

	let output = run_compare(baseline_file.path(), artifact.path(), &[]);

	assert_eq!(output.status.code(), Some(1));
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(
		stderr.contains("unsupported schema version 999"),
		"{stderr}"
	);
}

#[test]
fn cli_profile_compare_fails_on_missing_baseline_file() {
	let elf_data = build_sbf_elf(160, &[("entry", 0, 160)]);
	let artifact = write_temp_elf(&elf_data);

	let output = run_compare(
		Path::new("/nonexistent/baseline.json"),
		artifact.path(),
		&[],
	);

	assert_eq!(output.status.code(), Some(1));
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(stderr.contains("failed to read baseline"), "{stderr}");
}

#[test]
fn cli_profile_compare_rejects_a_zero_fail_cu_flag() {
	let elf_data = build_sbf_elf(160, &[("entry", 0, 160)]);
	let artifact = write_temp_elf(&elf_data);
	let baseline = write_baseline(artifact.path());

	let output = run_compare(baseline.path(), artifact.path(), &["--fail-cu", "0"]);

	assert_eq!(output.status.code(), Some(1), "zero --fail-cu must exit 1");
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(stderr.contains("--fail-cu must be at least 1"), "{stderr}");
}

#[test]
fn cli_profile_compare_rejects_a_negative_fail_percent_flag() {
	let elf_data = build_sbf_elf(160, &[("entry", 0, 160)]);
	let artifact = write_temp_elf(&elf_data);
	let baseline = write_baseline(artifact.path());

	let output = run_compare(baseline.path(), artifact.path(), &["--fail-percent=-5"]);

	assert_eq!(
		output.status.code(),
		Some(1),
		"negative --fail-percent must exit 1"
	);
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(
		stderr.contains("--fail-percent must be a finite number of at least 0"),
		"{stderr}"
	);
}

#[test]
fn cli_profile_compare_reports_an_unreadable_artifact() {
	let baseline_elf = build_sbf_elf(160, &[("entry", 0, 160)]);
	let baseline = write_temp_elf(&baseline_elf);
	let mut bad = tempfile::Builder::new()
		.suffix(".so")
		.tempfile()
		.unwrap_or_else(|error| panic!("temp file failed: {error}"));
	bad.write_all(b"definitely not an SBF ELF")
		.unwrap_or_else(|error| panic!("write failed: {error}"));
	bad.as_file()
		.flush()
		.unwrap_or_else(|error| panic!("flush failed: {error}"));

	let output = run_compare(baseline.path(), bad.path(), &[]);

	assert_eq!(
		output.status.code(),
		Some(1),
		"unprofileable artifact must exit 1: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(stderr.contains("Error"), "{stderr}");
}
