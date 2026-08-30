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

#[cfg(unix)]
fn create_executable(path: &Path, contents: &str) {
	use std::os::unix::fs::PermissionsExt;

	fs::write(path, contents)
		.unwrap_or_else(|error| panic!("failed to write {}: {error}", path.display()));
	let mut permissions = fs::metadata(path)
		.unwrap_or_else(|error| panic!("failed to stat {}: {error}", path.display()))
		.permissions();
	permissions.set_mode(0o755);
	fs::set_permissions(path, permissions)
		.unwrap_or_else(|error| panic!("failed to make {} executable: {error}", path.display()));
}

#[cfg(unix)]
fn create_fake_workflow_project(name: &str) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
	let project = reset_snapshot_dir(name);
	let manifest = project.join("Cargo.toml");
	let target = project.join("target");
	let cargo = project.join("fake-cargo.sh");
	let log = project.join("commands.log");
	pina_cli::init_project(&project, "test_program", false)
		.unwrap_or_else(|error| panic!("failed to create fake Pina project: {error}"));
	let cargo_toml = fs::read_to_string(&manifest)
		.unwrap_or_else(|error| panic!("failed to read {}: {error}", manifest.display()));
	fs::write(&manifest, format!("{cargo_toml}\n[workspace]\n"))
		.unwrap_or_else(|error| panic!("failed to isolate {}: {error}", manifest.display()));
	create_executable(
		&cargo,
		r#"#!/usr/bin/env bash
set -euo pipefail
printf 'cargo' >> "$PINA_FAKE_LOG"
printf ' %q' "$@" >> "$PINA_FAKE_LOG"
printf '\n' >> "$PINA_FAKE_LOG"
case "${1:-}" in
	metadata)
		exec cargo "$@"
		;;
	build-sbf)
		mkdir -p "$PINA_FAKE_TARGET/sbf-build"
		: > "$PINA_FAKE_TARGET/sbf-build/test_program.so"
		;;
	test)
		if [[ " $* " == *" --lib "* ]]; then
			test -f "${PINA_SBF_ARTIFACT:?}"
			printf 'artifact %s\n' "$PINA_SBF_ARTIFACT" >> "$PINA_FAKE_LOG"
		fi
		;;
	*) exit 91 ;;
esac
"#,
	);
	(project, manifest, target, log)
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
fn lint_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["lint", "--help"]);
	assert_cmd_snapshot!("lint_help", command);
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
fn docs_load_bundled_and_custom_topics() {
	let bundled = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["docs", "pina-overview"])
		.output()
		.unwrap_or_else(|error| panic!("failed to load bundled docs: {error}"));
	assert!(bundled.status.success());
	assert!(String::from_utf8_lossy(&bundled.stdout).contains("Pina"));

	let templates = reset_snapshot_dir("custom_docs");
	fs::write(templates.join("local.t.md"), "# Local Pina docs\n")
		.unwrap_or_else(|error| panic!("failed to write custom docs: {error}"));
	let custom = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["docs", "local"])
		.env("PINA_TEMPLATES_DIR", &templates)
		.output()
		.unwrap_or_else(|error| panic!("failed to load custom docs: {error}"));
	assert!(custom.status.success());
	assert!(String::from_utf8_lossy(&custom.stdout).contains("Local Pina docs"));
}

#[test]
fn docs_report_custom_template_failures_and_attempted_paths() {
	let templates = reset_snapshot_dir("invalid_custom_docs");
	fs::write(templates.join("invalid.t.md"), [0xff])
		.unwrap_or_else(|error| panic!("failed to write invalid custom docs: {error}"));
	let invalid = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["docs", "invalid"])
		.env("PINA_TEMPLATES_DIR", &templates)
		.output()
		.unwrap_or_else(|error| panic!("failed to run invalid custom docs: {error}"));
	assert!(!invalid.status.success());
	assert!(String::from_utf8_lossy(&invalid.stderr).contains("Failed to read template"));

	let missing = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["docs", "missing"])
		.env("PINA_TEMPLATES_DIR", &templates)
		.output()
		.unwrap_or_else(|error| panic!("failed to run missing custom docs: {error}"));
	assert!(!missing.status.success());
	assert!(String::from_utf8_lossy(&missing.stderr).contains("Attempted template paths"));
}

