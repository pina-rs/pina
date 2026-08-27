#![cfg(unix)]

use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use ed25519_dalek::SigningKey;
use flate2::Compression;
use flate2::write::ZlibEncoder;
use serde_json::Value;

const PROGRAM_ID: &str = "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS";

fn workspace_root() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.and_then(Path::parent)
		.unwrap_or_else(|| Path::new("."))
		.to_path_buf()
}

fn fixture_idl() -> Value {
	let root = pina_cli::generate_idl(&workspace_root().join("examples/anchor_declare_id"), None)
		.unwrap_or_else(|error| panic!("fixture generation failed: {error}"));
	serde_json::to_value(root)
		.unwrap_or_else(|error| panic!("fixture serialization failed: {error}"))
}

fn write_idl(directory: &Path, value: &Value) -> PathBuf {
	let path = directory.join("idl.json");
	fs::write(
		&path,
		serde_json::to_vec_pretty(value)
			.unwrap_or_else(|error| panic!("fixture serialization failed: {error}")),
	)
	.unwrap_or_else(|error| panic!("fixture write failed: {error}"));
	path
}

fn raw_zlib(value: &Value) -> String {
	let source = serde_json::to_vec(value)
		.unwrap_or_else(|error| panic!("fixture serialization failed: {error}"));
	let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
	encoder
		.write_all(&source)
		.unwrap_or_else(|error| panic!("fixture compression failed: {error}"));
	let compressed = encoder
		.finish()
		.unwrap_or_else(|error| panic!("fixture compression finish failed: {error}"));

	compressed.iter().fold(String::new(), |mut output, byte| {
		write!(output, "{byte:02x}")
			.unwrap_or_else(|error| panic!("hex formatting failed: {error}"));
		output
	})
}

fn fake_npx(directory: &Path, stdout: &str, status: i32) -> (PathBuf, PathBuf) {
	let runner = directory.join("fake-npx.sh");
	let capture = directory.join("args.txt");
	let escaped_stdout = stdout.replace('\\', "\\\\").replace('\'', "'\\''");
	let script = format!(
		"#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nprintf '{}'\nexit {status}\n",
		capture.display(),
		escaped_stdout,
	);
	fs::write(&runner, script).unwrap_or_else(|error| panic!("fake runner write failed: {error}"));
	let mut permissions = fs::metadata(&runner)
		.unwrap_or_else(|error| panic!("fake runner metadata failed: {error}"))
		.permissions();
	permissions.set_mode(0o755);
	fs::set_permissions(&runner, permissions)
		.unwrap_or_else(|error| panic!("fake runner permissions failed: {error}"));

	(runner, capture)
}

fn keypair(directory: &Path) -> PathBuf {
	let path = directory.join("authority.json");
	let bytes = SigningKey::from_bytes(&[41; 32]).to_keypair_bytes();
	fs::write(
		&path,
		serde_json::to_vec(bytes.as_slice())
			.unwrap_or_else(|error| panic!("keypair serialization failed: {error}")),
	)
	.unwrap_or_else(|error| panic!("keypair write failed: {error}"));
	let mut permissions = fs::metadata(&path)
		.unwrap_or_else(|error| panic!("keypair metadata failed: {error}"))
		.permissions();
	permissions.set_mode(0o600);
	fs::set_permissions(&path, permissions)
		.unwrap_or_else(|error| panic!("keypair permissions failed: {error}"));
	path
}

#[test]
fn fetch_outputs_validated_json_and_uses_the_pinned_raw_client() {
	let directory =
		tempfile::tempdir().unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
	let value = fixture_idl();
	let (runner, capture) = fake_npx(
		directory.path(),
		&format!("Fetching metadata...\n{}", raw_zlib(&value)),
		0,
	);
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"idl",
			"fetch",
			"--cluster",
			"devnet",
			"--program-id",
			PROGRAM_ID,
			"--npx",
			runner.to_str().unwrap_or_default(),
			"--json",
		])
		.output()
		.unwrap_or_else(|error| panic!("fetch command failed to start: {error}"));

	assert!(
		output.status.success(),
		"{}",
		String::from_utf8_lossy(&output.stderr)
	);
	let envelope: Value = serde_json::from_slice(&output.stdout)
		.unwrap_or_else(|error| panic!("fetch JSON failed: {error}"));
	assert_eq!(envelope["idl"], value);
	let args =
		fs::read_to_string(capture).unwrap_or_else(|error| panic!("capture read failed: {error}"));
	assert!(args.contains("@solana-program/program-metadata@0.9.0"));
	assert!(args.contains("--raw"));
}

