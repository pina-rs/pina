use std::fs;
use std::process::Command;

use ed25519_dalek::SigningKey;
use solana_address::Address;
use tempfile::TempDir;

fn project(program_id: &str) -> TempDir {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
	fs::create_dir_all(temp.path().join("src"))
		.unwrap_or_else(|error| panic!("create source failed: {error}"));
	fs::write(
		temp.path().join("Cargo.toml"),
		"[package]\nname = \"cli-demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
	)
	.unwrap_or_else(|error| panic!("manifest write failed: {error}"));
	fs::write(
		temp.path().join("src/lib.rs"),
		format!("use pina::*;\n\ndeclare_id!(\"{program_id}\");\n"),
	)
	.unwrap_or_else(|error| panic!("source write failed: {error}"));
	temp
}

fn write_keypair(temp: &TempDir, secret: [u8; 32]) -> (std::path::PathBuf, String) {
	let path = temp.path().join("custom-keypair.json");
	let public_key = SigningKey::from_bytes(&secret).verifying_key().to_bytes();
	let mut bytes = secret.to_vec();
	bytes.extend(public_key);
	fs::write(
		&path,
		serde_json::to_vec(&bytes).unwrap_or_else(|error| panic!("serialization failed: {error}")),
	)
	.unwrap_or_else(|error| panic!("keypair write failed: {error}"));
	(path, Address::from(public_key).to_string())
}

