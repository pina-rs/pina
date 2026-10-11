use std::fs;
use std::path::PathBuf;
use std::process::Command;

struct TempDir {
	path: PathBuf,
}

impl TempDir {
	fn new(prefix: &str) -> Self {
		let timestamp = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.map_or(0, |duration| duration.as_nanos());
		let path = std::env::temp_dir().join(format!(
			"pina_cli_init_test_{prefix}_{}_{}",
			std::process::id(),
			timestamp
		));
		Self { path }
	}
}

impl Drop for TempDir {
	fn drop(&mut self) {
		let _ = fs::remove_dir_all(&self.path);
	}
}

#[test]
fn init_command_creates_project() {
	let dir = TempDir::new("create");
	let project_path = dir.path.join("my_program");

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.arg("init")
		.arg("my_program")
		.arg("--path")
		.arg(&project_path)
		.output()
		.unwrap_or_else(|err| panic!("failed to execute pina binary: {err}"));
	assert!(
		output.status.success(),
		"expected successful init command, stderr: {}",
		String::from_utf8_lossy(&output.stderr)
	);

	assert!(project_path.join("Cargo.toml").exists());
	assert!(project_path.join("src/lib.rs").exists());
}

#[test]
fn init_command_refuses_to_overwrite_without_force() {
	let dir = TempDir::new("overwrite");
	let project_path = dir.path.join("my_program");
	fs::create_dir_all(project_path.join("src"))
		.unwrap_or_else(|err| panic!("expected src dir creation to succeed: {err}"));
	fs::write(project_path.join("Cargo.toml"), "invalid")
		.unwrap_or_else(|err| panic!("expected seed file write to succeed: {err}"));

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.arg("init")
		.arg("my_program")
		.arg("--path")
		.arg(&project_path)
		.output()
		.unwrap_or_else(|err| panic!("failed to execute pina binary: {err}"));
	assert!(
		!output.status.success(),
		"expected init command to fail without --force"
	);
	assert!(
		String::from_utf8_lossy(&output.stderr).contains("refusing to overwrite"),
		"expected overwrite refusal message, stderr: {}",
		String::from_utf8_lossy(&output.stderr)
	);
}

/// The scaffolded release profile must contain every setting that measurably
/// shrinks a deployed program. `panic = "abort"` and `strip = true` are free
/// wins; a dual crate type is the one mistake that silently costs fat LTO.
#[test]
fn init_scaffolds_the_production_release_profile() {
	let dir = TempDir::new("profile");
	let project_path = dir.path.join("profile_program");

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.arg("init")
		.arg("profile_program")
		.arg("--path")
		.arg(&project_path)
		.output()
		.unwrap_or_else(|err| panic!("failed to execute pina binary: {err}"));
	assert!(
		output.status.success(),
		"expected successful init command, stderr: {}",
		String::from_utf8_lossy(&output.stderr)
	);

	let manifest = fs::read_to_string(project_path.join("Cargo.toml"))
		.unwrap_or_else(|error| panic!("read scaffolded manifest: {error}"));

	assert!(
		manifest.contains("panic = \"abort\""),
		"deployed programs should not pay for unwinding: {manifest}"
	);
	assert!(
		manifest.contains("strip = true"),
		"the scaffold should strip symbols from the deployed binary: {manifest}"
	);
	assert!(
		manifest.contains("lto = \"fat\"") && manifest.contains("codegen-units = 1"),
		"the scaffold should keep the size-oriented LTO settings: {manifest}"
	);
	assert!(
		manifest.contains("[profile.release.build-override]"),
		"build scripts and proc macros should not run at the deployed program's opt-level: \
		 {manifest}"
	);
	// The crate-type must stay cdylib-only, because a second lib/rlib target
	// makes rustc refuse LTO and costs 20-35% of program size.
	assert!(
		manifest.contains("crate-type = [\"cdylib\"]"),
		"the scaffold must not add a second crate type: {manifest}"
	);
}

/// The upstream sbpf-linker scaffold must carry the static-syscall rustflags
/// and swap the build alias away from cargo-build-sbf; the flags are
/// load-bearing, see docs/sbpf-linker-evaluation.md.
#[test]
fn init_command_with_sbpf_linker_scaffolds_upstream_config() {
	let dir = TempDir::new("upstream");
	let project_path = dir.path.join("upstream_program");

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.arg("init")
		.arg("upstream_program")
		.arg("--path")
		.arg(&project_path)
		.arg("--sbpf-linker")
		.output()
		.unwrap_or_else(|err| panic!("failed to execute pina binary: {err}"));
	assert!(
		output.status.success(),
		"expected successful init command, stderr: {}",
		String::from_utf8_lossy(&output.stderr)
	);

	let config = fs::read_to_string(project_path.join(".cargo/config.toml"))
		.unwrap_or_else(|error| panic!("read scaffolded config: {error}"));
	assert!(
		config.contains("[target.bpfel-unknown-none]"),
		"expected upstream target rustflags: {config}"
	);
	assert!(
		config.contains("linker=sbpf-linker"),
		"expected the sbpf-linker driver: {config}"
	);
	assert!(
		config.contains("static-syscalls") && config.contains("target_os=\\\"solana\\\""),
		"the static-syscall cfg flags are load-bearing: {config}"
	);
	assert!(
		config.contains("build-program = \"build --release --target bpfel-unknown-none"),
		"expected the upstream build alias: {config}"
	);
	assert!(
		!config.contains("build-sbf"),
		"the upstream scaffold must not reference cargo-build-sbf: {config}"
	);

	// The pina.toml key is what routes `pina build` to the upstream toolchain.
	let pina_toml = fs::read_to_string(project_path.join("pina.toml"))
		.unwrap_or_else(|error| panic!("read scaffolded pina.toml: {error}"));
	assert!(
		pina_toml.contains("sbpf-linker = true"),
		"expected the scaffold to record the upstream toolchain: {pina_toml}"
	);

	let readme = fs::read_to_string(project_path.join("README.md"))
		.unwrap_or_else(|error| panic!("read scaffolded readme: {error}"));
	assert!(
		readme.contains("sbpf-linker = true"),
		"expected the upstream pina.toml key in the instructions: {readme}"
	);
	assert!(
		readme.contains("cargo binstall sbpf-linker"),
		"expected the sbpf-linker install instructions: {readme}"
	);
}
