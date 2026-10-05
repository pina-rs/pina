//! Real-runtime coverage for `pina rehearse`.
//!
//! An offline Surfpool plays the remote cluster: the counter example is
//! deployed there and exercised with real signed transactions, then the
//! cluster's clock jumps far enough ahead that every blockhash has expired.
//! `pina rehearse` forks that cluster with its own Surfpool and replays the
//! traffic against the deployed binary and three candidates: the same binary,
//! a counter whose increment adds two, and an unrelated program.
//!
//! The `surfpool` CI job builds every example, and the counter variant, into
//! `target/surfpool/examples` with `scripts/build-surfpool-examples.sh` before
//! running this test.
//!
//! The test proves no fork outlives its rehearsal without scanning processes
//! (CI's shell has no procps): `PINA_SURFPOOL` points at a wrapper that records
//! its pid and then becomes Surfpool.

#![cfg(all(unix, not(coverage)))]

mod support;

use std::ffi::OsString;
use std::fs;
use std::net::TcpListener;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::str::FromStr as _;
use std::time::Duration;
use std::time::Instant;

use base64::Engine as _;
use ed25519_dalek::Signer as _;
use ed25519_dalek::SigningKey;
use serde_json::Value;
use serde_json::json;
use solana_address::Address;

const COUNTER_PROGRAM_ID: &str = "GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const RPC_TIMEOUT: Duration = Duration::from_secs(30);
const READY_TIMEOUT: Duration = Duration::from_secs(60);

fn workspace_root() -> &'static Path {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.and_then(Path::parent)
		.unwrap_or_else(|| Path::new("."))
}

fn artifacts() -> PathBuf {
	std::env::var_os("SBF_OUT_DIR").map_or_else(
		|| workspace_root().join("target/surfpool/examples"),
		PathBuf::from,
	)
}

fn artifact(relative: &str) -> PathBuf {
	let path = artifacts().join(relative);
	assert!(
		path.is_file(),
		"missing {}; run scripts/build-surfpool-examples.sh first",
		path.display()
	);
	path
}

fn surfpool() -> OsString {
	std::env::var_os("PINA_SURFPOOL").unwrap_or_else(|| OsString::from("surfpool"))
}

fn free_port() -> u16 {
	TcpListener::bind("127.0.0.1:0")
		.and_then(|listener| listener.local_addr())
		.map_or_else(
			|error| panic!("allocate a local port: {error}"),
			|address| address.port(),
		)
}

/// An offline Surfpool killed when dropped, so a failing assertion never
/// leaves a validator behind.
struct RemoteCluster {
	child: Child,
	url: String,
	_directory: tempfile::TempDir,
}

impl RemoteCluster {
	fn start() -> Self {
		let directory =
			tempfile::tempdir().unwrap_or_else(|error| panic!("create surfpool dir: {error}"));
		let port = free_port();
		let child = Command::new(surfpool())
			.current_dir(directory.path())
			.args([
				"start",
				"--offline",
				"--no-tui",
				"--no-studio",
				"--no-deploy",
			])
			.args(["--airdrop-amount", "0", "--slot-time", "50"])
			.args(["--port", &port.to_string()])
			.args(["--ws-port", &free_port().to_string()])
			.args(["--studio-port", &free_port().to_string()])
			.arg("--log-path")
			.arg(directory.path().join("logs"))
			.stdin(Stdio::null())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.spawn()
			.unwrap_or_else(|error| panic!("start the remote surfpool: {error}"));
		let cluster = Self {
			child,
			url: format!("http://127.0.0.1:{port}"),
			_directory: directory,
		};
		let deadline = Instant::now() + READY_TIMEOUT;

		while try_rpc(&cluster.url, "getHealth", json!([])).is_err() {
			assert!(
				Instant::now() < deadline,
				"the remote surfpool was not ready within {READY_TIMEOUT:?}"
			);
			std::thread::sleep(Duration::from_millis(100));
		}

		cluster
	}
}

impl Drop for RemoteCluster {
	fn drop(&mut self) {
		let _ = self.child.kill();
		let _ = self.child.wait();
	}
}

fn try_rpc(url: &str, method: &str, params: Value) -> Result<Value, String> {
	let mut body = json!({ "jsonrpc": "2.0", "id": 1, "method": method });
	body["params"] = params;
	let mut response = ureq::post(url)
		.config()
		.timeout_global(Some(RPC_TIMEOUT))
		.build()
		.send_json(&body)
		.map_err(|error| format!("{method}: {error}"))?;
	let text = response
		.body_mut()
		.read_to_string()
		.map_err(|error| format!("{method}: {error}"))?;
	let value: Value =
		serde_json::from_str(&text).map_err(|error| format!("{method}: {error}: {text}"))?;

	if let Some(error) = value.get("error") {
		return Err(format!("{method}: {error}"));
	}

	Ok(value["result"].clone())
}