#[test]
fn official_client_receives_eof_instead_of_operator_input() {
	let directory =
		tempfile::tempdir().unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
	let value = fixture_idl();
	let raw = raw_zlib(&value)
		.replace('\\', "\\\\")
		.replace('\'', "'\\''");
	let runner = directory.path().join("stdin-probe.sh");
	let script = format!(
		"#!/bin/sh\nif IFS= read -r input; then\n  printf 'client inherited stdin: %s\\n' \
		 \"$input\" >&2\n  exit 91\nfi\nprintf '{raw}\\n'\n"
	);
	fs::write(&runner, script).unwrap_or_else(|error| panic!("stdin probe write failed: {error}"));
	let mut permissions = fs::metadata(&runner)
		.unwrap_or_else(|error| panic!("stdin probe metadata failed: {error}"))
		.permissions();
	permissions.set_mode(0o755);
	fs::set_permissions(&runner, permissions)
		.unwrap_or_else(|error| panic!("stdin probe permissions failed: {error}"));

	let mut child = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"idl",
			"fetch",
			"--cluster",
			"devnet",
			"--program-id",
			PROGRAM_ID,
			"--npx",
			runner.to_str().unwrap_or_default(),
		])
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.unwrap_or_else(|error| panic!("fetch command failed to start: {error}"));
	child
		.stdin
		.take()
		.expect("stdin pipe must exist")
		.write_all(b"operator input must stay private\n")
		.unwrap_or_else(|error| panic!("stdin write failed: {error}"));
	let output = child
		.wait_with_output()
		.unwrap_or_else(|error| panic!("fetch command wait failed: {error}"));

	assert!(
		output.status.success(),
		"{}",
		String::from_utf8_lossy(&output.stderr)
	);
	let fetched: Value = serde_json::from_slice(&output.stdout)
		.unwrap_or_else(|error| panic!("fetched IDL failed to parse: {error}"));
	assert_eq!(fetched, value);
	assert!(!String::from_utf8_lossy(&output.stderr).contains("operator input"));
}

#[test]
fn fetch_supports_plain_stdout_and_atomic_output() {
	let directory =
		tempfile::tempdir().unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
	let value = fixture_idl();
	let (runner, _) = fake_npx(directory.path(), &raw_zlib(&value), 0);
	let plain = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"idl",
			"fetch",
			"--cluster",
			"devnet",
			"--program-id",
			PROGRAM_ID,
			"--npx",
			runner.to_str().unwrap_or_default(),
		])
		.output()
		.unwrap_or_else(|error| panic!("plain fetch failed to start: {error}"));
	assert!(plain.status.success());
	let parsed: Value = serde_json::from_slice(&plain.stdout)
		.unwrap_or_else(|error| panic!("plain IDL failed to parse: {error}"));
	assert_eq!(parsed, value);

	let output_path = directory.path().join("nested/fetched.json");
	let written = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"idl",
			"fetch",
			"--cluster",
			"devnet",
			"--program-id",
			PROGRAM_ID,
			"--output",
			output_path.to_str().unwrap_or_default(),
			"--npx",
			runner.to_str().unwrap_or_default(),
		])
		.output()
		.unwrap_or_else(|error| panic!("output fetch failed to start: {error}"));
	assert!(written.status.success());
	let parsed: Value = serde_json::from_slice(&fs::read(output_path).unwrap_or_default())
		.unwrap_or_else(|error| panic!("written IDL failed to parse: {error}"));
	assert_eq!(parsed, value);
}

#[test]
fn fetch_infers_the_program_from_a_generated_project_idl() {
	let directory =
		tempfile::tempdir().unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
	let value = fixture_idl();
	let (runner, _) = fake_npx(directory.path(), &raw_zlib(&value), 0);
	let project = workspace_root().join("examples/anchor_declare_id");
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"idl",
			"fetch",
			"--cluster",
			"devnet",
			"--project",
			project.to_str().unwrap_or_default(),
			"--npx",
			runner.to_str().unwrap_or_default(),
		])
		.output()
		.unwrap_or_else(|error| panic!("inferred fetch failed to start: {error}"));
	assert!(
		output.status.success(),
		"{}",
		String::from_utf8_lossy(&output.stderr)
	);
}

