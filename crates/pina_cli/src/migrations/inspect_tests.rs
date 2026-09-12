//! Inspect tests: envelope decoding, hop estimation, the RPC transport, and
//! the full report classification. Everything except the process exit is
//! exercised here so the command's logic stays at 100% coverage.

use std::io::Read as _;
use std::io::Write as _;

use base64::Engine as _;
use pina_abi::ContractHistory;
use pina_abi::ContractIdentity;
use pina_abi::ContractKind;
use pina_abi::DataSchema;
use pina_abi::FieldSchema;
use pina_abi::LayoutKind;
use pina_abi::MigrationManifest;
use pina_abi::MigrationVersionType;
use pina_abi::SchemaVersion;
use pina_abi::Transition;
use pina_abi::TransitionMode;

use super::*;
use crate::ir::DiscriminatorIr;

fn schema(fields: &[(&str, &str)]) -> DataSchema {
	DataSchema::try_new(
		LayoutKind::Fixed,
		fields
			.iter()
			.map(|(name, rust_type)| {
				FieldSchema {
					name: (*name).to_owned(),
					rust_type: (*rust_type).to_owned(),
				}
			})
			.collect(),
	)
	.unwrap_or_else(|error| panic!("valid test schema: {error}"))
}

/// A one-contract manifest: `State` with u8 discriminator 1, three versions
/// of growing payload size.
fn manifest() -> MigrationManifest {
	let identity = ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap();
	// Build the three-version ladder with valid adjacent transitions.
	let version = |index: u32, fields: &[(&str, &str)]| {
		SchemaVersion {
			version: index,
			schema_sha256: schema(fields).sha256(),
			schema: schema(fields),
			process: None,
			process_sha256: None,
			transition: None,
		}
	};
	let v0_fields: &[(&str, &str)] = &[("value", "u64")];
	let v1_fields: &[(&str, &str)] = &[("value", "u64"), ("extra", "u8")];
	let v2_fields: &[(&str, &str)] = &[("value", "u64"), ("extra", "u8"), ("more", "u16")];
	let v0 = version(0, v0_fields);
	let mut v1 = version(1, v1_fields);
	let mut v2 = version(2, v2_fields);
	let transition = |from: &SchemaVersion, to: &SchemaVersion| {
		Transition {
			from: from.version,
			to: to.version,
			mode: TransitionMode::Automatic,
			renames: vec![],
			source_schema_sha256: from.schema_sha256.clone(),
			destination_schema_sha256: to.schema_sha256.clone(),
			source_process_sha256: None,
			destination_process_sha256: None,
			process: None,
			implementation_sha256: Some("implementation".to_owned()),
		}
	};
	v1.transition = Some(transition(&v0, &v1));
	v2.transition = Some(transition(&v1, &v2));
	let mut manifest = MigrationManifest::new("program".to_owned(), MigrationVersionType::U8);
	manifest.contracts.insert(
		identity.key(),
		ContractHistory {
			identity,
			rust_name: "State".to_owned(),
			versions: vec![v0, v1, v2],
		},
	);
	manifest
}

/// Account bytes: [discriminator = 1, version, zeroed payload].
fn account_bytes(version: u8, payload_len: usize) -> Vec<u8> {
	let mut data = vec![1_u8, version];
	data.resize(2 + payload_len, 0);
	data
}

#[test]
fn decode_envelope_classifies_current_stale_future_and_unknown() {
	let manifest = manifest();

	let (history, stored, state) =
		decode_envelope(&manifest, &account_bytes(2, 42)).expect("current decodes");
	assert_eq!(history.rust_name, "State");
	assert_eq!(stored, 2);
	assert_eq!(state, InspectState::Current);

	let (history, stored, state) =
		decode_envelope(&manifest, &account_bytes(0, 42)).expect("stale decodes");
	assert_eq!(history.rust_name, "State");
	assert_eq!(stored, 0);
	assert_eq!(state, InspectState::Stale);

	let (history, stored, state) =
		decode_envelope(&manifest, &account_bytes(5, 42)).expect("future decodes");
	assert_eq!(history.rust_name, "State");
	assert_eq!(stored, 5);
	assert_eq!(state, InspectState::Future);

	// A foreign discriminator matches nothing.
	assert!(
		decode_envelope(&manifest, &{
			let mut data = account_bytes(2, 42);
			data[0] = 9;
			data
		})
		.is_none()
	);
	// Data too short to hold the envelope classifies as unknown.
	let (history, _, state) =
		decode_envelope(&manifest, &[1]).expect("short data maps to the contract");
	assert_eq!(state, InspectState::UnknownContract);
	let _ = history;
}