#[test]
fn keys_json_is_stable_and_machine_readable() {
	let temp = project("11111111111111111111111111111111");
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(temp.path())
		.args(["keys", "--json"])
		.output()
		.unwrap_or_else(|error| panic!("keys failed to launch: {error}"));

	assert!(
		output.status.success(),
		"keys failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout)
		.unwrap_or_else(|error| panic!("invalid keys JSON: {error}"));
	assert_eq!(value["project"]["packageName"], "cli-demo");
	assert_eq!(
		value["declaredProgramId"],
		"11111111111111111111111111111111"
	);
	assert!(value["keypairProgramId"].is_null());
	assert!(value["matches"].is_null());
}

#[test]
fn keys_sync_is_explicit_and_preserves_unrelated_source() {
	let temp = project("11111111111111111111111111111111");
	let (keypair, expected) = write_keypair(&temp, [4u8; 32]);
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(temp.path())
		.args([
			"keys",
			"sync",
			"--keypair",
			keypair.to_str().expect("UTF-8 keypair path"),
			"--json",
		])
		.output()
		.unwrap_or_else(|error| panic!("keys sync failed to launch: {error}"));

	assert!(
		output.status.success(),
		"keys sync failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let value: serde_json::Value = serde_json::from_slice(&output.stdout)
		.unwrap_or_else(|error| panic!("invalid sync JSON: {error}"));
	let source = fs::read_to_string(temp.path().join("src/lib.rs"))
		.unwrap_or_else(|error| panic!("source read failed: {error}"));
	assert_eq!(value["programId"], expected);
	assert_eq!(value["changed"], true);
	assert!(source.starts_with("use pina::*;\n\n"));
	assert!(source.contains(&format!("declare_id!(\"{expected}\")")));
}

#[cfg(not(windows))]
#[test]
fn keys_human_diagnostics_escape_control_characters_in_paths() {
	let temp = project("11111111111111111111111111111111");
	let keypair = temp.path().join("keypair\n\u{1b}[31m.json");
	let public_key = SigningKey::from_bytes(&[9u8; 32])
		.verifying_key()
		.to_bytes();
	let mut bytes = [9u8; 32].to_vec();
	bytes.extend(public_key);
	fs::write(
		&keypair,
		serde_json::to_vec(&bytes).unwrap_or_else(|error| panic!("serialization failed: {error}")),
	)
	.unwrap_or_else(|error| panic!("keypair write failed: {error}"));

	let success = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(temp.path())
		.args(["keys", "show", "--keypair"])
		.arg(&keypair)
		.output()
		.unwrap_or_else(|error| panic!("keys show failed to launch: {error}"));
	assert!(success.status.success());
	let stdout = String::from_utf8_lossy(&success.stdout);
	assert!(!stdout.contains('\u{1b}'));
	assert!(stdout.contains("\\n"));
	assert!(stdout.contains("\\u{1b}"));

	let missing = temp.path().join("missing\n\u{1b}[31m.json");
	let failure = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(temp.path())
		.args(["keys", "sync", "--keypair"])
		.arg(&missing)
		.output()
		.unwrap_or_else(|error| panic!("keys failure failed to launch: {error}"));
	assert!(!failure.status.success());
	let stderr = String::from_utf8_lossy(&failure.stderr);
	assert!(!stderr.contains("missing\n\u{1b}[31m.json"));
	assert!(stderr.contains("\\n"));
	assert!(stderr.contains("\\u{1b}"));
}

#[cfg(not(windows))]
#[test]
fn keys_human_output_covers_show_sync_and_generation_states() {
	let temp = project("11111111111111111111111111111111");
	let root = fs::canonicalize(temp.path())
		.unwrap_or_else(|error| panic!("canonicalize failed: {error}"));
	let keypair = root.join("human-keypair.json");
	let missing = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(&root)
		.arg("keys")
		.output()
		.unwrap_or_else(|error| panic!("keys show failed to launch: {error}"));
	assert!(missing.status.success());
	assert!(String::from_utf8_lossy(&missing.stdout).contains("keypair not found"));

	let generated = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(&root)
		.args([
			"keys",
			"new",
			"--keypair",
			keypair.to_str().expect("UTF-8 keypair path"),
		])
		.output()
		.unwrap_or_else(|error| panic!("keys new failed to launch: {error}"));
	assert!(generated.status.success());
	assert!(String::from_utf8_lossy(&generated.stdout).contains("Created"));

	let matching = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(&root)
		.args([
			"keys",
			"show",
			"--keypair",
			keypair.to_str().expect("UTF-8 keypair path"),
		])
		.output()
		.unwrap_or_else(|error| panic!("keys show failed to launch: {error}"));
	assert!(String::from_utf8_lossy(&matching.stdout).contains("source and keypair match"));

	fs::write(
		root.join("src/lib.rs"),
		"declare_id!(\"11111111111111111111111111111111\");\n",
	)
	.unwrap_or_else(|error| panic!("source reset failed: {error}"));
	let mismatch = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(&root)
		.args([
			"keys",
			"show",
			"--keypair",
			keypair.to_str().expect("UTF-8 keypair path"),
		])
		.output()
		.unwrap_or_else(|error| panic!("keys mismatch failed to launch: {error}"));
	assert!(String::from_utf8_lossy(&mismatch.stdout).contains("mismatch"));

	let changed = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(&root)
		.args([
			"keys",
			"sync",
			"--keypair",
			keypair.to_str().expect("UTF-8 keypair path"),
		])
		.output()
		.unwrap_or_else(|error| panic!("keys sync failed to launch: {error}"));
	assert!(String::from_utf8_lossy(&changed.stdout).contains("Previous program ID"));

	let unchanged = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(&root)
		.args([
			"keys",
			"sync",
			"--keypair",
			keypair.to_str().expect("UTF-8 keypair path"),
		])
		.output()
		.unwrap_or_else(|error| panic!("keys sync failed to launch: {error}"));
	assert!(String::from_utf8_lossy(&unchanged.stdout).contains("already matches"));
}

#[test]
fn doctor_json_reports_a_versioned_schema() {
	let temp = project("11111111111111111111111111111111");
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(temp.path())
		.args(["doctor", "--json"])
		.output()
		.unwrap_or_else(|error| panic!("doctor failed to launch: {error}"));

	let value: serde_json::Value = serde_json::from_slice(&output.stdout)
		.unwrap_or_else(|error| panic!("invalid doctor JSON: {error}"));
	assert_eq!(value["schemaVersion"], 1);
	assert_eq!(value["project"]["packageName"], "cli-demo");
	assert!(matches!(
		value["status"].as_str(),
		Some("ok" | "warning" | "error")
	));
	assert!(value["tools"].is_array());
	assert!(value["checks"].is_array());
	assert!(value["findings"].is_array());
}

#[test]
fn doctor_json_remains_available_on_failure() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(temp.path())
		.args(["doctor", "--json"])
		.output()
		.unwrap_or_else(|error| panic!("doctor failed to launch: {error}"));

	assert!(!output.status.success());
	let value: serde_json::Value = serde_json::from_slice(&output.stdout)
		.unwrap_or_else(|error| panic!("invalid doctor failure JSON: {error}"));
	assert_eq!(value["status"], "error");
	assert!(value["project"].is_null());
	assert!(
		!value["findings"]
			.as_array()
			.expect("findings array")
			.is_empty()
	);
}

#[test]
fn doctor_human_output_is_available_for_projects_and_discovery_failures() {
	let project = project("11111111111111111111111111111111");
	let project_output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(project.path())
		.arg("doctor")
		.output()
		.unwrap_or_else(|error| panic!("project doctor failed to launch: {error}"));
	let project_stdout = String::from_utf8_lossy(&project_output.stdout);
	assert!(project_stdout.contains("Pina doctor"));
	assert!(project_stdout.contains("Project:"));

	let missing = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
	let missing_output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(missing.path())
		.arg("doctor")
		.output()
		.unwrap_or_else(|error| panic!("missing doctor failed to launch: {error}"));
	assert!(!missing_output.status.success());
	assert!(String::from_utf8_lossy(&missing_output.stdout).contains("Project: unavailable"));
}

#[cfg(not(windows))]
#[test]
fn keys_new_creates_a_valid_identity_and_refuses_replacement() {
	let temp = project("11111111111111111111111111111111");
	let keypair = fs::canonicalize(temp.path())
		.unwrap_or_else(|error| panic!("canonicalize failed: {error}"))
		.join("generated-keypair.json");
	let first = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(temp.path())
		.args([
			"keys",
			"new",
			"--keypair",
			keypair.to_str().expect("UTF-8 keypair path"),
			"--json",
		])
		.output()
		.unwrap_or_else(|error| panic!("keys new failed to launch: {error}"));
	assert!(
		first.status.success(),
		"keys new failed: {}",
		String::from_utf8_lossy(&first.stderr)
	);
	let generated: serde_json::Value = serde_json::from_slice(&first.stdout)
		.unwrap_or_else(|error| panic!("invalid generation JSON: {error}"));
	let inspection = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(temp.path())
		.args([
			"keys",
			"show",
			"--keypair",
			keypair.to_str().expect("UTF-8 keypair path"),
			"--json",
		])
		.output()
		.unwrap_or_else(|error| panic!("keys show failed to launch: {error}"));
	let inspected: serde_json::Value = serde_json::from_slice(&inspection.stdout)
		.unwrap_or_else(|error| panic!("invalid inspection JSON: {error}"));
	assert_eq!(generated["programId"], inspected["declaredProgramId"]);
	assert_eq!(inspected["matches"], true);

	let second = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(temp.path())
		.args([
			"keys",
			"new",
			"--keypair",
			keypair.to_str().expect("UTF-8 keypair path"),
		])
		.output()
		.unwrap_or_else(|error| panic!("second keys new failed to launch: {error}"));
	assert!(!second.status.success());
	assert!(String::from_utf8_lossy(&second.stderr).contains("pass --force"));

	let rotated = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(temp.path())
		.args([
			"keys",
			"new",
			"--force",
			"--keypair",
			keypair.to_str().expect("UTF-8 keypair path"),
			"--json",
		])
		.output()
		.unwrap_or_else(|error| panic!("forced keys new failed to launch: {error}"));
	assert!(rotated.status.success());
	let rotated: serde_json::Value = serde_json::from_slice(&rotated.stdout)
		.unwrap_or_else(|error| panic!("invalid rotation JSON: {error}"));
	assert_ne!(generated["programId"], rotated["programId"]);

	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;

		let mode = fs::metadata(&keypair)
			.unwrap_or_else(|error| panic!("keypair metadata failed: {error}"))
			.permissions()
			.mode() & 0o777;
		assert_eq!(mode, 0o600);
	}
}

