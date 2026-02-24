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
		.arg("--npx")
		.arg(fake_npx);
	assert_cmd_snapshot!("codama_generate_missing_examples_path_error", command);
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