fn rpc(url: &str, method: &str, params: Value) -> Value {
	try_rpc(url, method, params).unwrap_or_else(|error| panic!("{error}"))
}

fn address(value: &str) -> Address {
	Address::from_str(value).unwrap_or_else(|error| panic!("parse address {value}: {error}"))
}

fn hex(bytes: &[u8]) -> String {
	use std::fmt::Write as _;

	bytes.iter().fold(
		String::with_capacity(bytes.len() * 2),
		|mut output, byte| {
			let _ = write!(output, "{byte:02x}");
			output
		},
	)
}

fn deploy(cluster: &RemoteCluster, program_id: &str, artifact: &Path) {
	let bytes =
		fs::read(artifact).unwrap_or_else(|error| panic!("read {}: {error}", artifact.display()));
	let mut offset = 0;

	for chunk in bytes.chunks(256 * 1024) {
		rpc(
			&cluster.url,
			"surfnet_writeProgram",
			json!([program_id, hex(chunk), offset, null]),
		);
		offset += chunk.len();
	}
}

struct AccountMeta {
	address: Address,
	writable: bool,
}

/// Encode and sign a legacy transaction whose only signer is the fee payer.
fn signed_transaction(
	payer: &SigningKey,
	program_id: &Address,
	accounts: &[AccountMeta],
	data: &[u8],
	blockhash: &[u8; 32],
) -> Vec<u8> {
	let payer_address = Address::new_from_array(payer.verifying_key().to_bytes());
	let mut keys = vec![payer_address];

	for meta in accounts {
		if !keys.contains(&meta.address) {
			keys.push(meta.address);
		}
	}

	keys.push(*program_id);
	// The legacy header describes writable keys first, then readonly ones.
	let is_writable = |key: &Address| {
		*key == payer_address
			|| accounts
				.iter()
				.any(|meta| meta.address == *key && meta.writable)
	};
	let (mut ordered, readonly): (Vec<Address>, Vec<Address>) =
		keys.into_iter().partition(|key| is_writable(key));
	let readonly_count = u8::try_from(readonly.len()).unwrap_or(u8::MAX);
	ordered.extend(readonly);
	let index = |key: &Address| -> u8 {
		ordered
			.iter()
			.position(|candidate| candidate == key)
			.and_then(|position| u8::try_from(position).ok())
			.unwrap_or_else(|| panic!("missing account key {key}"))
	};

	let mut message = vec![
		1,
		0,
		readonly_count,
		u8::try_from(ordered.len()).unwrap_or(u8::MAX),
	];
	for key in &ordered {
		message.extend_from_slice(key.as_ref());
	}
	message.extend_from_slice(blockhash);
	message.extend([
		1,
		index(program_id),
		u8::try_from(accounts.len()).unwrap_or(u8::MAX),
	]);
	message.extend(accounts.iter().map(|meta| index(&meta.address)));
	message.push(u8::try_from(data.len()).unwrap_or(u8::MAX));
	message.extend_from_slice(data);

	let mut transaction = vec![1];
	transaction.extend_from_slice(&payer.sign(&message).to_bytes());
	transaction.extend_from_slice(&message);
	transaction
}

fn latest_blockhash(cluster: &RemoteCluster) -> [u8; 32] {
	let value = rpc(&cluster.url, "getLatestBlockhash", json!([]));
	let encoded = value["value"]["blockhash"]
		.as_str()
		.unwrap_or_else(|| panic!("blockhash missing: {value}"));
	bs58::decode(encoded)
		.into_vec()
		.ok()
		.and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
		.unwrap_or_else(|| panic!("invalid blockhash {encoded}"))
}

/// Send a transaction and wait until the cluster confirms it.
fn send(cluster: &RemoteCluster, transaction: &[u8]) -> String {
	let encoded = base64::engine::general_purpose::STANDARD.encode(transaction);
	let signature = rpc(
		&cluster.url,
		"sendTransaction",
		json!([encoded, { "encoding": "base64" }]),
	);
	let signature = signature
		.as_str()
		.unwrap_or_else(|| panic!("signature missing: {signature}"))
		.to_owned();
	let deadline = Instant::now() + READY_TIMEOUT;

	while Instant::now() < deadline {
		let statuses = rpc(&cluster.url, "getSignatureStatuses", json!([[signature]]));
		let status = &statuses["value"][0];

		if status["confirmationStatus"] == "confirmed"
			|| status["confirmationStatus"] == "finalized"
		{
			assert!(status["err"].is_null(), "transaction failed: {status}");
			return signature;
		}

		std::thread::sleep(Duration::from_millis(50));
	}

	panic!("transaction {signature} was not confirmed");
}