#[cfg(not(windows))]
#[test]
fn doctor_distinguishes_a_corrupt_keypair_from_a_missing_one() {
	let temp = project("11111111111111111111111111111111");
	let generated = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(temp.path())
		.args(["keys", "new", "--json"])
		.output()
		.unwrap_or_else(|error| panic!("keys new failed to launch: {error}"));
	assert!(generated.status.success());
	let generated: serde_json::Value = serde_json::from_slice(&generated.stdout)
		.unwrap_or_else(|error| panic!("invalid generation JSON: {error}"));
	let keypair = std::path::PathBuf::from(
		generated["keypair"]
			.as_str()
			.expect("generated keypair path"),
	);
	let mut corrupt = vec![1u8; 32];
	corrupt.extend([2u8; 32]);
	fs::write(
		&keypair,
		serde_json::to_vec(&corrupt)
			.unwrap_or_else(|error| panic!("serialization failed: {error}")),
	)
	.unwrap_or_else(|error| panic!("keypair write failed: {error}"));

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(temp.path())
		.args(["doctor", "--json"])
		.output()
		.unwrap_or_else(|error| panic!("doctor failed to launch: {error}"));

	assert!(!output.status.success());
	let value: serde_json::Value = serde_json::from_slice(&output.stdout)
		.unwrap_or_else(|error| panic!("invalid doctor JSON: {error}"));
	let checks = value["checks"].as_array().expect("checks array");
	assert!(
		checks
			.iter()
			.any(|check| { check["id"] == "project.program-id" && check["status"] == "pass" })
	);
	assert!(
		checks
			.iter()
			.any(|check| { check["id"] == "project.keypair" && check["status"] == "fail" })
	);
}