#[test]
fn semantic_diff_has_distinct_equal_and_different_statuses() {
	let directory =
		tempfile::tempdir().unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
	let value = fixture_idl();
	let local = write_idl(directory.path(), &value);
	let (equal_runner, _) = fake_npx(directory.path(), &raw_zlib(&value), 0);
	let equal = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"idl",
			"diff",
			"--cluster",
			"devnet",
			"--program-id",
			PROGRAM_ID,
			"--file",
			local.to_str().unwrap_or_default(),
			"--npx",
			equal_runner.to_str().unwrap_or_default(),
		])
		.output()
		.unwrap_or_else(|error| panic!("equal diff failed to start: {error}"));
	assert_eq!(equal.status.code(), Some(0));

	let mut changed = value.clone();
	changed["program"]["version"] = Value::String("99.0.0".to_owned());
	let (different_runner, _) = fake_npx(directory.path(), &raw_zlib(&changed), 0);
	let different = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"idl",
			"diff",
			"--cluster",
			"devnet",
			"--program-id",
			PROGRAM_ID,
			"--file",
			local.to_str().unwrap_or_default(),
			"--npx",
			different_runner.to_str().unwrap_or_default(),
			"--json",
		])
		.output()
		.unwrap_or_else(|error| panic!("different diff failed to start: {error}"));
	assert_eq!(different.status.code(), Some(2));
	let report: Value = serde_json::from_slice(&different.stdout)
		.unwrap_or_else(|error| panic!("diff JSON failed: {error}"));
	assert_eq!(report["equal"], false);
}

#[test]
fn publish_export_preserves_all_framing_and_never_submits() {
	let directory =
		tempfile::tempdir().unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
	let local = write_idl(directory.path(), &fixture_idl());
	let framed = "Exporting 2 transactions:\n[Transaction #1]\nAAAA\n[Transaction #2]\nBBBB";
	let (runner, capture) = fake_npx(directory.path(), framed, 0);
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"idl",
			"publish",
			"--cluster",
			"mainnet-beta",
			"--program-id",
			PROGRAM_ID,
			"--file",
			local.to_str().unwrap_or_default(),
			"--export",
			"ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S",
			"--npx",
			runner.to_str().unwrap_or_default(),
		])
		.output()
		.unwrap_or_else(|error| panic!("export failed to start: {error}"));

	assert!(
		output.status.success(),
		"{}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), framed);
	let args =
		fs::read_to_string(capture).unwrap_or_else(|error| panic!("capture read failed: {error}"));
	assert!(args.contains("--export\nProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S"));
	assert!(!args.contains("--keypair"));
}

#[test]
fn bare_export_uses_a_local_authority_and_writes_every_transaction() {
	let directory =
		tempfile::tempdir().unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
	let local = write_idl(directory.path(), &fixture_idl());
	let authority = keypair(directory.path());
	let framed = "Exporting 2 transactions:\n[Transaction #1]\nAAAA\n[Transaction #2]\nBBBB\n";
	let (runner, capture) = fake_npx(directory.path(), framed, 0);
	let export_path = directory.path().join("plans/all.txt");
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"idl",
			"publish",
			"--cluster",
			"mainnet-beta",
			"--file",
			local.to_str().unwrap_or_default(),
			"--authority",
			authority.to_str().unwrap_or_default(),
			"--export",
			"--export-encoding",
			"base58",
			"--output",
			export_path.to_str().unwrap_or_default(),
			"--npx",
			runner.to_str().unwrap_or_default(),
		])
		.output()
		.unwrap_or_else(|error| panic!("local export failed to start: {error}"));

	assert!(
		output.status.success(),
		"{}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert!(output.stdout.is_empty());
	assert_eq!(fs::read_to_string(export_path).unwrap_or_default(), framed);
	let args = fs::read_to_string(capture).unwrap_or_default();
	assert!(args.contains("--keypair"));
	assert!(args.contains("--export\n--export-encoding\nbase58"));
}

#[test]
fn direct_publish_requires_confirmation_before_starting_the_client() {
	let directory =
		tempfile::tempdir().unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
	let local = write_idl(directory.path(), &fixture_idl());
	let authority = keypair(directory.path());
	let (runner, capture) = fake_npx(directory.path(), "must not run", 0);
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"idl",
			"publish",
			"--cluster",
			"mainnet-beta",
			"--file",
			local.to_str().unwrap_or_default(),
			"--authority",
			authority.to_str().unwrap_or_default(),
			"--npx",
			runner.to_str().unwrap_or_default(),
		])
		.output()
		.unwrap_or_else(|error| panic!("publish failed to start: {error}"));

	assert_eq!(output.status.code(), Some(1));
	assert!(String::from_utf8_lossy(&output.stderr).contains("requires confirmation"));
	assert!(!capture.exists());
}