/// Deploy the counter, initialize one counter, and increment it three times.
/// Returns the signatures oldest first.
fn counter_traffic(cluster: &RemoteCluster) -> Vec<String> {
	let program_id = address(COUNTER_PROGRAM_ID);
	deploy(cluster, COUNTER_PROGRAM_ID, &artifact("counter_program.so"));
	let authority = SigningKey::from_bytes(&[7; 32]);
	let authority_address = Address::new_from_array(authority.verifying_key().to_bytes());
	rpc(
		&cluster.url,
		"surfnet_setAccount",
		json!([authority_address.to_string(), { "lamports": 10_000_000_000_u64 }]),
	);
	let (counter, bump) =
		Address::find_program_address(&[b"counter", authority_address.as_ref()], &program_id);
	let initialize = signed_transaction(
		&authority,
		&program_id,
		&[
			AccountMeta {
				address: authority_address,
				writable: true,
			},
			AccountMeta {
				address: counter,
				writable: true,
			},
			AccountMeta {
				address: address(SYSTEM_PROGRAM_ID),
				writable: false,
			},
		],
		&[0, bump],
		&latest_blockhash(cluster),
	);
	let mut signatures = vec![send(cluster, &initialize)];

	for _ in 0..3 {
		let increment = signed_transaction(
			&authority,
			&program_id,
			&[
				AccountMeta {
					address: authority_address,
					writable: false,
				},
				AccountMeta {
					address: counter,
					writable: true,
				},
			],
			&[1],
			&latest_blockhash(cluster),
		);
		signatures.push(send(cluster, &increment));
		// A later blockhash keeps otherwise identical increments distinct.
		std::thread::sleep(Duration::from_millis(120));
	}

	// Age every transaction far past the recent-blockhash window, as real
	// traffic is by the time it is rehearsed.
	rpc(
		&cluster.url,
		"surfnet_timeTravel",
		json!([{ "absoluteSlot": 5000 }]),
	);

	signatures
}

/// A `PINA_SURFPOOL` wrapper that appends its pid to a file and then `exec`s
/// the real Surfpool, so every recorded pid is a fork `pina` started.
struct ForkRecorder {
	directory: tempfile::TempDir,
	executable: PathBuf,
}

impl ForkRecorder {
	fn new() -> Self {
		let directory =
			tempfile::tempdir().unwrap_or_else(|error| panic!("create recorder dir: {error}"));
		let executable = directory.path().join("surfpool");
		let pids = directory.path().join("pids");
		let real = surfpool();
		let real = real.to_string_lossy();
		assert!(
			!real.contains('\'') && !pids.to_string_lossy().contains('\''),
			"paths must not contain single quotes"
		);
		// The version probe exits at once; only `start` launches a fork.
		let script = format!(
			"#!/bin/sh\nif [ \"$1\" != \"--version\" ]; then\n\techo $$ >> '{}'\nfi\nexec '{real}' \"$@\"\n",
			pids.display()
		);
		// Written without this process opening it, so `exec` cannot hit
		// "Text file busy" from a descriptor a concurrent `fork` inherited.
		support::write_executable(&executable, &script)
			.unwrap_or_else(|error| panic!("write the surfpool recorder: {error}"));

		Self {
			directory,
			executable,
		}
	}

	/// Take the pids recorded since the last call.
	fn take(&self) -> Vec<String> {
		let pids = self.directory.path().join("pids");
		let recorded = fs::read_to_string(&pids).unwrap_or_default();
		let _ = fs::remove_file(&pids);

		recorded.lines().map(str::to_owned).collect()
	}

	/// Assert the rehearsal started exactly one fork and that it has exited.
	///
	/// `pina` reaps the fork's supervisor, which reaps the fork, before it
	/// exits, so a fork that is still running here was left behind.
	fn assert_forks_stopped(&self) {
		let pids = self.take();
		assert_eq!(pids.len(), 1, "a rehearsal starts one fork: {pids:?}");

		for pid in pids {
			// The shell's builtin `kill`, so the probe needs no extra package.
			let alive = Command::new("/bin/sh")
				.args(["-c", "kill -0 \"$1\" 2>/dev/null", "probe", &pid])
				.status()
				.unwrap_or_else(|error| panic!("probe Surfpool {pid}: {error}"));
			assert!(
				!alive.success(),
				"pina rehearse left its Surfpool fork {pid} running"
			);
		}
	}
}

