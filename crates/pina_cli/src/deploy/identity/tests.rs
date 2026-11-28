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
			surfpool: Ok(false),
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
			surfpool: Ok(true),
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
	};
	let cluster = |genesis_hash, surfpool| {
		identity(genesis_hash, Ok(surfpool))
			.public_cluster()
			.map_err(str::to_owned)
	};

	assert_eq!(cluster(DEVNET_GENESIS, false), Ok(Some(Cluster::Devnet)));
	assert_eq!(cluster(TESTNET_GENESIS, false), Ok(Some(Cluster::Testnet)));
	assert_eq!(
		cluster(MAINNET_GENESIS, false),
		Ok(Some(Cluster::MainnetBeta))
	);
	assert_eq!(cluster(PRIVATE_GENESIS, false), Ok(None));

	for genesis in [DEVNET_GENESIS, TESTNET_GENESIS, MAINNET_GENESIS] {
		assert_eq!(cluster(genesis, true), Ok(None));
	}

	// Without a `getVersion` answer, even a private hash cannot be classified.
	let unversioned = identity(PRIVATE_GENESIS, Err("getVersion timed out".to_owned()));
	assert_eq!(unversioned.public_cluster(), Err("getVersion timed out"));
}

#[test]
fn an_endpoint_matches_only_the_answers_it_gave_at_planning() {
	let identity = |genesis_hash: &str, surfpool: Result<bool, &str>| {
		Identity {
			genesis_hash: genesis_hash.to_owned(),
			surfpool: surfpool.map_err(str::to_owned),
		}
	};
	let surfnet = identity(MAINNET_GENESIS, Ok(true));

	assert!(surfnet.matches(&identity(MAINNET_GENESIS, Ok(true))));
	assert!(!surfnet.matches(&identity(MAINNET_GENESIS, Ok(false))));
	assert!(!surfnet.matches(&identity(MAINNET_GENESIS, Err("timed out"))));
	assert!(!surfnet.matches(&identity(DEVNET_GENESIS, Ok(true))));

	// A plan that never read the version pinned only the hash.
	let unversioned = identity(DEVNET_GENESIS, Err("timed out"));
	assert!(unversioned.matches(&identity(DEVNET_GENESIS, Ok(false))));
	assert!(!unversioned.matches(&identity(MAINNET_GENESIS, Ok(false))));
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

	// A failed `getVersion` leaves the endpoint unclassified, but keeps the hash.
	let no_version = fakes::FakeRpcServer::start(|method, _| {
		match method {
			"getGenesisHash" => fakes::Reply::Result(json!(PRIVATE_GENESIS)),
			_ => fakes::Reply::Error(-32601, format!("method {method} not found")),
		}
	});
	let identity = probe(&no_version.url).unwrap();
	assert_eq!(identity.genesis_hash, PRIVATE_GENESIS);
	let error = identity.public_cluster().unwrap_err();
	assert!(error.starts_with("getVersion "), "{error}");

	// The listener stays bound, so no other test can take the port, and it
	// closes the connection without answering.
	let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
	let address = listener.local_addr().unwrap();
	let closer = std::thread::spawn(move || drop(listener.accept()));
	let error = probe(&format!("http://{address}")).unwrap_err();
	closer.join().unwrap();
	assert!(error.starts_with("getGenesisHash "), "{error}");
}
