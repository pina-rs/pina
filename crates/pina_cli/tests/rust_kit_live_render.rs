//! Live smoke: `pina generate --client rust-kit` against the published
//! `codama-renderers-rust-solana-kit@0.2.1`.
//!
//! The CLI's own render script runs here, not a copy of it, so the prepare
//! step, staging, and scaffold publication are all exercised against the real
//! renderer. `fake_npx` in `project_commands.rs` covers the CLI plumbing but
//! cannot see any of those.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::process::Output;

const PROGRAM: &str = r#"use pina::*;

declare_id!("11111111111111111111111111111111");

#[discriminator]
pub enum CounterInstruction {
	Initialize = 0,
}

#[instruction(discriminator = CounterInstruction::Initialize)]
pub struct InitializeInstruction {
	pub value: u8,
}

#[derive(Accounts)]
pub struct InitializeAccounts<'a> {
	pub payer: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _args = InitializeInstruction::try_from_bytes(data)?;
		self.payer.assert_signer()?;
		Ok(())
	}
}

pub fn process_instruction(
	program_id: &Address,
	accounts: &mut [AccountView],
	data: &[u8],
) -> ProgramResult {
	let instruction: CounterInstruction = parse_instruction(program_id, &ID, data)?;

	match instruction {
		CounterInstruction::Initialize => {
			InitializeAccounts::try_from((program_id, accounts))?.process(data)
		}
	}
}
"#;

#[test]
fn generate_renders_and_scaffolds_a_rust_kit_client() {
	// Windows CI portability runs this crate with the image Node (currently 22).
	// The Kit renderer package engines field requires >= 24; skip there rather
	// than fail a Rust-portability job that does not install that toolchain.
	// Linux/devenv (Node 24) still executes the smoke.
	let Some(version) = node_major() else {
		eprintln!("skipping live rust-kit render: node is not on PATH");
		return;
	};
	if version < 24 {
		eprintln!("skipping live rust-kit render: Node >= 24 required (found {version})");
		return;
	}

	let temp = tempfile::TempDir::new().expect("temp dir");
	let project = temp.path().join("project");
	write_project(&project);
	npm_install(&project);

	let tmp = renderer_tmpdir(temp.path());
	let client = project.join("clients/rust-kit/counter_program");

	let output = generate(&project, temp.path(), &tmp);
	assert_success(&output);
	assert!(
		client.join("src/generated/mod.rs").is_file(),
		"the renderer must publish generated modules"
	);
	let manifest = fs::read_to_string(client.join("Cargo.toml")).expect("scaffold Cargo.toml");
	assert!(
		manifest.contains("name = \"counter-program-kit-client\"")
			&& manifest.contains("solana_kit_core = { version = \"0.2.1\""),
		"unexpected scaffold manifest:\n{manifest}"
	);
	let lib = fs::read_to_string(client.join("src/lib.rs")).expect("scaffold lib.rs");
	assert!(
		lib.contains("#![no_std]") && lib.contains("pub mod generated;"),
		"unexpected scaffold lib.rs:\n{lib}"
	);

	// Recreating a deleted manifest must not replace a crate root the developer
	// has since edited.
	let edited = "#![no_std]\n\nextern crate alloc;\n\npub mod generated;\npub mod extra;\n";
	fs::write(client.join("src/lib.rs"), edited).expect("edit lib.rs");
	fs::remove_file(client.join("Cargo.toml")).expect("remove Cargo.toml");

	let output = generate(&project, temp.path(), &tmp);
	assert_success(&output);
	assert!(
		client.join("Cargo.toml").is_file(),
		"a missing scaffold manifest must be recreated"
	);
	assert_eq!(
		fs::read_to_string(client.join("src/lib.rs")).expect("read lib.rs"),
		edited,
		"regeneration must keep an existing lib.rs"
	);
}

/// The temporary directory the render script stages under.
///
/// On Unix this is reached through a symbolic link, the way macOS hands out
/// `TMPDIR` beneath `/var` (a link to `/private/var`). The Kit renderer refuses
/// to write beneath a symlinked ancestor, so this proves the script resolves
/// its staging directory first.
fn renderer_tmpdir(root: &Path) -> std::path::PathBuf {
	let real = root.join("real-tmp");
	fs::create_dir_all(&real).expect("tmp dir");

	#[cfg(unix)]
	{
		let link = root.join("linked-tmp");
		std::os::unix::fs::symlink(&real, &link).expect("tmp symlink");
		link
	}

	#[cfg(not(unix))]
	{
		real
	}
}

fn generate(project: &Path, root: &Path, tmp: &Path) -> Output {
	Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(project)
		.env("CARGO", env!("CARGO"))
		.env("CARGO_TARGET_DIR", root.join("target"))
		.env("TMPDIR", tmp)
		.env_remove("GIT_DIR")
		.env_remove("GIT_WORK_TREE")
		.args(["generate", "--client", "rust-kit", "--npx", "node"])
		.output()
		.expect("run pina generate")
}

fn assert_success(output: &Output) {
	assert!(
		output.status.success(),
		"rust-kit generation failed:\nstdout:\n{}\nstderr:\n{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
}

fn write_project(root: &Path) {
	fs::create_dir_all(root.join("src")).expect("project src dir");
	fs::write(
		root.join("Cargo.toml"),
		r#"[package]
name = "counter-program"
version = "0.1.0"
edition = "2024"

[lib]
name = "counter_program"
crate-type = ["cdylib", "lib"]

[features]
default = []
bpf-entrypoint = []
"#,
	)
	.expect("project Cargo.toml");
	fs::write(
		root.join("pina.toml"),
		"[project]\nprogram = \".\"\n\n[clients]\noutput = \"clients\"\nlanguages = \
		 [\"rust-kit\"]\n",
	)
	.expect("pina.toml");
	fs::write(root.join("src/lib.rs"), PROGRAM).expect("program source");
	// `--npx node` resolves the renderer packages from the project on purpose.
	fs::write(
		root.join("package.json"),
		r#"{
  "name": "pina-rust-kit-live-render",
  "private": true,
  "type": "module",
  "dependencies": {
    "codama": "1.11.0",
    "codama-renderers-rust-solana-kit": "0.2.1"
  }
}
"#,
	)
	.expect("package.json");
}

fn node_major() -> Option<u32> {
	let output = Command::new("node")
		.arg("-p")
		.arg("process.versions.node")
		.output()
		.ok()?;
	if !output.status.success() {
		return None;
	}
	String::from_utf8_lossy(&output.stdout)
		.trim()
		.split('.')
		.next()?
		.parse()
		.ok()
}

fn npm_install(pkg: &Path) {
	let output = Command::new("npm")
		.current_dir(pkg)
		.args(["install", "--ignore-scripts", "--no-fund", "--no-audit"])
		.output()
		.expect("npm install");
	assert!(
		output.status.success(),
		"npm install failed:\n{}",
		String::from_utf8_lossy(&output.stderr)
	);
}
