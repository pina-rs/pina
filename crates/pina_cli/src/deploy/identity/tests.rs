use super::*;

#[path = "../../../tests/support/rehearse.rs"]
mod fakes;

const PRIVATE_GENESIS: &str = "11111111111111111111111111111111";

fn endpoint(genesis: Value, version: Value) -> fakes::FakeRpcServer {
	fakes::FakeRpcServer::start(move |method, _| {
		match method {
			"getGenesisHash" => fakes::Reply::Result(genesis.clone()),
			"getVersion" => fakes::Reply::Result(version.clone()),
			_ => fakes::Reply::Error(-32601, format!("method {method} not found")),
		}
	})
}

fn agave_version() -> Value {
	json!({ "solana-core": "3.0.0", "feature-set": 1 })
}

#[test]
fn reads_the_genesis_hash_and_whether_the_endpoint_is_surfpool() {
	let validator = endpoint(json!(PRIVATE_GENESIS), agave_version());
	assert_eq!(
		probe(&validator.url),
		Ok(Identity {
			genesis_hash: PRIVATE_GENESIS.to_owned(),
			surfpool: false,
		})
	);
	assert_eq!(validator.methods(), ["getGenesisHash", "getVersion"]);

	let surfnet = endpoint(
		json!(MAINNET_GENESIS),
		json!({ "surfnet-version": "1.7.0", "solana-core": "3.0.0" }),
	);
	assert_eq!(
		probe(&surfnet.url),
		Ok(Identity {
			genesis_hash: MAINNET_GENESIS.to_owned(),
			surfpool: true,
		})
	);
}

#[test]
fn names_only_the_public_clusters_an_agave_endpoint_reports() {
	let identity = |genesis_hash: &str, surfpool| {
		Identity {
			genesis_hash: genesis_hash.to_owned(),
			surfpool,
		}
		.public_cluster()
	};

	assert_eq!(identity(DEVNET_GENESIS, false), Some(Cluster::Devnet));
	assert_eq!(identity(TESTNET_GENESIS, false), Some(Cluster::Testnet));
	assert_eq!(identity(MAINNET_GENESIS, false), Some(Cluster::MainnetBeta));
	assert_eq!(identity(PRIVATE_GENESIS, false), None);

	for genesis in [DEVNET_GENESIS, TESTNET_GENESIS, MAINNET_GENESIS] {
		assert_eq!(identity(genesis, true), None);
	}
}

#[test]
fn an_endpoint_that_cannot_prove_its_identity_is_unidentified() {
	for (genesis, version) in [
		(json!("not base58!"), agave_version()),
		(json!("1111"), agave_version()),
		(json!(42), agave_version()),
	] {
		let server = endpoint(genesis, version);
		assert_eq!(
			probe(&server.url),
			Err("getGenesisHash returned an invalid hash".to_owned())
		);
	}

	let no_version = fakes::FakeRpcServer::start(|method, _| {
		match method {
			"getGenesisHash" => fakes::Reply::Result(json!(PRIVATE_GENESIS)),
			_ => fakes::Reply::Error(-32601, format!("method {method} not found")),
		}
	});
	let error = probe(&no_version.url).unwrap_err();
	assert!(error.starts_with("getVersion "), "{error}");

	let closed = std::net::TcpListener::bind("127.0.0.1:0")
		.and_then(|listener| listener.local_addr())
		.unwrap_or_else(|error| panic!("reserve a closed port: {error}"));
	let error = probe(&format!("http://{closed}")).unwrap_err();
	assert!(error.starts_with("getGenesisHash "), "{error}");
}