#[test]
fn init_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["init", "--help"]);
	assert_cmd_snapshot!("init_help", command);
}

#[test]
fn keys_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["keys", "--help"]);
	assert_cmd_snapshot!("keys_help", command);
}

#[test]
fn keys_sync_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["keys", "sync", "--help"]);
	assert_cmd_snapshot!("keys_sync_help", command);
}

#[test]
fn keys_show_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["keys", "show", "--help"]);
	assert_cmd_snapshot!("keys_show_help", command);
}

#[test]
fn keys_new_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["keys", "new", "--help"]);
	assert_cmd_snapshot!("keys_new_help", command);
}

#[test]
fn doctor_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["doctor", "--help"]);
	assert_cmd_snapshot!("doctor_help", command);
}

#[test]
fn completions_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["completions", "--help"]);
	assert_cmd_snapshot!("completions_help", command);
}

#[test]
fn bash_completions_are_written_to_stdout() {
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["completions", "bash"])
		.output()
		.unwrap_or_else(|error| panic!("failed to generate completions: {error}"));
	let stdout = String::from_utf8_lossy(&output.stdout);

	assert!(output.status.success());
	assert!(output.stderr.is_empty());
	assert!(stdout.contains("_pina"));
	assert!(stdout.contains("doctor"));
	assert!(stdout.contains("completions"));
}

#[test]
fn test_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["test", "--help"]);
	assert_cmd_snapshot!("test_help", command);
}

#[test]
fn dev_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["dev", "--help"]);
	assert_cmd_snapshot!("dev_help", command);
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
fn deploy_help_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command.args(["deploy", "--help"]);
	assert_cmd_snapshot!("deploy_help", command);
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
fn idl_reports_generation_and_output_write_failures() {
	let generation = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(workspace_root())
		.args(["idl", "--path", "target/definitely-missing-program"])
		.output()
		.unwrap_or_else(|error| panic!("failed to run invalid IDL generation: {error}"));
	assert!(!generation.status.success());
	assert!(String::from_utf8_lossy(&generation.stderr).contains("Error"));

	let output_dir = reset_snapshot_dir("idl_output_write_failure");
	let write = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(workspace_root())
		.args([
			"idl",
			"--path",
			"examples/anchor_declare_id",
			"--output",
			&workspace_relative(&output_dir),
		])
		.output()
		.unwrap_or_else(|error| panic!("failed to run IDL output failure: {error}"));
	assert!(!write.status.success());
	assert!(String::from_utf8_lossy(&write.stderr).contains("Failed to write"));
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

#[cfg(unix)]
#[test]
fn unit_test_mode_never_builds_or_starts_surfpool() {
	let (project, manifest, target, log) = create_fake_workflow_project("unit workflow");
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["test", "--project"])
		.arg(&project)
		.arg("--unit")
		.arg("--filter")
		.arg("authority")
		.env("CARGO", project.join("fake-cargo.sh"))
		.env("CARGO_TARGET_DIR", &target)
		.env("PINA_SURFPOOL", project.join("missing-surfpool"))
		.env("PINA_FAKE_LOG", &log)
		.env("PINA_FAKE_MANIFEST", &manifest)
		.env("PINA_FAKE_TARGET", &target)
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina test --unit: {error}"));

	assert!(output.status.success());
	let commands = fs::read_to_string(&log)
		.unwrap_or_else(|error| panic!("failed to read {}: {error}", log.display()));
	assert!(commands.contains("cargo metadata"));
	assert!(commands.contains("cargo test"));
	assert!(commands.contains("authority"));
	assert!(!commands.contains("cargo build"));
}

