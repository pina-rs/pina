//! `pina explain` against transactions captured from a real Surfnet.
//!
//! Each fixture under `tests/fixtures/explain` is the unmodified
//! `getTransaction` result of a transaction sent to the `validation_program`
//! example with preflight disabled, so the account flags, errors, and logs are
//! the runtime's own.

use std::path::Path;
use std::process::Command;
use std::process::Output;

const FIXTURES: &[&str] = &[
	"custom_error",
	"duplicate_mutable",
	"missing_signer",
	"not_enough_accounts",
	"other_program",
	"readonly_mut_field",
	"readonly_writable_rule",
	"succeeded",
	"too_many_accounts",
	"unknown_discriminator",
	"wrong_owner",
];

const SIGNATURE: &str =
	"63T6XGyihfHKThEZFe36L5LkmAqJ8uVaqzQ6Po4ovSZPzCCDKk5h8EJGHdyQFP5sbbDwbW2csgfXWbWZWrXDjD7f";

fn workspace_root() -> &'static Path {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.and_then(Path::parent)
		.unwrap_or_else(|| Path::new("."))
}

fn explain(args: &[&str]) -> Output {
	Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(workspace_root())
		.arg("explain")
		.args(args)
		.args(["--project", "examples/validation_program"])
		.output()
		.unwrap_or_else(|error| panic!("pina explain failed to launch: {error}"))
}

fn fixture(name: &str) -> String {
	format!("crates/pina_cli/tests/fixtures/explain/{name}.json")
}

fn stdout(output: &Output) -> String {
	String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
	String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn explains_every_captured_transaction() {
	for name in FIXTURES {
		let file = fixture(name);
		let text = explain(&["--transaction-file", &file]);
		assert_eq!(text.status.code(), Some(0), "{name}: {}", stderr(&text));
		insta::assert_snapshot!(format!("explain_{name}"), stdout(&text));

		let json = explain(&["--transaction-file", &file, "--json"]);
		assert_eq!(json.status.code(), Some(0), "{name}: {}", stderr(&json));
		let document: serde_json::Value = serde_json::from_slice(&json.stdout)
			.unwrap_or_else(|error| panic!("{name}: invalid JSON: {error}"));
		assert_eq!(document["schemaVersion"], 1);
		insta::assert_json_snapshot!(format!("explain_{name}_json"), document);
	}
}

#[test]
fn a_file_with_an_unreachable_endpoint_still_explains() {
	let closed = std::net::TcpListener::bind("127.0.0.1:0")
		.unwrap_or_else(|error| panic!("failed to reserve a port: {error}"));
	let url = format!(
		"http://127.0.0.1:{}",
		closed
			.local_addr()
			.unwrap_or_else(|error| panic!("failed to read the port: {error}"))
			.port()
	);
	drop(closed);

	let output = explain(&[
		"--transaction-file",
		&fixture("readonly_writable_rule"),
		"--rpc-url",
		&url,
	]);
	let text = stdout(&output);

	assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
	assert!(text.contains("audit: writable [confirmed, matches the program log]"));
	assert!(text.contains(
		"Current account state is unavailable: getMultipleAccounts on the custom RPC endpoint failed"
	));
	assert!(!text.contains("127.0.0.1"));
}

#[test]
fn rejects_invalid_inputs_with_exit_code_one() {
	for args in [
		vec!["not-a-signature"],
		vec!["not-a-signature", "--network", "devnet"],
	] {
		let output = explain(&args);
		assert_eq!(output.status.code(), Some(1), "{args:?}");
		assert!(stderr(&output).contains("invalid transaction signature `not-a-signature`"));
		assert!(stdout(&output).is_empty());
	}

	let output = explain(&[
		SIGNATURE,
		"--rpc-url",
		"https://user:secret@rpc.example.com",
	]);
	assert_eq!(output.status.code(), Some(1));
	assert!(stderr(&output).contains("unsafe RPC URL: embedded credentials are not accepted"));
	assert!(!stderr(&output).contains("secret"));

	let output = explain(&[
		"--transaction-file",
		"crates/pina_cli/tests/fixtures/explain/missing.json",
	]);
	assert_eq!(output.status.code(), Some(1));
	assert!(
		stderr(&output)
			.contains("could not read crates/pina_cli/tests/fixtures/explain/missing.json")
	);
}

#[test]
fn requires_exactly_one_transaction_source() {
	let neither = explain(&[]);
	assert_eq!(neither.status.code(), Some(2));
	assert!(stderr(&neither).contains("--transaction-file"));

	let both = explain(&[SIGNATURE, "--transaction-file", &fixture("missing_signer")]);
	assert_eq!(both.status.code(), Some(2));
	assert!(stderr(&both).contains("cannot be used with"));

	let networks = explain(&[
		SIGNATURE,
		"--network",
		"devnet",
		"--rpc-url",
		"http://127.0.0.1:8899",
	]);
	assert_eq!(networks.status.code(), Some(2));
}
