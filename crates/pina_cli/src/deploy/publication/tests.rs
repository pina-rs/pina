use std::fs;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use base64::Engine as _;
use ed25519_dalek::SigningKey;

use super::*;
use crate::deploy::InputFingerprint;
use crate::deploy::ResolvedTarget;

#[path = "../../../tests/support/rehearse.rs"]
mod fakes;

fn fixture() -> (tempfile::TempDir, DeploymentPlan, Value) {
	let temp = tempfile::tempdir().unwrap();
	let program_path = temp.path().join("program.so");
	let authority_path = temp.path().join("authority.json");
	let key = SigningKey::from_bytes(&[5; 32]);
	let mut key_bytes = key.to_bytes().to_vec();
	key_bytes.extend_from_slice(&key.verifying_key().to_bytes());
	fs::write(&program_path, b"SBF").unwrap();
	fs::write(&authority_path, serde_json::to_vec(&key_bytes).unwrap()).unwrap();
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt as _;
		fs::set_permissions(&authority_path, fs::Permissions::from_mode(0o600)).unwrap();
	}
	let program = Address::new_from_array([3; 32]);
	let (programdata, _) =
		Address::find_program_address(&[program.as_ref()], &Address::from_str_const(LOADER));
	let mut program_bytes = 2_u32.to_le_bytes().to_vec();
	program_bytes.extend_from_slice(programdata.as_ref());
	let mut data_bytes = 3_u32.to_le_bytes().to_vec();
	data_bytes.extend_from_slice(&41_u64.to_le_bytes());
	data_bytes.push(1);
	data_bytes.extend_from_slice(&key.verifying_key().to_bytes());
	data_bytes.extend_from_slice(b"SBF\0\0");
	let account = |bytes: Vec<u8>, executable| {
		json!({
			"lamports": 1, "owner": LOADER, "executable": executable,
			"data": [base64::engine::general_purpose::STANDARD.encode(bytes), "base64"]
		})
	};
	let accounts = json!({ "context": { "slot": 42 }, "value": [
		account(program_bytes, true), account(data_bytes, false)
	] });
	let plan = DeploymentPlan {
		project_root: temp.path().to_str().unwrap().to_owned(),
		program_dir: String::new(),
		library_name: String::new(),
		program: program_path.to_str().unwrap().to_owned(),
		program_keypair: None,
		upgrade_authority: authority_path.to_str().unwrap().to_owned(),
		payer: String::new(),
		program_id: program.to_string(),
		target: ResolvedTarget::resolve(&super::super::DeploymentTarget::from_cluster_arg(
			"localnet",
		))
		.unwrap(),
		remote_command: None,
		input_fingerprint: InputFingerprint {
			program: Sha256::digest(b"SBF").into(),
			program_keypair: None,
			upgrade_authority: Sha256::digest(fs::read(&authority_path).unwrap()).into(),
			payer: [0; 32],
		},
		warnings: Vec::new(),
	};

	(temp, plan, accounts)
}

fn server(accounts: Value) -> fakes::FakeRpcServer {
	fakes::FakeRpcServer::start(move |method, params| {
		match method {
			"getGenesisHash" => fakes::Reply::Result(json!("11111111111111111111111111111111")),
			"getMultipleAccounts" => {
				assert_eq!(
					params[1],
					json!({ "encoding": "base64", "commitment": "finalized" })
				);
				fakes::Reply::Result(accounts.clone())
			}
			_ => panic!("unexpected method: {method}"),
		}
	})
}

#[test]
fn verifies_finalized_program_bytes_with_zero_padding() {
	let (_temp, mut plan, mut accounts) = fixture();

	for slot in [41, 42] {
		accounts["context"]["slot"] = json!(slot);
		let server = server(accounts.clone());
		plan.target.rpc_url.clone_from(&server.url);
		plan.verify_publication().unwrap();
		assert_eq!(
			server.methods(),
			["getGenesisHash", "getMultipleAccounts", "getGenesisHash"]
		);
	}
	assert!(!PublicationUnverified("unverified").is_unsent());
}