#[cfg(unix)]
#[test]
fn surfpool_test_mode_builds_and_requires_the_real_artifact() {
	let (project, manifest, target, log) = create_fake_workflow_project("test workflow");
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["test", "--project"])
		.arg(&project)
		.arg("--filter")
		.arg("deploys")
		.env("CARGO", project.join("fake-cargo.sh"))
		.env("CARGO_TARGET_DIR", &target)
		.env("PINA_FAKE_LOG", &log)
		.env("PINA_FAKE_MANIFEST", &manifest)
		.env("PINA_FAKE_TARGET", &target)
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina test: {error}"));

	assert!(output.status.success());
	let artifact = target.join("deploy/test_program.so");
	assert!(artifact.is_file());
	let commands = fs::read_to_string(&log)
		.unwrap_or_else(|error| panic!("failed to read {}: {error}", log.display()));
	let build_position = commands
		.find("cargo build")
		.unwrap_or_else(|| panic!("missing build command in {commands}"));
	let test_position = commands
		.find("cargo test")
		.unwrap_or_else(|| panic!("missing test command in {commands}"));
	assert!(build_position < test_position);
	assert!(commands.contains("tests/surfpool/Cargo.toml --lib deploys -- --ignored --nocapture"));
	assert!(commands.contains(&format!("artifact {}", artifact.display())));
}

#[cfg(unix)]
#[test]
fn dev_delegates_offline_watch_to_surfpool() {
	let (project, manifest, target, log) = create_fake_workflow_project("dev workflow");
	let surfpool = project.join("fake-surfpool.sh");
	create_executable(
		&surfpool,
		r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--version" ]]; then
	echo "surfpool 1.5.0"
	exit 0
fi
printf 'surfpool' >> "$PINA_FAKE_LOG"
printf ' %q' "$@" >> "$PINA_FAKE_LOG"
printf '\n' >> "$PINA_FAKE_LOG"
"#,
	);
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["dev", "--yes", "--project"])
		.arg(&project)
		.env("CARGO", project.join("fake-cargo.sh"))
		.env("CARGO_TARGET_DIR", &target)
		.env("PINA_SURFPOOL", &surfpool)
		.env("PINA_FAKE_LOG", &log)
		.env("PINA_FAKE_MANIFEST", &manifest)
		.env("PINA_FAKE_TARGET", &target)
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina dev: {error}"));

	assert!(
		output.status.success(),
		"pina dev failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert!(target.join("deploy/test_program.so").is_file());
	let commands = fs::read_to_string(&log)
		.unwrap_or_else(|error| panic!("failed to read {}: {error}", log.display()));
	assert!(commands.contains("cargo build"));
	assert!(commands.contains("surfpool start --watch --artifacts-path"));
	assert!(commands.contains("--offline"));
}

#[cfg(unix)]
#[test]
fn dev_forwards_explicit_upstream_selection() {
	let (project, manifest, target, log) = create_fake_workflow_project("dev upstream workflow");
	let surfpool = project.join("fake-surfpool.sh");
	create_executable(
		&surfpool,
		r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--version" ]]; then
	echo "surfpool 1.5.0"
	exit 0
fi
printf 'surfpool' >> "$PINA_FAKE_LOG"
printf ' %q' "$@" >> "$PINA_FAKE_LOG"
printf '\n' >> "$PINA_FAKE_LOG"
"#,
	);

	for cluster in ["mainnet", "devnet", "testnet"] {
		let status = Command::new(env!("CARGO_BIN_EXE_pina"))
			.args(["dev", "--yes", "--project"])
			.arg(&project)
			.args(["--network", cluster])
			.env("CARGO", project.join("fake-cargo.sh"))
			.env("PINA_SURFPOOL", &surfpool)
			.env("PINA_FAKE_LOG", &log)
			.env("PINA_FAKE_MANIFEST", &manifest)
			.env("PINA_FAKE_TARGET", &target)
			.status()
			.unwrap_or_else(|error| panic!("failed to run pina dev for {cluster}: {error}"));
		assert!(status.success());
	}

	let status = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["dev", "--yes", "--project"])
		.arg(&project)
		.args(["--rpc-url", "http://127.0.0.1:8899"])
		.env("CARGO", project.join("fake-cargo.sh"))
		.env("PINA_SURFPOOL", &surfpool)
		.env("PINA_FAKE_LOG", &log)
		.env("PINA_FAKE_MANIFEST", &manifest)
		.env("PINA_FAKE_TARGET", &target)
		.status()
		.unwrap_or_else(|error| panic!("failed to run pina dev with RPC URL: {error}"));
	assert!(status.success());

	let commands = fs::read_to_string(&log)
		.unwrap_or_else(|error| panic!("failed to read {}: {error}", log.display()));
	for cluster in ["mainnet", "devnet", "testnet"] {
		assert!(commands.contains(&format!("--network {cluster}")));
	}
	assert!(commands.contains("--rpc-url http://127.0.0.1:8899"));
	assert!(!commands.contains("--daemon"));
}

