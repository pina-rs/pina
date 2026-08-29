#![cfg(unix)]

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;

use base64::Engine;
use ed25519_dalek::SigningKey;
use sha2::Digest;
use sha2::Sha256;
use tempfile::TempDir;

const PROGRAM_ID: &str = "11111111111111111111111111111111";
const MATCHING_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn fake_verifier(temp: &TempDir) -> std::path::PathBuf {
	let path = temp.path().join("solana verify with spaces");
	let script = r#"#!/bin/sh
set -eu
if [ "${1:-}" = "--version" ]; then
  if IFS= read -r unexpected; then
    printf '%s\n' 'verifier consumed inherited stdin' >&2
    exit 91
  fi
  printf '%s\n' 'solana-verify 0.5.1'
  exit 0
fi
case " $* " in
  *" get-executable-hash "*)
    printf '%s\n' 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
    ;;
  *" get-program-hash "*)
    printf '%s\n' "${PINA_FAKE_DEPLOYED_HASH:-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa}"
    ;;
  *" verify-from-repo "*)
    printf '%s\n' 'Program uploaded successfully.'
    ;;
  *" export-pda-tx "*)
    printf '%s\n' 'Building repository'
    printf '%s\n' "$PINA_FAKE_EXPORT_PAYLOAD"
    ;;
  *" remote submit-job "*)
    printf '%s\n' 'job submitted'
    ;;
  *" remote get-status "*)
    if [ "${PINA_FAKE_SIGNAL:-0}" = 1 ]; then
      kill -TERM $$
    fi
    printf '%s\n' 'verified'
    ;;
  *)
    printf '%s\n' 'unexpected fake invocation' >&2
    exit 9
    ;;
esac
"#;
	fs::write(&path, script).unwrap();
	fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
	path
}

fn create_record_and_keypair(temp: &TempDir) -> (std::path::PathBuf, std::path::PathBuf, String) {
	let bytes = vec![7_u8; 128];
	let hash = Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect::<String>();
	let record = temp.path().join(format!("fixture-{hash}.json"));
	let artifact = record.with_extension("so");
	let json = serde_json::json!({
		"schemaVersion": 1,
		"packageName": "fixture",
		"libraryName": "fixture",
		"executableHash": hash,
		"solanaVerifyVersion": "0.5.1",
		"build": {
			"mountPath": ".",
			"workspacePath": ".",
			"programPath": "programs/fixture",
			"libraryName": "fixture",
			"features": ["bpf-entrypoint"],
			"defaultFeatures": true,
			"cargoLockSha256": "a".repeat(64),
		},
		"source": {
			"repository": "https://github.com/pina-rs/pina",
			"revision": "0123456789abcdef0123456789abcdef01234567",
			"dirty": false,
		},
		"diagnostics": [],
	});
	fs::write(&artifact, bytes).unwrap();
	fs::write(&record, serde_json::to_vec_pretty(&json).unwrap()).unwrap();

	let keypair = temp.path().join("authority keypair.json");
	let signing_key = SigningKey::from_bytes(&[3_u8; 32]);
	let public = signing_key.verifying_key().to_bytes();
	let keypair_bytes = [signing_key.to_bytes().as_slice(), public.as_slice()].concat();
	fs::write(&keypair, serde_json::to_vec(&keypair_bytes).unwrap()).unwrap();
	fs::set_permissions(&keypair, fs::Permissions::from_mode(0o600)).unwrap();

	(record, keypair, hash)
}

fn run_check(verifier: &Path, program: &Path, deployed_hash: &str) -> std::process::Output {
	Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"verify",
			"--solana-verify",
			verifier.to_str().unwrap(),
			"check",
			"--program-id",
			PROGRAM_ID,
			"--cluster",
			"devnet",
			"--program",
			program.to_str().unwrap(),
		])
		.env("PINA_FAKE_DEPLOYED_HASH", deployed_hash)
		.output()
		.unwrap()
}

#[test]
fn check_uses_documented_zero_two_one_exit_codes() {
	let temp = TempDir::new().unwrap();
	let verifier = fake_verifier(&temp);
	let program = temp.path().join("program with spaces.so");
	fs::write(&program, b"program").unwrap();

	let matching = run_check(&verifier, &program, MATCHING_HASH);
	assert_eq!(matching.status.code(), Some(0));
	assert!(String::from_utf8_lossy(&matching.stdout).contains("matches"));

	let mismatch = run_check(&verifier, &program, &"b".repeat(64));
	assert_eq!(mismatch.status.code(), Some(2));
	assert!(String::from_utf8_lossy(&mismatch.stderr).contains("differ"));

	let malformed = run_check(&verifier, &program, "not-a-hash");
	assert_eq!(malformed.status.code(), Some(1));
	assert!(String::from_utf8_lossy(&malformed.stderr).contains("invalid executable hash"));
}