#[cfg(not(windows))]
#[test]
fn doctor_checks_keypair_validity_independently_from_source() {
	let temp = project("11111111111111111111111111111111");
	let generated = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(temp.path())
		.args(["keys", "new", "--json"])
		.output()
		.unwrap_or_else(|error| panic!("keys new failed to launch: {error}"));
	assert!(generated.status.success());
	fs::write(temp.path().join("src/lib.rs"), "pub fn malformed(\n")
		.unwrap_or_else(|error| panic!("source write failed: {error}"));

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(temp.path())
		.args(["doctor", "--json"])
		.output()
		.unwrap_or_else(|error| panic!("doctor failed to launch: {error}"));

	assert!(!output.status.success());
	let value: serde_json::Value = serde_json::from_slice(&output.stdout)
		.unwrap_or_else(|error| panic!("invalid doctor JSON: {error}"));
	let checks = value["checks"].as_array().expect("checks array");
	assert!(
		checks
			.iter()
			.any(|check| { check["id"] == "project.program-id" && check["status"] == "fail" })
	);
	assert!(
		checks
			.iter()
			.any(|check| { check["id"] == "project.keypair" && check["status"] == "pass" })
	);
}

#[cfg(windows)]
#[test]
fn keys_new_fails_closed_before_creating_a_windows_secret() {
	let temp = project("11111111111111111111111111111111");
	let keypair = temp.path().join("generated-keypair.json");
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(temp.path())
		.args([
			"keys",
			"new",
			"--keypair",
			keypair.to_str().expect("UTF-8 keypair path"),
		])
		.output()
		.unwrap_or_else(|error| panic!("keys new failed to launch: {error}"));

	assert!(!output.status.success());
	assert!(
		String::from_utf8_lossy(&output.stderr)
			.contains("private file permissions are unsupported")
	);
	assert!(!keypair.exists());
}

#[test]
fn doctor_json_reports_missing_required_tools_as_failures() {
	let temp = project("11111111111111111111111111111111");
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(temp.path())
		.env("PATH", "")
		.args(["doctor", "--json"])
		.output()
		.unwrap_or_else(|error| panic!("doctor failed to launch: {error}"));

	assert!(!output.status.success());
	let value: serde_json::Value = serde_json::from_slice(&output.stdout)
		.unwrap_or_else(|error| panic!("invalid doctor JSON: {error}"));
	assert_eq!(value["status"], "error");
	let checks = value["checks"].as_array().expect("checks array");
	assert!(
		checks
			.iter()
			.any(|check| { check["id"] == "tool.cargo" && check["status"] == "fail" })
	);
	assert!(
		checks
			.iter()
			.any(|check| { check["id"] == "rust.unstable-flags" && check["status"] == "fail" })
	);
}