#[cfg(unix)]
#[test]
fn dev_rejects_unsafe_rpc_urls_before_running_project_commands() {
	let (project, manifest, target, log) = create_fake_workflow_project("unsafe RPC workflow");
	let secret = "private-rpc-credential";
	let endpoint = format!("https://agent:{secret}@rpc.example/?token={secret}#fragment");
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["dev", "--yes", "--project"])
		.arg(&project)
		.args(["--rpc-url", &endpoint])
		.env("CARGO", project.join("fake-cargo.sh"))
		.env("PINA_SURFPOOL", project.join("surfpool-that-must-not-run"))
		.env("PINA_FAKE_LOG", &log)
		.env("PINA_FAKE_MANIFEST", &manifest)
		.env("PINA_FAKE_TARGET", &target)
		.output()
		.expect("run pina dev with an unsafe RPC URL");
	let stderr = String::from_utf8_lossy(&output.stderr);

	assert_eq!(output.status.code(), Some(1));
	assert!(stderr.contains("unsafe Surfpool RPC URL"));
	assert!(!stderr.contains(secret));
	assert!(!log.exists(), "project and Surfpool commands must not run");
	assert!(
		!target.exists(),
		"an invalid endpoint must not trigger a build"
	);
}

#[cfg(unix)]
#[test]
fn workflow_errors_are_actionable_and_preserve_exit_status() {
	let empty = tempfile::tempdir().expect("create empty temporary directory");
	let missing = empty.path().join("absent");
	let test_output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["test", "--project"])
		.arg(&missing)
		.output()
		.expect("run failing test workflow");
	assert_eq!(test_output.status.code(), Some(1));
	assert!(String::from_utf8_lossy(&test_output.stderr).contains("Could not inspect"));

	let (project, manifest, target, log) = create_fake_workflow_project("failed dev workflow");
	let missing_runbook = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["dev", "--project"])
		.arg(&project)
		.env("CARGO", project.join("fake-cargo.sh"))
		.env("PINA_FAKE_LOG", &log)
		.env("PINA_FAKE_MANIFEST", &manifest)
		.env("PINA_FAKE_TARGET", &target)
		.output()
		.unwrap_or_else(|error| panic!("failed to run missing-runbook workflow: {error}"));
	assert_eq!(missing_runbook.status.code(), Some(1));
	assert!(String::from_utf8_lossy(&missing_runbook.stderr).contains("run `pina dev --yes` once"));

	let surfpool = executable_script_with_exit(&project, 37);
	let dev_output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["dev", "--yes", "--project"])
		.arg(&project)
		.env("CARGO", project.join("fake-cargo.sh"))
		.env("PINA_SURFPOOL", surfpool)
		.env("PINA_FAKE_LOG", &log)
		.env("PINA_FAKE_MANIFEST", &manifest)
		.env("PINA_FAKE_TARGET", &target)
		.output()
		.expect("run failing development workflow");
	assert_eq!(dev_output.status.code(), Some(37));
}

#[cfg(unix)]
fn executable_script_with_exit(project: &Path, exit_code: u8) -> PathBuf {
	let path = project.join("failed-surfpool.sh");
	create_executable(&path, &format!("#!/bin/sh\nexit {exit_code}\n"));
	path
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
