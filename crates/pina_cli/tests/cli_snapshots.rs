use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use insta_cmd::assert_cmd_snapshot;

fn workspace_root() -> &'static Path {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.and_then(|path| path.parent())
		.unwrap_or_else(|| Path::new("."))
}

fn reset_snapshot_dir(name: &str) -> PathBuf {
	let path = workspace_root().join("target/cli-snapshot-temp").join(name);
	let _ = fs::remove_dir_all(&path);
	fs::create_dir_all(&path).unwrap_or_else(|error| {
		panic!(
			"failed to create snapshot temp directory {}: {error}",
			path.display()
		)
	});
	path
}

fn workspace_relative(path: &Path) -> String {
	path.strip_prefix(workspace_root())
		.unwrap_or(path)
		.to_string_lossy()
		.replace('\\', "/")
}

fn create_fake_npx(temp_dir: &Path) -> String {
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;

		let path = temp_dir.join("fake-npx.sh");
		fs::write(&path, "#!/usr/bin/env bash\nset -euo pipefail\nexit 0\n").unwrap_or_else(
			|error| {
				panic!(
					"failed to write fake npx script {}: {error}",
					path.display()
				)
			},
		);
		let metadata = fs::metadata(&path).unwrap_or_else(|error| {
			panic!("failed to stat fake npx script {}: {error}", path.display())
		});
		let mut permissions = metadata.permissions();
		permissions.set_mode(0o755);
		fs::set_permissions(&path, permissions).unwrap_or_else(|error| {
			panic!(
				"failed to set executable permissions on fake npx script {}: {error}",
				path.display()
			)
		});
		return workspace_relative(&path);
	}

	#[cfg(windows)]
	{
		let path = temp_dir.join("fake-npx.cmd");
		fs::write(&path, "@echo off\r\nexit /b 0\r\n").unwrap_or_else(|error| {
			panic!(
				"failed to write fake npx script {}: {error}",
				path.display()
			)
		});
		workspace_relative(&path)
	}
}

#[test]
fn root_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.arg("--help");
	assert_cmd_snapshot!("root_help", command);
}

#[test]
fn build_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["build", "--help"]);
	assert_cmd_snapshot!("build_help", command);
}

#[test]
fn generate_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["generate", "--help"]);
	assert_cmd_snapshot!("generate_help", command);
}

#[test]
fn idl_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["idl", "--help"]);
	assert_cmd_snapshot!("idl_help", command);
}

#[test]
fn idl_generate_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["idl", "generate", "--help"]);
	assert_cmd_snapshot!("idl_generate_help", command);
}

#[test]
fn idl_fetch_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["idl", "fetch", "--help"]);
	assert_cmd_snapshot!("idl_fetch_help", command);
}

#[test]
fn idl_diff_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["idl", "diff", "--help"]);
	assert_cmd_snapshot!("idl_diff_help", command);
}

#[test]
fn idl_publish_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["idl", "publish", "--help"]);
	assert_cmd_snapshot!("idl_publish_help", command);
}

#[test]
fn docs_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["docs", "--help"]);
	assert_cmd_snapshot!("docs_help", command);
}

#[test]
fn docs_index_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.arg("docs");
	assert_cmd_snapshot!("docs_index", command);
}

#[test]
fn docs_unknown_topic_error_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["docs", "unknown-topic"]);
	assert_cmd_snapshot!("docs_unknown_topic_error", command);
}

#[test]
fn init_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["init", "--help"]);
	assert_cmd_snapshot!("init_help", command);
}

#[test]
fn profile_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["profile", "--help"]);
	assert_cmd_snapshot!("profile_help", command);
}

#[test]
fn verify_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["verify", "--help"]);
	assert_cmd_snapshot!("verify_help", command);
}

#[test]
fn verify_check_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["verify", "check", "--help"]);
	assert_cmd_snapshot!("verify_check_help", command);
}

#[test]
fn verify_record_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["verify", "record", "--help"]);
	assert_cmd_snapshot!("verify_record_help", command);
}