#[test]
fn pending_hops_quote_exact_sizes_and_rent() {
	let manifest = manifest();
	let history = manifest.contracts.values().next().expect("contract");
	let hops = pending_hops(history, 0, 1, 1);

	assert_eq!(hops.len(), 2);
	// Header is 2 bytes (1 discriminator + 1 version); payloads are 8, 9,
	// and 11 bytes across v0/v1/v2.
	assert_eq!(hops[0].from, 0);
	assert_eq!(hops[0].to, 1);
	assert_eq!(hops[0].byte_size_from, 10);
	assert_eq!(hops[0].byte_size_to, 11);
	assert_eq!(hops[0].rent_delta_lamports, 6_960);
	assert_eq!(hops[1].from, 1);
	assert_eq!(hops[1].to, 2);
	assert_eq!(hops[1].byte_size_to, 13);
	assert_eq!(hops[1].rent_delta_lamports, 6_960 * 2);

	assert!(pending_hops(history, 2, 1, 1).is_empty());
}

#[test]
fn build_report_classifies_every_state() {
	let manifest = manifest();

	let report = build_report("addr", "http://rpc", None, &manifest);
	assert!(!report.exists);
	assert_eq!(report.state, InspectState::Empty);

	let report = build_report("addr", "http://rpc", Some(vec![7, 7, 7]), &manifest);
	assert!(report.exists);
	assert_eq!(report.state, InspectState::UnknownContract);

	let report = build_report("addr", "http://rpc", Some(account_bytes(2, 11)), &manifest);
	assert_eq!(report.state, InspectState::Current);
	assert_eq!(report.contract.as_deref(), Some("account:1:01"));
	assert_eq!(report.current_version, Some(2));
	assert!(report.hops.is_empty());

	let report = build_report("addr", "http://rpc", Some(account_bytes(0, 8)), &manifest);
	assert_eq!(report.state, InspectState::Stale);
	assert_eq!(report.hops.len(), 2);

	let report = build_report("addr", "http://rpc", Some(account_bytes(9, 11)), &manifest);
	assert_eq!(report.state, InspectState::Future);
	assert_eq!(report.stored_version, Some(9));
}

#[test]
fn parse_rpc_account_data_handles_success_null_and_errors() {
	let address = solana_address::Address::from_str("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS")
		.expect("valid address");
	let encoded = base64::engine::general_purpose::STANDARD.encode([1, 2, 3]);
	let ok = serde_json::json!({
		"jsonrpc": "2.0",
		"result": {
			"context": { "slot": 1 },
			"value": { "data": [encoded, "base64"], "lamports": 1 }
		},
		"id": 1
	})
	.to_string();
	assert_eq!(
		parse_rpc_account_data(&ok, &address).expect("decodes"),
		Some(vec![1, 2, 3])
	);

	let null =
		"{\"jsonrpc\":\"2.0\",\"result\":{\"context\":{\"slot\":1},\"value\":null},\"id\":1}";
	assert_eq!(parse_rpc_account_data(null, &address).expect("null"), None);

	let rpc_error = "{\"jsonrpc\":\"2.0\",\"error\":{\"code\":-1,\"message\":\"boom\"},\"id\":1}";
	assert!(
		parse_rpc_account_data(rpc_error, &address)
			.expect_err("rpc error")
			.contains("boom")
	);

	assert!(parse_rpc_account_data("not json", &address).is_err());

	let missing = "{\"jsonrpc\":\"2.0\",\"result\":{},\"id\":1}";
	assert!(parse_rpc_account_data(missing, &address).is_err());

	let wrong_encoding =
		"{\"jsonrpc\":\"2.0\",\"result\":{\"value\":{\"data\":[\"AAA=\",\"base58\"]}},\"id\":1}";
	assert!(parse_rpc_account_data(wrong_encoding, &address).is_err());
}

/// Serve one canned HTTP response, run `fetch_account_data` against it, and
/// assert the transport round-trips a real JSON-RPC request.
#[test]
fn fetch_account_data_round_trips_through_http() {
	let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
	let port = listener.local_addr().expect("addr").port();
	let server = std::thread::spawn(move || {
		let (mut stream, _) = listener.accept().expect("accept");
		let mut buffer = [0_u8; 2048];
		let _ = stream.read(&mut buffer);
		let body = serde_json::json!({
			"jsonrpc": "2.0",
			"result": { "value": { "data": ["AQJB", "base64"] } },
			"id": 1
		})
		.to_string();
		let response = format!(
			"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: \
			 {}\r\nconnection: close\r\n\r\n{body}",
			body.len()
		);
		stream
			.write_all(response.as_bytes())
			.expect("write response");
	});

	use std::str::FromStr as _;
	let address = solana_address::Address::from_str("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS")
		.expect("valid address");
	let data = fetch_account_data(&format!("http://127.0.0.1:{port}"), &address)
		.expect("fetch over local http");
	assert_eq!(data, Some(vec![1, 2, 65]));
	server.join().expect("server thread");
}