#[test]
fn refuses_missing_malformed_foreign_or_unfinalized_program_state() {
	let (_temp, mut plan, approved) = fixture();
	let mut cases = vec![
		json!({}),
		json!({ "context": { "slot": 42 }, "value": [null, null] }),
	];

	for (account, field, value) in [
		(0, "owner", json!("11111111111111111111111111111111")),
		(0, "executable", json!(false)),
		(0, "data", json!(["%%%", "base64"])),
		(1, "owner", json!("11111111111111111111111111111111")),
		(1, "executable", json!(true)),
	] {
		let mut accounts = approved.clone();
		accounts["value"][account][field] = value;
		cases.push(accounts);
	}

	for (account, offset) in [(0, 0), (0, 4), (1, 0), (1, 12), (1, 13), (1, 45), (1, 49)] {
		let mut accounts = approved.clone();
		let data = &mut accounts["value"][account]["data"][0];
		let mut bytes = base64::engine::general_purpose::STANDARD
			.decode(data.as_str().unwrap())
			.unwrap();
		bytes[offset] ^= 1;
		*data = json!(base64::engine::general_purpose::STANDARD.encode(bytes));
		cases.push(accounts);
	}

	for (account, len) in [(0, 35), (1, 44), (1, 46)] {
		let mut accounts = approved.clone();
		let data = &mut accounts["value"][account]["data"][0];
		let mut bytes = base64::engine::general_purpose::STANDARD
			.decode(data.as_str().unwrap())
			.unwrap();
		bytes.truncate(len);
		*data = json!(base64::engine::general_purpose::STANDARD.encode(bytes));
		cases.push(accounts);
	}
	let mut inactive = approved;
	inactive["context"]["slot"] = json!(40);
	cases.push(inactive);

	for accounts in cases {
		let server = server(accounts.clone());
		plan.target.rpc_url.clone_from(&server.url);
		assert!(plan.verify_publication().is_err(), "accepted {accounts}");
	}
}

#[test]
fn refuses_rpc_errors_and_cluster_changes_without_retrying() {
	let (_temp, mut plan, accounts) = fixture();

	for failure in [
		"transport",
		"redirect",
		"malformed genesis",
		"accounts",
		"changed cluster",
	] {
		let accounts = accounts.clone();
		let calls = AtomicUsize::new(0);
		let server = fakes::FakeRpcServer::start(move |method, _| {
			let call = calls.fetch_add(1, Ordering::SeqCst);

			match (failure, method) {
				("transport", _) => fakes::Reply::Status(503),
				("redirect", _) => fakes::Reply::Status(302),
				("malformed genesis", _) => fakes::Reply::Result(json!("invalid")),
				("accounts", "getMultipleAccounts") => {
					fakes::Reply::Error(-32005, "unavailable".to_owned())
				}
				("changed cluster", "getGenesisHash") if call > 0 => {
					fakes::Reply::Result(json!(bs58::encode([2; 32]).into_string()))
				}
				(_, "getGenesisHash") => {
					fakes::Reply::Result(json!("11111111111111111111111111111111"))
				}
				_ => fakes::Reply::Result(accounts.clone()),
			}
		});
		plan.target.rpc_url.clone_from(&server.url);
		assert!(plan.verify_publication().is_err(), "accepted {failure}");
		assert!(server.methods().len() <= 3);
	}
}

#[test]
fn pins_named_cluster_genesis_and_rejects_unreadable_inputs() {
	for (cluster, expected) in [
		(
			Cluster::Devnet,
			"EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG",
		),
		(
			Cluster::Testnet,
			"4uhcVJyU9pJkvQyS88uRDiswHXSCkY3zQawwpjk2NsNY",
		),
		(
			Cluster::MainnetBeta,
			"5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
		),
	] {
		verify_genesis(Some(cluster), &json!(expected)).unwrap();
		assert!(verify_genesis(Some(cluster), &json!("11111111111111111111111111111111")).is_err());
	}
	verify_genesis(None, &json!("11111111111111111111111111111111")).unwrap();
	let (_temp, mut plan, _) = fixture();
	fs::write(plan.program(), b"changed").unwrap();
	assert!(plan.verify_publication().is_err());
	fs::write(plan.program(), b"SBF").unwrap();
	plan.input_fingerprint.upgrade_authority = [0; 32];
	assert!(plan.verify_publication().is_err());
	fs::remove_file(plan.program()).unwrap();
	assert!(plan.verify_publication().is_err());
	plan.program_id = "invalid".to_owned();
	assert!(plan.verify_publication().is_err());
}