#[test]
fn verify_submit_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["verify", "submit", "--help"]);
	assert_cmd_snapshot!("verify_submit_help", command);
}

#[test]
fn verify_status_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["verify", "status", "--help"]);
	assert_cmd_snapshot!("verify_status_help", command);
}

#[test]
fn codama_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["codama", "--help"]);
	assert_cmd_snapshot!("codama_help", command);
}

#[test]
fn codama_generate_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["codama", "generate", "--help"]);
	assert_cmd_snapshot!("codama_generate_help", command);
}

#[test]
fn idl_stdout_is_machine_readable_json() {
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(workspace_root())
		.args(["idl", "--path", "examples/anchor_declare_id", "--compact"])
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina idl: {error}"));

	assert!(output.status.success());
	serde_json::from_slice::<serde_json::Value>(&output.stdout)
		.unwrap_or_else(|error| panic!("IDL stdout was not valid JSON: {error}"));
	assert!(String::from_utf8_lossy(&output.stderr).contains("IDL generation complete"));
}

#[test]
fn idl_output_file_is_machine_readable_json() {
	let temp_dir = reset_snapshot_dir("idl_output_file");
	let output_path = temp_dir.join("anchor_declare_id.json");
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(workspace_root())
		.args([
			"idl",
			"--path",
			"examples/anchor_declare_id",
			"--output",
			&workspace_relative(&output_path),
		])
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina idl: {error}"));

	assert!(output.status.success());
	assert!(output.stdout.is_empty());
	let json = fs::read(&output_path).unwrap_or_else(|error| {
		panic!(
			"failed to read generated IDL {}: {error}",
			output_path.display()
		)
	});
	serde_json::from_slice::<serde_json::Value>(&json)
		.unwrap_or_else(|error| panic!("generated IDL was not valid JSON: {error}"));
	assert!(String::from_utf8_lossy(&output.stderr).contains("Wrote"));
}

#[test]
fn idl_legacy_pretty_flag_remains_accepted() {
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(workspace_root())
		.args(["idl", "--path", "examples/anchor_declare_id", "--pretty"])
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina idl: {error}"));

	assert!(output.status.success());
	serde_json::from_slice::<serde_json::Value>(&output.stdout)
		.unwrap_or_else(|error| panic!("IDL stdout was not valid JSON: {error}"));
}

#[test]
fn explicit_idl_generate_matches_bare_idl() {
	let bare = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(workspace_root())
		.args(["idl", "--path", "examples/anchor_declare_id", "--compact"])
		.output()
		.unwrap_or_else(|error| panic!("failed to run bare pina idl: {error}"));
	let explicit = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(workspace_root())
		.args([
			"idl",
			"generate",
			"--path",
			"examples/anchor_declare_id",
			"--compact",
		])
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina idl generate: {error}"));

	assert!(bare.status.success());
	assert!(explicit.status.success());
	assert_eq!(bare.stdout, explicit.stdout);
}

#[test]
fn idl_success_output_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command
		.current_dir(workspace_root())
		.args(["idl", "--path", "examples/anchor_declare_id"]);
	assert_cmd_snapshot!("idl_success_output", command);
}