#[test]
fn fetch_account_data_reports_transport_failures() {
	use std::str::FromStr as _;
	let address = solana_address::Address::from_str("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS")
		.expect("valid address");
	// Port 1 on localhost is not listening in CI; ureq fails closed.
	let error = fetch_account_data("http://127.0.0.1:1", &address).expect_err("transport error");
	assert!(!error.is_empty());
}

#[test]
fn run_inspect_wires_discovery_manifest_fetch_and_exit_codes() {
	// Full flow against a fixture project and a local RPC endpoint.
	let temp = tempfile::TempDir::new().expect("temp dir");
	let root = std::fs::canonicalize(temp.path()).expect("canonical");
	std::fs::create_dir_all(root.join("src")).expect("src");
	std::fs::write(
		root.join("Cargo.toml"),
		"[package]\nname = \"inspect_fixture\"\nversion = \"0.0.0\"\nedition = \
		 \"2024\"\n[lib]\npath = \"src/lib.rs\"\n",
	)
	.expect("cargo manifest");
	std::fs::write(
		root.join("src/lib.rs"),
		"use pina::*;\ndeclare_id!(\"GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS\");\n#\
		 [discriminator]\nenum Kind { State = 1 }\n#[account(discriminator = Kind::State, \
		 migrations)]\nstruct State { value: u64 }\n",
	)
	.expect("source");

	let manifest = manifest();
	std::fs::create_dir_all(root.join("migrations")).expect("migrations dir");
	std::fs::write(
		root.join(pina_abi::MANIFEST_PATH),
		serde_json::to_vec_pretty(&manifest).expect("serialize"),
	)
	.expect("manifest");

	let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
	let port = listener.local_addr().expect("addr").port();
	let server = std::thread::spawn(move || {
		let (mut stream, _) = listener.accept().expect("accept");
		let mut buffer = [0_u8; 2048];
		let _ = stream.read(&mut buffer);
		let encoded = base64::engine::general_purpose::STANDARD.encode(account_bytes_for_server());
		let body = serde_json::json!({
			"jsonrpc": "2.0",
			"result": { "value": { "data": [encoded, "base64"] } },
			"id": 1
		})
		.to_string();
		let response = format!(
			"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: \
			 {}\r\nconnection: close\r\n\r\n{body}",
			body.len()
		);
		stream.write_all(response.as_bytes()).expect("write");
	});

	let (report, exit) = match run_inspect(
		&root,
		"GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS",
		&format!("http://127.0.0.1:{port}"),
	) {
		Ok(outcome) => outcome,
		Err(error) => panic!("inspect runs: {error:?}"),
	};
	server.join().expect("server thread");
	assert_eq!(report.state, InspectState::Stale);
	assert_eq!(exit.code, 1);

	// Invalid addresses fail before any network activity.
	let error =
		run_inspect(&root, "not-an-address", "http://127.0.0.1:1").expect_err("invalid address");
	assert!(matches!(error, InspectError::InvalidAddress { .. }));

	// A project without a manifest fails at the manifest step.
	let empty = tempfile::TempDir::new().expect("temp dir");
	let error = run_inspect(
		empty.path(),
		"GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS",
		"http://127.0.0.1:1",
	)
	.expect_err("no project");
	assert!(matches!(error, InspectError::Project(_)));
}

fn account_bytes_for_server() -> Vec<u8> {
	let mut data = vec![1_u8, 0];
	data.resize(10, 0);
	data
}

#[test]
fn discriminator_hex_decode_rejects_bad_lengths() {
	let mut out = [0_u8; 2];
	assert!(base16_decode("ff", &mut out).is_err());
	assert!(base16_decode("zzzz", &mut out).is_err());
	assert!(base16_decode("aabb", &mut out).is_ok());
	assert_eq!(out, [0xaa, 0xbb]);
}

// Keep the unused-import lint honest in non-test builds of this module's
// siblings that share the discriminator type.
#[allow(unused)]
fn discriminator_shape_marker(_: DiscriminatorIr) {}
