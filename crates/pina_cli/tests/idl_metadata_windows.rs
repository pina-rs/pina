#![cfg(windows)]

use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use flate2::Compression;
use flate2::write::ZlibEncoder;

const PROGRAM_ID: &str = "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS";

fn workspace_root() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.and_then(Path::parent)
		.unwrap_or_else(|| Path::new("."))
		.to_path_buf()
}

#[test]
fn npx_cmd_receives_the_exact_pinned_raw_fetch_arguments() {
	let directory =
		tempfile::tempdir().unwrap_or_else(|error| panic!("temporary directory failed: {error}"));
	let root = pina_cli::generate_idl(&workspace_root().join("examples/anchor_declare_id"), None)
		.unwrap_or_else(|error| panic!("fixture generation failed: {error}"));
	let source = serde_json::to_vec(&root)
		.unwrap_or_else(|error| panic!("fixture serialization failed: {error}"));
	let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
	encoder
		.write_all(&source)
		.unwrap_or_else(|error| panic!("fixture compression failed: {error}"));
	let hex = encoder
		.finish()
		.unwrap_or_else(|error| panic!("fixture compression finish failed: {error}"))
		.iter()
		.fold(String::new(), |mut output, byte| {
			write!(output, "{byte:02x}")
				.unwrap_or_else(|error| panic!("hex formatting failed: {error}"));
			output
		});
	let runner = directory.path().join("fake npx.cmd");
	let capture = directory.path().join("args.txt");
	fs::write(
		&runner,
		format!(
			"@echo off\r\necho %* > \"{}\"\r\necho {hex}\r\n",
			capture.display()
		),
	)
	.unwrap_or_else(|error| panic!("runner write failed: {error}"));

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
		])
		.output()
		.unwrap_or_else(|error| panic!("fetch failed to start: {error}"));
	assert!(
		output.status.success(),
		"{}",
		String::from_utf8_lossy(&output.stderr)
	);
	let args =
		fs::read_to_string(capture).unwrap_or_else(|error| panic!("capture read failed: {error}"));
	assert!(args.contains("@solana-program/program-metadata@0.9.0"));
	assert!(args.contains("fetch idl"));
	assert!(args.contains("--raw"));
}