#[test]
fn codama_generate_success_output_snapshot() {
	let temp_dir = reset_snapshot_dir("codama_generate_success");
	let fake_npx = create_fake_npx(&temp_dir);
	let idls_dir = temp_dir.join("idls");
	let rust_out = temp_dir.join("rust");
	let js_out = temp_dir.join("js");
	let dart_out = temp_dir.join("dart");
	let js_generated = js_out.join("counter_program/src/generated");
	let dart_generated = dart_out.join("lib/src/generated/counter_program");
	fs::create_dir_all(&js_generated).unwrap_or_else(|error| {
		panic!(
			"failed to create fake JavaScript client directory {}: {error}",
			js_generated.display()
		)
	});
	fs::create_dir_all(&dart_generated).unwrap_or_else(|error| {
		panic!(
			"failed to create fake Dart client directory {}: {error}",
			dart_generated.display()
		)
	});

	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command
		.current_dir(workspace_root())
		.arg("codama")
		.arg("generate")
		.arg("--examples-dir")
		.arg("examples")
		.arg("--idls-dir")
		.arg(workspace_relative(&idls_dir))
		.arg("--rust-out")
		.arg(workspace_relative(&rust_out))
		.arg("--js-out")
		.arg(workspace_relative(&js_out))
		.arg("--dart-out")
		.arg(workspace_relative(&dart_out))
		.arg("--example")
		.arg("counter_program")
		.arg("--npx")
		.arg(fake_npx);
	assert_cmd_snapshot!("codama_generate_success_output", command);

	assert!(
		idls_dir.join("counter_program.json").is_file(),
		"expected generated counter_program IDL at {}",
		idls_dir.join("counter_program.json").display()
	);
	assert!(
		rust_out
			.join("counter_program")
			.join("src/generated/mod.rs")
			.is_file(),
		"expected generated Rust client module at {}",
		rust_out
			.join("counter_program")
			.join("src/generated/mod.rs")
			.display()
	);
	assert!(
		dart_out.join("lib/counter_program.dart").is_file(),
		"expected generated Dart package entrypoint at {}",
		dart_out.join("lib/counter_program.dart").display()
	);
	assert!(
		js_out
			.join("counter_program")
			.join("src/generated/zeropodCodecs.ts")
			.is_file(),
		"expected generated JavaScript validation helpers at {}",
		js_out
			.join("counter_program")
			.join("src/generated/zeropodCodecs.ts")
			.display()
	);
}

#[test]
fn codama_generate_unknown_example_error_snapshot() {
	let temp_dir = reset_snapshot_dir("codama_generate_unknown_example");
	let fake_npx = create_fake_npx(&temp_dir);

	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command
		.current_dir(workspace_root())
		.arg("codama")
		.arg("generate")
		.arg("--examples-dir")
		.arg("examples")
		.arg("--idls-dir")
		.arg(workspace_relative(&temp_dir.join("idls")))
		.arg("--rust-out")
		.arg(workspace_relative(&temp_dir.join("rust")))
		.arg("--js-out")
		.arg(workspace_relative(&temp_dir.join("js")))
		.arg("--dart-out")
		.arg(workspace_relative(&temp_dir.join("dart")))
		.arg("--example")
		.arg("does_not_exist")
		.arg("--npx")
		.arg(fake_npx);
	assert_cmd_snapshot!("codama_generate_unknown_example_error", command);
}

#[test]
fn codama_generate_missing_examples_path_error_snapshot() {
	let temp_dir = reset_snapshot_dir("codama_generate_missing_examples");
	let fake_npx = create_fake_npx(&temp_dir);

	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command
		.current_dir(workspace_root())
		.arg("codama")
		.arg("generate")
		.arg("--examples-dir")
		.arg(workspace_relative(&temp_dir.join("missing_examples")))
		.arg("--idls-dir")
		.arg(workspace_relative(&temp_dir.join("idls")))
		.arg("--rust-out")
		.arg(workspace_relative(&temp_dir.join("rust")))
		.arg("--js-out")
		.arg(workspace_relative(&temp_dir.join("js")))
		.arg("--dart-out")
		.arg(workspace_relative(&temp_dir.join("dart")))
		.arg("--npx")
		.arg(fake_npx);
	let output = command
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina codama generate: {error}"));
	let stderr = String::from_utf8_lossy(&output.stderr);

	assert!(!output.status.success());
	assert!(stderr.contains("Failed to read examples directory"));
	assert!(stderr.contains("missing_examples"));
}

#[test]
fn codama_generate_invalid_argument_error_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command
		.current_dir(workspace_root())
		.arg("codama")
		.arg("generate")
		.arg("--example");
	assert_cmd_snapshot!("codama_generate_invalid_argument_error", command);
}