#[test]
fn confirmed_publish_returns_a_machine_readable_result() {
	let directory =
		tempfile::tempdir().unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
	let local = write_idl(directory.path(), &fixture_idl());
	let authority = keypair(directory.path());
	let (runner, _) = fake_npx(directory.path(), "published", 0);
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"idl",
			"publish",
			"--cluster",
			"devnet",
			"--file",
			local.to_str().unwrap_or_default(),
			"--authority",
			authority.to_str().unwrap_or_default(),
			"--npx",
			runner.to_str().unwrap_or_default(),
			"--yes",
			"--json",
		])
		.output()
		.unwrap_or_else(|error| panic!("publish failed to start: {error}"));

	assert!(
		output.status.success(),
		"{}",
		String::from_utf8_lossy(&output.stderr)
	);
	let result: Value = serde_json::from_slice(&output.stdout)
		.unwrap_or_else(|error| panic!("publish JSON failed: {error}"));
	assert_eq!(result["status"], "published");
}

#[test]
fn confirmed_publish_supports_text_and_redacts_custom_rpc_output() {
	let directory =
		tempfile::tempdir().unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
	let local = write_idl(directory.path(), &fixture_idl());
	let authority = keypair(directory.path());
	let (runner, _) = fake_npx(directory.path(), "published", 0);
	let text = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"idl",
			"publish",
			"--cluster",
			"https://rpc.example.test:9443",
			"--file",
			local.to_str().unwrap_or_default(),
			"--authority",
			authority.to_str().unwrap_or_default(),
			"--npx",
			runner.to_str().unwrap_or_default(),
			"--yes",
		])
		.output()
		.unwrap_or_else(|error| panic!("text publish failed to start: {error}"));
	assert!(text.status.success());
	assert!(String::from_utf8_lossy(&text.stdout).contains("Published the canonical IDL"));
}

#[test]
fn confirmed_publish_can_generate_the_project_idl_in_memory() {
	let directory =
		tempfile::tempdir().unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
	let authority = keypair(directory.path());
	let (runner, _) = fake_npx(directory.path(), "published", 0);
	let project = workspace_root().join("examples/anchor_declare_id");
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"idl",
			"publish",
			"--cluster",
			"devnet",
			"--project",
			project.to_str().unwrap_or_default(),
			"--authority",
			authority.to_str().unwrap_or_default(),
			"--npx",
			runner.to_str().unwrap_or_default(),
			"--yes",
		])
		.output()
		.unwrap_or_else(|error| panic!("generated publish failed to start: {error}"));
	assert!(
		output.status.success(),
		"{}",
		String::from_utf8_lossy(&output.stderr)
	);
}

#[test]
fn publish_rejects_incompatible_authority_modes_before_spawning() {
	let directory =
		tempfile::tempdir().unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
	let local = write_idl(directory.path(), &fixture_idl());
	let authority = keypair(directory.path());
	let (runner, capture) = fake_npx(directory.path(), "must not run", 0);
	let multisig = "ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S";

	for extra in [
		vec![
			"--export",
			multisig,
			"--authority",
			authority.to_str().unwrap_or_default(),
		],
		vec![
			"--export",
			multisig,
			"--authority",
			authority.to_str().unwrap_or_default(),
			"--payer",
			authority.to_str().unwrap_or_default(),
		],
		vec!["--export"],
		Vec::new(),
	] {
		let mut args = vec![
			"idl",
			"publish",
			"--cluster",
			"devnet",
			"--file",
			local.to_str().unwrap_or_default(),
			"--npx",
			runner.to_str().unwrap_or_default(),
		];
		args.extend(extra);
		let output = Command::new(env!("CARGO_BIN_EXE_pina"))
			.args(args)
			.output()
			.unwrap_or_else(|error| panic!("invalid publish failed to start: {error}"));
		assert_eq!(output.status.code(), Some(1));
	}
	assert!(!capture.exists());
}

#[test]
fn official_client_errors_have_status_one() {
	let directory =
		tempfile::tempdir().unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
	let local = write_idl(directory.path(), &fixture_idl());
	let (runner, _) = fake_npx(directory.path(), "failure", 9);
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"idl",
			"diff",
			"--cluster",
			"devnet",
			"--file",
			local.to_str().unwrap_or_default(),
			"--npx",
			runner.to_str().unwrap_or_default(),
		])
		.output()
		.unwrap_or_else(|error| panic!("failing diff failed to start: {error}"));

	assert_eq!(output.status.code(), Some(1));
	assert!(String::from_utf8_lossy(&output.stderr).contains("status 9"));
}