#[test]
fn verifier_children_never_inherit_operator_input() {
	let temp = TempDir::new().unwrap();
	let verifier = fake_verifier(&temp);
	let program = temp.path().join("program.so");
	fs::write(&program, b"program").unwrap();
	let mut child = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"verify",
			"--solana-verify",
			verifier.to_str().unwrap(),
			"check",
			"--program-id",
			PROGRAM_ID,
			"--cluster",
			"devnet",
			"--program",
			program.to_str().unwrap(),
		])
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.unwrap();
	child
		.stdin
		.take()
		.unwrap()
		.write_all(b"operator input must stay with Pina\n")
		.unwrap();
	let output = child.wait_with_output().unwrap();

	assert_eq!(output.status.code(), Some(0));
	assert!(!String::from_utf8_lossy(&output.stderr).contains("consumed inherited stdin"));
}

#[test]
fn non_interactive_record_refuses_before_spawning_verifier() {
	let temp = TempDir::new().unwrap();
	let verifier = temp.path().join("must not run");
	fs::write(
		&verifier,
		"#!/bin/sh\ntouch \"$PINA_SPAWN_MARKER\"\nexit 99\n",
	)
	.unwrap();
	fs::set_permissions(&verifier, fs::Permissions::from_mode(0o755)).unwrap();
	let marker = temp.path().join("spawned");
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"verify",
			"--solana-verify",
			verifier.to_str().unwrap(),
			"record",
			"--program-id",
			PROGRAM_ID,
			"--cluster",
			"devnet",
			"--build-record",
			"missing.json",
			"--authority",
			"missing-keypair.json",
		])
		.env("PINA_SPAWN_MARKER", &marker)
		.stdin(Stdio::null())
		.output()
		.unwrap();

	assert_eq!(output.status.code(), Some(1));
	assert!(!marker.exists());
	assert!(String::from_utf8_lossy(&output.stderr).contains("requires confirmation"));
}

#[test]
fn record_export_submit_and_status_use_the_real_process_adapter() {
	let temp = TempDir::new().unwrap();
	let verifier = fake_verifier(&temp);
	let (record, keypair, hash) = create_record_and_keypair(&temp);
	let common = ["verify", "--solana-verify", verifier.to_str().unwrap()];
	let recorded = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(common)
		.args([
			"record",
			"--program-id",
			PROGRAM_ID,
			"--cluster",
			"devnet",
			"--build-record",
			record.to_str().unwrap(),
			"--authority",
			keypair.to_str().unwrap(),
			"--yes",
		])
		.env("PINA_FAKE_DEPLOYED_HASH", &hash)
		.output()
		.unwrap();
	assert_eq!(recorded.status.code(), Some(0));
	assert!(String::from_utf8_lossy(&recorded.stdout).contains("uploaded successfully"));

	let exported_path = temp.path().join("export\n\u{1b}[31m.tx");
	let payload = base64::engine::general_purpose::STANDARD.encode([9_u8; 128]);
	let exported = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(common)
		.args([
			"record",
			"--program-id",
			PROGRAM_ID,
			"--cluster",
			"mainnet-beta",
			"--build-record",
			record.to_str().unwrap(),
			"--export",
			"SysvarRent111111111111111111111111111111111",
			"--output",
			exported_path.to_str().unwrap(),
			"--export-encoding",
			"base64",
		])
		.env("PINA_FAKE_DEPLOYED_HASH", &hash)
		.env("PINA_FAKE_EXPORT_PAYLOAD", &payload)
		.output()
		.unwrap();
	assert_eq!(exported.status.code(), Some(0));
	assert_eq!(fs::read_to_string(&exported_path).unwrap(), payload);
	let summary = String::from_utf8_lossy(&exported.stdout);
	assert!(summary.contains("export\\n\\u{1b}[31m.tx"));
	assert!(!summary.contains("export\n\u{1b}[31m.tx"));

	for (command, expected) in [("submit", "job submitted"), ("status", "verified")] {
		let mut process = Command::new(env!("CARGO_BIN_EXE_pina"));
		process
			.args(common)
			.arg(command)
			.args(["--program-id", PROGRAM_ID]);
		if command == "submit" {
			process.args(["--uploader", "SysvarRent111111111111111111111111111111111"]);
		}
		let output = process.output().unwrap();
		assert_eq!(output.status.code(), Some(0));
		assert!(String::from_utf8_lossy(&output.stdout).contains(expected));
	}
}

#[test]
fn missing_and_signaled_verifier_processes_are_errors() {
	let temp = TempDir::new().unwrap();
	let missing = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"verify",
			"--solana-verify",
			temp.path().join("missing").to_str().unwrap(),
			"status",
			"--program-id",
			PROGRAM_ID,
		])
		.output()
		.unwrap();
	assert_eq!(missing.status.code(), Some(1));
	assert!(String::from_utf8_lossy(&missing.stderr).contains("could not run"));

	let verifier = fake_verifier(&temp);
	let signaled = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"verify",
			"--solana-verify",
			verifier.to_str().unwrap(),
			"status",
			"--program-id",
			PROGRAM_ID,
		])
		.env("PINA_FAKE_SIGNAL", "1")
		.output()
		.unwrap();
	assert_eq!(signaled.status.code(), Some(1));
	assert!(String::from_utf8_lossy(&signaled.stderr).contains("signal"));
}