fn rehearse(
	remote: &RemoteCluster,
	recorder: &ForkRecorder,
	candidate: &Path,
	extra: &[&str],
) -> Output {
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.arg("rehearse")
		.arg("--project")
		.arg(workspace_root().join("examples/counter_program"))
		.args(["--rpc-url", &remote.url])
		.arg("--program")
		.arg(candidate)
		.args(extra)
		.env("PINA_SURFPOOL", &recorder.executable)
		.output()
		.unwrap_or_else(|error| panic!("run pina rehearse: {error}"));
	println!(
		"$ pina rehearse --program {} {}",
		candidate.display(),
		extra.join(" ")
	);
	println!("{}", String::from_utf8_lossy(&output.stdout));
	eprintln!("{}", String::from_utf8_lossy(&output.stderr));
	recorder.assert_forks_stopped();
	output
}

fn report(output: &Output) -> Value {
	serde_json::from_slice(&output.stdout)
		.unwrap_or_else(|error| panic!("the JSON report must parse: {error}"))
}

#[test]
#[ignore = "starts real Surfpool instances; the surfpool CI job runs it after building examples"]
fn rehearse_detects_upgrade_behaviour_on_real_surfpool() {
	let remote = RemoteCluster::start();
	let recorder = ForkRecorder::new();
	let signatures = counter_traffic(&remote);
	let initialize = &signatures[0];

	// The deployed binary itself: nothing changes, and the initialize that
	// already ran fails identically in both runs.
	let same = rehearse(
		&remote,
		&recorder,
		&artifact("counter_program.so"),
		&["--json"],
	);
	assert_eq!(same.status.code(), Some(0));
	let same = report(&same);
	assert_eq!(same["schemaVersion"], 1);
	assert_eq!(same["deployedSha256"], same["candidateSha256"]);
	assert_eq!(
		same["summary"],
		json!({
			"total": 4, "unchanged": 3, "cuChanged": 0, "stateChanged": 0, "outcomeChanged": 0,
			"skipped": 1
		})
	);
	let skipped = same["transactions"]
		.as_array()
		.into_iter()
		.flatten()
		.find(|transaction| transaction["signature"] == *initialize)
		.unwrap_or_else(|| panic!("the initialize transaction is reported"));
	assert_eq!(skipped["skip"]["reason"], "failed_in_both");
	assert_eq!(skipped["instructions"][0]["name"], "initialize");

	// A counter whose increment adds two: every increment writes a different
	// count, which the report decodes from the IR.
	let variant = artifact("rehearse-variant/counter_program.so");
	let changed = rehearse(&remote, &recorder, &variant, &["--json"]);
	assert_eq!(changed.status.code(), Some(2));
	let changed = report(&changed);
	assert_ne!(changed["deployedSha256"], changed["candidateSha256"]);
	assert_eq!(changed["summary"]["stateChanged"], 3);
	let increment = changed["transactions"]
		.as_array()
		.into_iter()
		.flatten()
		.find(|transaction| transaction["status"] == "state_changed")
		.unwrap_or_else(|| panic!("an increment changed state"));
	assert_eq!(increment["instructions"][0]["name"], "increment");
	assert_eq!(increment["accounts"][0]["accountType"], "CounterState");
	assert_eq!(
		increment["accounts"][0]["fields"],
		json!([{ "name": "count", "baseline": "4", "candidate": "5" }])
	);

	// The text report of the same rehearsal, for humans.
	let text = rehearse(&remote, &recorder, &variant, &[]);
	assert_eq!(text.status.code(), Some(2));
	assert!(String::from_utf8_lossy(&text.stdout).contains("count: 4 -> 5"));

	// An unrelated program rejects every counter instruction.
	let foreign = artifact("hello_solana_program.so");
	let outcome = rehearse(&remote, &recorder, &foreign, &["--json"]);
	assert_eq!(outcome.status.code(), Some(2));
	assert_eq!(report(&outcome)["summary"]["outcomeChanged"], 4);
	let accepted = rehearse(&remote, &recorder, &foreign, &["--json", "--allow-changes"]);
	assert_eq!(accepted.status.code(), Some(0));

	// Named signatures replace the most recent ones.
	let named = rehearse(
		&remote,
		&recorder,
		&variant,
		&["--json", "--signature", &signatures[3]],
	);
	assert_eq!(named.status.code(), Some(2));
	assert_eq!(report(&named)["summary"]["total"], 1);
}
