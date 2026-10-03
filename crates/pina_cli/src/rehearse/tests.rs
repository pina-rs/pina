use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use super::support::AUTHORITY;
use super::support::Binary;
use super::support::FakeFork;
use super::support::FakeRpcServer;
use super::support::Fixtures;
use super::support::ForkFaults;
use super::support::PROGRAM_ID;
use super::support::Reply;
use super::support::remote_handler;
use super::support::requests_by_method;
use super::*;

fn counter_project() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/counter_program")
}

/// A scratch directory holding a candidate file with `bytes`.
fn candidate(bytes: &[u8]) -> (tempfile::TempDir, PathBuf) {
	let directory =
		tempfile::tempdir().unwrap_or_else(|error| panic!("create candidate dir: {error}"));
	let path = directory.path().join("candidate.so");
	std::fs::write(&path, bytes).unwrap_or_else(|error| panic!("write candidate: {error}"));
	(directory, path)
}

fn options(program: Option<PathBuf>, signatures: Vec<String>) -> RehearseOptions {
	RehearseOptions {
		project: counter_project(),
		network: RehearseNetwork::Cluster(RehearseCluster::Devnet),
		program,
		build: false,
		limit: DEFAULT_LIMIT,
		signatures,
	}
}

fn target(binary: Binary) -> Target {
	let (_directory, path) = candidate(&binary.elf());
	Target::load(&options(Some(path), Vec::new()), &mut io::sink())
		.unwrap_or_else(|error| panic!("load the counter target: {error}"))
}

fn endpoint(url: &str) -> Endpoint {
	Endpoint {
		url: url.to_owned(),
		label: "fixture".to_owned(),
	}
}

/// Fetch the captured traffic through a fake remote.
fn replays(target: &Target, selection: &Selection) -> Vec<Replay> {
	let remote = FakeRpcServer::start(remote_handler(&Fixtures::load()));
	let client = JsonRpc::new(&remote.url, REMOTE_TIMEOUT, true);
	fetch(
		&client,
		&endpoint(&remote.url),
		target,
		selection,
		&mut io::sink(),
	)
	.unwrap_or_else(|error| panic!("fetch the captured traffic: {error}"))
}

/// A rehearsal's report with its progress output, and every request the fork
/// received.
type Rehearsed = (
	Result<(RehearsalReport, String), RehearseError>,
	Vec<(String, Value)>,
);

/// Replay the captured traffic with `binary` as the candidate.
fn rehearse_against(binary: Binary, faults: ForkFaults) -> Rehearsed {
	let target = target(binary);
	let transactions = replays(&target, &Selection::Latest(DEFAULT_LIMIT));
	let fork = FakeFork::new(faults).serve();
	let client = JsonRpc::new(&fork.url, LOCAL_TIMEOUT, true);
	let mut progress = Vec::new();
	let report = replay(
		&client,
		&endpoint("unused"),
		&target,
		&transactions,
		&mut progress,
	);

	(
		report.map(|report| (report, String::from_utf8_lossy(&progress).into_owned())),
		fork.requests(),
	)
}

fn report(binary: Binary) -> (RehearsalReport, Vec<(String, Value)>) {
	let (report, requests) = rehearse_against(binary, ForkFaults::default());
	let (report, progress) = report.unwrap_or_else(|error| panic!("rehearse {binary:?}: {error}"));

	for stage in [
		"Profiling 4 transactions against the deployed program",
		"Installing the candidate (sha256 ",
		"Profiling 4 transactions against the candidate",
	] {
		assert!(
			progress.contains(stage),
			"missing {stage:?} in:\n{progress}"
		);
	}

	(report, requests)
}

fn transaction<'a>(report: &'a RehearsalReport, signature: &str) -> &'a TransactionRehearsal {
	report
		.transactions
		.iter()
		.find(|transaction| transaction.signature == signature)
		.unwrap_or_else(|| panic!("{signature} is reported"))
}

#[test]
fn networks_come_from_cluster_names_or_an_rpc_url() {
	let url = Some("http://127.0.0.1:8899".to_owned());

	assert_eq!(
		RehearseNetwork::from_flags(Some("devnet"), url.clone()).ok(),
		Some(RehearseNetwork::RpcUrl("http://127.0.0.1:8899".to_owned()))
	);

	for (name, cluster) in [
		("mainnet", RehearseCluster::Mainnet),
		("devnet", RehearseCluster::Devnet),
		("testnet", RehearseCluster::Testnet),
	] {
		assert_eq!(
			RehearseNetwork::from_flags(Some(name), None).ok(),
			Some(RehearseNetwork::Cluster(cluster))
		);
	}

	let missing = RehearseNetwork::from_flags(None, None)
		.err()
		.unwrap_or_else(|| panic!("a network is required"));
	assert!(missing.to_string().ends_with("or --rpc-url"));
	let unknown = RehearseNetwork::from_flags(Some("localnet"), None)
		.err()
		.unwrap_or_else(|| panic!("localnet is not a rehearsal cluster"));
	assert!(
		unknown
			.to_string()
			.contains("(unknown network \"localnet\")")
	);
}

#[test]
fn endpoints_resolve_clusters_and_label_custom_urls_by_origin() {
	for (cluster, url, label) in [
		(
			RehearseCluster::Mainnet,
			"https://api.mainnet-beta.solana.com",
			"mainnet",
		),
		(
			RehearseCluster::Devnet,
			"https://api.devnet.solana.com",
			"devnet",
		),
		(
			RehearseCluster::Testnet,
			"https://api.testnet.solana.com",
			"testnet",
		),
	] {
		let endpoint = Endpoint::resolve(&RehearseNetwork::Cluster(cluster))
			.unwrap_or_else(|error| panic!("resolve {label}: {error}"));
		assert_eq!(
			(endpoint.url.as_str(), endpoint.label.as_str()),
			(url, label)
		);
	}

	let custom = Endpoint::resolve(&RehearseNetwork::RpcUrl(
		"https://rpc.example/v2/provider-key".to_owned(),
	))
	.unwrap_or_else(|error| panic!("resolve a custom URL: {error}"));
	assert_eq!(custom.url, "https://rpc.example/v2/provider-key");
	assert_eq!(custom.label, "https://rpc.example");

	let secret = "private-rpc-credential";
	let unsafe_url = Endpoint::resolve(&RehearseNetwork::RpcUrl(format!(
		"https://agent:{secret}@rpc.example/?token={secret}"
	)))
	.err()
	.unwrap_or_else(|| panic!("credentials are rejected"));
	assert!(matches!(
		unsafe_url,
		RehearseError::Workflow(WorkflowError::UnsafeSurfpoolRpcUrl)
	));
	assert!(!unsafe_url.to_string().contains(secret));
}

#[test]
fn selections_validate_limits_and_signatures() {
	let signature = Fixtures::load().signatures()[0].clone();
	let mut limited = options(None, Vec::new());

	for limit in [0, MAX_LIMIT + 1] {
		limited.limit = limit;
		assert!(matches!(
			Selection::new(&limited),
			Err(RehearseError::InvalidLimit { .. })
		));
	}

	limited.limit = MAX_LIMIT;
	assert!(matches!(
		Selection::new(&limited),
		Ok(Selection::Latest(MAX_LIMIT))
	));

	for invalid in ["not base58!", "1111"] {
		let error = Selection::new(&options(None, vec![invalid.to_owned()]))
			.err()
			.unwrap_or_else(|| panic!("{invalid} is not a signature"));
		assert!(error.to_string().contains(invalid));
	}

	assert!(matches!(
		Selection::new(&options(None, vec![signature.clone()])),
		Ok(Selection::Signatures(signatures)) if signatures == [signature]
	));
}

#[test]
fn targets_read_the_project_program_and_candidate() {
	let target = target(Binary::Variant);

	assert_eq!(target.program_id, PROGRAM_ID);
	assert_eq!(target.candidate, Binary::Variant.elf());
	assert_eq!(
		target.candidate_sha256,
		executable_sha256(&Binary::Variant.elf())
	);
	assert_eq!(target.catalog.instruction_name(&[1]), "increment");

	let (directory, not_elf) = candidate(b"not an elf");
	assert!(matches!(
		Target::load(&options(Some(not_elf), Vec::new()), &mut io::sink()),
		Err(RehearseError::CandidateNotElf { .. })
	));
	assert!(matches!(
		Target::load(
			&options(Some(directory.path().join("missing.so")), Vec::new()),
			&mut io::sink()
		),
		Err(RehearseError::ReadCandidate { .. })
	));
}

#[test]
fn targets_reject_missing_projects_and_invalid_program_ids() {
	let directory =
		tempfile::tempdir().unwrap_or_else(|error| panic!("create project dir: {error}"));
	let mut missing = options(None, Vec::new());
	missing.project = directory.path().join("absent");

	assert!(matches!(
		Target::load(&missing, &mut io::sink()),
		Err(RehearseError::Project(_))
	));

	std::fs::create_dir_all(directory.path().join("src"))
		.unwrap_or_else(|error| panic!("create source dir: {error}"));
	std::fs::write(
		directory.path().join("Cargo.toml"),
		"[package]\nname = \"bad_id\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n",
	)
	.unwrap_or_else(|error| panic!("write manifest: {error}"));
	std::fs::write(
		directory.path().join("src/lib.rs"),
		"use pina::*;\ndeclare_id!(\"not-an-address\");\n",
	)
	.unwrap_or_else(|error| panic!("write source: {error}"));
	let mut invalid = options(None, Vec::new());
	invalid.project = directory.path().to_path_buf();

	assert!(matches!(
		Target::load(&invalid, &mut io::sink()),
		Err(RehearseError::InvalidProgramId { .. })
	));

	std::fs::write(
		directory.path().join("src/lib.rs"),
		format!("use pina::*;\ndeclare_id!(\"{PROGRAM_ID}\");\n"),
	)
	.unwrap_or_else(|error| panic!("write source: {error}"));
	let error = Target::load(&invalid, &mut io::sink())
		.err()
		.unwrap_or_else(|| panic!("the conventional artifact has not been built"));
	let RehearseError::ReadCandidate { path, .. } = error else {
		panic!("unexpected error: {error}");
	};
	assert!(path.ends_with("deploy/bad_id.so"), "{}", path.display());
}

#[test]
fn fetches_recent_traffic_and_names_program_instructions() {
	let fixtures = Fixtures::load();
	let remote = FakeRpcServer::start(remote_handler(&fixtures));
	let client = JsonRpc::new(&remote.url, REMOTE_TIMEOUT, true);
	let target = target(Binary::Deployed);
	let mut progress = Vec::new();
	let transactions = fetch(
		&client,
		&endpoint(&remote.url),
		&target,
		&Selection::Latest(7),
		&mut progress,
	)
	.unwrap_or_else(|error| panic!("fetch traffic: {error}"));

	assert_eq!(transactions.len(), 4);
	let names = transactions
		.iter()
		.map(|replay| {
			replay
				.transaction
				.as_ref()
				.map(|decoded| decoded.instructions[0].name.clone())
				.unwrap_or_default()
		})
		.collect::<Vec<_>>();
	assert_eq!(names, ["increment", "increment", "increment", "initialize"]);
	assert!(String::from_utf8_lossy(&progress).contains("Fetching up to 7 recent transactions"));

	let requests = requests_by_method(&remote.requests());
	assert_eq!(
		requests["getSignaturesForAddress"][0],
		json!([PROGRAM_ID, { "limit": 7, "commitment": "confirmed" }])
	);
	assert_eq!(
		requests["getTransaction"][0][1],
		json!({ "encoding": "base64", "maxSupportedTransactionVersion": 0, "commitment": "confirmed" })
	);
}

#[test]
fn unavailable_refused_and_undecodable_transactions_are_skipped() {
	let fixtures = Fixtures::load();
	let known = fixtures.signatures()[0].clone();
	let transactions = fixtures.transactions.clone();
	let remote = FakeRpcServer::start(move |_, params| {
		match params[0].as_str() {
			Some("refused") => {
				Reply::Error(
					-32015,
					"Transaction version (1) is not supported".to_owned(),
				)
			}
			Some("garbage") => {
				Reply::Result(json!({ "transaction": ["AA==", "base64"], "meta": {} }))
			}
			Some(signature) => {
				Reply::Result(transactions.get(signature).cloned().unwrap_or(Value::Null))
			}
			None => Reply::Result(Value::Null),
		}
	});
	let client = JsonRpc::new(&remote.url, REMOTE_TIMEOUT, true);
	let selection = Selection::Signatures(vec![
		known.clone(),
		"missing".to_owned(),
		"refused".to_owned(),
		"garbage".to_owned(),
	]);
	let transactions = fetch(
		&client,
		&endpoint(&remote.url),
		&target(Binary::Deployed),
		&selection,
		&mut io::sink(),
	)
	.unwrap_or_else(|error| panic!("fetch named transactions: {error}"));
	let skips = transactions
		.iter()
		.map(|replay| replay.transaction.as_ref().err().cloned())
		.collect::<Vec<_>>();

	assert_eq!(
		skips,
		[
			None,
			Some((
				SkipReason::Unavailable,
				"the RPC no longer returns this transaction".to_owned()
			)),
			Some((
				SkipReason::Undecodable,
				"Transaction version (1) is not supported".to_owned()
			)),
			Some((
				SkipReason::Undecodable,
				"transaction has no message".to_owned()
			)),
		]
	);
	assert_eq!(remote.methods(), ["getTransaction"; 4]);
}

#[test]
fn remote_transport_failures_abort_the_fetch() {
	let target = target(Binary::Deployed);
	let failing = FakeRpcServer::start(|_, _| Reply::Status(429));
	let client = JsonRpc::new(&failing.url, REMOTE_TIMEOUT, true);
	let error = fetch(
		&client,
		&endpoint(&failing.url),
		&target,
		&Selection::Latest(1),
		&mut io::sink(),
	)
	.err()
	.unwrap_or_else(|| panic!("a rate-limited endpoint fails the fetch"));

	assert_eq!(
		error.to_string(),
		"getSignaturesForAddress failed against fixture: the endpoint answered HTTP 429"
	);

	let malformed = FakeRpcServer::start(|_, _| Reply::Result(json!({ "transaction": 7 })));
	let client = JsonRpc::new(&malformed.url, REMOTE_TIMEOUT, true);
	let error = fetch(
		&client,
		&endpoint(&malformed.url),
		&target,
		&Selection::Signatures(vec!["any".to_owned()]),
		&mut io::sink(),
	)
	.err()
	.unwrap_or_else(|| panic!("a malformed transaction fails the fetch"));

	assert!(
		error
			.to_string()
			.starts_with("getTransaction failed against fixture: unexpected response")
	);

	// Providers report an unhealthy node, a rate limit, or a server fault as a
	// JSON-RPC error in an HTTP 200 response. None of them describes the
	// transaction, so each one aborts instead of skipping it.
	for (code, message) in [
		(-32005, "Node is unhealthy"),
		(-32429, "rate limit exceeded"),
		(-32603, "Internal error"),
	] {
		let refusing = FakeRpcServer::start(move |_, _| Reply::Error(code, message.to_owned()));
		let client = JsonRpc::new(&refusing.url, REMOTE_TIMEOUT, true);
		let error = fetch(
			&client,
			&endpoint(&refusing.url),
			&target,
			&Selection::Signatures(vec!["any".to_owned()]),
			&mut io::sink(),
		)
		.err()
		.unwrap_or_else(|| panic!("JSON-RPC error {code} must abort the fetch"));

		assert_eq!(
			error.to_string(),
			format!("getTransaction failed against fixture: JSON-RPC error {code}: {message}")
		);
	}
}

#[test]
fn the_deployed_binary_rehearses_as_unchanged() {
	let fixtures = Fixtures::load();
	let (report, requests) = report(Binary::Deployed);
	let requests = requests_by_method(&requests);

	assert_eq!(
		report.summary,
		RehearsalSummary {
			total: 4,
			unchanged: 3,
			skipped: 1,
			..RehearsalSummary::default()
		}
	);
	assert_eq!(report.deployed_sha256, report.candidate_sha256);
	assert_eq!(report.slot, 2001);
	assert_eq!(report.cluster, "fixture");
	assert_eq!(report.exit_code(false), 0);
	let initialize = transaction(&report, &fixtures.initialize_signature());
	assert_eq!(
		initialize.skip.as_ref().map(|skip| skip.reason),
		Some(SkipReason::FailedInBoth)
	);
	assert_eq!(
		report.instructions,
		[InstructionUnits {
			name: "increment".to_owned(),
			samples: 3,
			baseline: UnitStats {
				min: 473,
				median: 473,
				max: 473,
			},
			candidate: UnitStats {
				min: 473,
				median: 473,
				max: 473,
			},
			median_delta: 0,
		}]
	);

	// Every account the traffic touches is hydrated in one request, and only the
	// missing ones are frozen offline.
	assert_eq!(requests["getMultipleAccounts"].len(), 1);
	let hydrated = requests["getMultipleAccounts"][0][0]
		.as_array()
		.map_or(0, Vec::len);
	assert_eq!(requests["surfnet_offlineAccount"].len(), hydrated - 1);
	// The upgrade keeps the deployed authority.
	assert!(
		requests["surfnet_writeProgram"]
			.iter()
			.all(|params| params[3] == AUTHORITY)
	);
	// Two sentinel loads, each rewriting the program account twice.
	assert_eq!(requests["surfnet_setAccount"].len(), 4);
}

#[test]
fn a_changed_increment_is_a_decoded_state_change() {
	let (report, requests) = report(Binary::Variant);
	let requests = requests_by_method(&requests);

	assert_eq!(report.summary.state_changed, 3);
	assert_eq!(report.exit_code(false), 2);
	assert_eq!(report.exit_code(true), 0);
	let changed = report
		.transactions
		.iter()
		.find(|transaction| transaction.status == RehearsalStatus::StateChanged)
		.unwrap_or_else(|| panic!("an increment changed state"));
	assert_eq!(
		changed.accounts[0].account_type.as_deref(),
		Some("CounterState")
	);
	assert_eq!(
		changed.accounts[0].fields,
		[FieldChange {
			name: "count".to_owned(),
			baseline: "4".to_owned(),
			candidate: "5".to_owned(),
		}]
	);

	// The shorter candidate's stale tail is zeroed after it is written.
	let tail = &requests["surfnet_writeProgram"][1];
	assert_eq!(tail[2], Binary::Variant.elf().len());
	assert_eq!(
		tail[1],
		"00".repeat(Binary::Deployed.elf().len() - Binary::Variant.elf().len())
	);
}

#[test]
fn a_foreign_program_changes_every_outcome() {
	let (report, _) = report(Binary::Foreign);

	assert_eq!(report.summary.outcome_changed, 4);
	assert_eq!(report.instructions, Vec::<InstructionUnits>::new());
	let changed = &report.transactions[0];
	let logs = changed
		.logs
		.as_ref()
		.unwrap_or_else(|| panic!("outcome changes carry logs"));
	assert!(
		logs.candidate
			.iter()
			.any(|line| line.contains("incorrect program id"))
	);
	assert_eq!(changed.accounts, Vec::<AccountChange>::new());
	let text = report.render_text();
	assert!(text.contains("candidate logs:"), "{text}");
	assert!(text.contains("baseline   succeeded"), "{text}");
}

#[test]
fn program_faults_fail_the_rehearsal() {
	let cases = [
		(
			ForkFaults {
				missing_program: true,
				..ForkFaults::default()
			},
			"does not exist on fixture",
		),
		(
			ForkFaults {
				foreign_owner: true,
				..ForkFaults::default()
			},
			"its account is not an upgradeable-loader program",
		),
		(
			ForkFaults {
				missing_programdata: true,
				..ForkFaults::default()
			},
			"its program data is missing or closed",
		),
		(
			ForkFaults {
				unloadable: vec![Binary::Deployed],
				..ForkFaults::default()
			},
			"could not load the deployed program",
		),
		(
			ForkFaults {
				unloadable: vec![Binary::Variant],
				..ForkFaults::default()
			},
			"could not load the candidate program",
		),
		(
			ForkFaults {
				corrupt_writes: true,
				..ForkFaults::default()
			},
			"does not hold the candidate after installation",
		),
		(
			ForkFaults {
				malformed_profiles: true,
				..ForkFaults::default()
			},
			"surfnet_profileTransaction failed against the Surfpool fork",
		),
	];

	for (faults, message) in cases {
		let (report, _) = rehearse_against(Binary::Variant, faults.clone());
		let error = report
			.err()
			.unwrap_or_else(|| panic!("{faults:?} must fail the rehearsal"));
		assert!(error.to_string().contains(message), "{faults:?}: {error}");
	}
}

#[test]
fn refused_profiles_and_unfetchable_transactions_are_skipped() {
	let fixtures = Fixtures::load();
	let refused = fixtures.signatures()[0].clone();
	let target = target(Binary::Variant);
	let mut transactions = replays(&target, &Selection::Latest(DEFAULT_LIMIT));
	transactions.push(Replay {
		signature: "gone".to_owned(),
		transaction: Err((SkipReason::Unavailable, "pruned".to_owned())),
	});
	let fork = FakeFork::new(ForkFaults {
		refused_profiles: vec![refused.clone()],
		..ForkFaults::default()
	})
	.serve();
	let client = JsonRpc::new(&fork.url, LOCAL_TIMEOUT, true);
	let report = replay(
		&client,
		&endpoint("unused"),
		&target,
		&transactions,
		&mut io::sink(),
	)
	.unwrap_or_else(|error| panic!("rehearse with a refused profile: {error}"));

	let skipped = transaction(&report, &refused);
	assert_eq!(
		skipped.skip,
		Some(SkippedTransaction {
			reason: SkipReason::NotProfiled,
			detail: "Surfpool refused it in both runs: Transaction signature verification failure"
				.to_owned(),
		})
	);
	assert_eq!(skipped.instructions[0].name, "increment");
	assert_eq!(
		transaction(&report, "gone").skip,
		Some(SkippedTransaction {
			reason: SkipReason::Unavailable,
			detail: "pruned".to_owned(),
		})
	);
	assert_eq!(report.summary.skipped, 3);
	let text = report.render_text();
	assert_eq!(text.matches("Skipped transactions:").count(), 1);
	assert!(
		text.contains("not_profiled: Surfpool refused it in both runs:"),
		"{text}"
	);
	assert!(text.contains("gone  unavailable: pruned"), "{text}");
}

#[test]
fn inconsistent_profile_refusals_abort_the_rehearsal() {
	let fixtures = Fixtures::load();
	let increment = fixtures.signatures()[0].clone();
	let target = target(Binary::Variant);
	let transactions = replays(&target, &Selection::Latest(DEFAULT_LIMIT));

	// Refused only once the candidate is installed, or with a message that
	// differs between the runs: neither can come from the binary.
	for deployed_message in [None, Some("lookup table changed")] {
		let fork = FakeFork::new(ForkFaults::default());
		let installed = AtomicBool::new(false);
		let refused = increment.clone();
		let wires = fixtures.transactions.clone();
		let server = FakeRpcServer::start(move |method, params| {
			if method == "surfnet_writeProgram" {
				installed.store(true, Ordering::SeqCst);
			}

			let is_refused = method == "surfnet_profileTransaction"
				&& wires[&refused]["transaction"][0] == params[0];

			match (
				is_refused,
				installed.load(Ordering::SeqCst),
				deployed_message,
			) {
				(true, true, _) => Reply::Error(-32603, "Internal error".to_owned()),
				(true, false, Some(message)) => Reply::Error(-32603, message.to_owned()),
				_ => fork.handle(method, params),
			}
		});
		let client = JsonRpc::new(&server.url, LOCAL_TIMEOUT, true);
		let error = replay(
			&client,
			&endpoint("unused"),
			&target,
			&transactions,
			&mut io::sink(),
		)
		.err()
		.unwrap_or_else(|| panic!("an inconsistent refusal must abort the rehearsal"));
		let message = error.to_string();

		assert!(
			message.starts_with("surfnet_profileTransaction failed against the Surfpool fork"),
			"{message}"
		);
		assert!(message.contains(&increment), "{message}");
		assert!(
			message.contains("candidate run: Internal error"),
			"{message}"
		);

		if let Some(deployed) = deployed_message {
			assert!(
				message.contains(&format!("deployed run: {deployed}")),
				"{message}"
			);
		}
	}
}

#[test]
fn fork_transport_failures_abort_the_rehearsal() {
	let target = target(Binary::Variant);
	let failing = FakeRpcServer::start(|_, _| Reply::Status(500));
	let client = JsonRpc::new(&failing.url, LOCAL_TIMEOUT, true);
	let error = replay(&client, &endpoint("unused"), &target, &[], &mut io::sink())
		.err()
		.unwrap_or_else(|| panic!("a failing fork aborts the rehearsal"));

	assert_eq!(
		error.to_string(),
		"getMultipleAccounts failed against the Surfpool fork: the endpoint answered HTTP 500"
	);

	let no_slot = FakeRpcServer::start(|_, _| Reply::Result(json!({ "value": [] })));
	let client = JsonRpc::new(&no_slot.url, LOCAL_TIMEOUT, true);
	let error = replay(&client, &endpoint("unused"), &target, &[], &mut io::sink())
		.err()
		.unwrap_or_else(|| panic!("a slot-less hydration aborts the rehearsal"));
	assert!(
		error
			.to_string()
			.starts_with("getMultipleAccounts failed against the Surfpool fork: unexpected"),
		"{error}"
	);

	let ownerless = FakeRpcServer::start(|method, _| {
		if method == "getMultipleAccounts" {
			return Reply::Result(json!({ "context": { "slot": 1 }, "value": [null] }));
		}

		Reply::Result(json!({ "value": { "lamports": 1 } }))
	});
	let client = JsonRpc::new(&ownerless.url, LOCAL_TIMEOUT, true);
	let error = replay(&client, &endpoint("unused"), &target, &[], &mut io::sink())
		.err()
		.unwrap_or_else(|| panic!("an ownerless program account aborts the rehearsal"));
	assert_eq!(
		error.to_string(),
		"getAccountInfo failed against the Surfpool fork: unexpected response: an account has \
		 no owner"
	);

	let profile_failure = FakeFork::new(ForkFaults::default());
	let flaky = FakeRpcServer::start(move |method, params| {
		if method == "surfnet_profileTransaction" {
			return Reply::Status(503);
		}
		profile_failure.handle(method, params)
	});
	let client = JsonRpc::new(&flaky.url, LOCAL_TIMEOUT, true);
	let transactions = replays(&target, &Selection::Latest(DEFAULT_LIMIT));
	let error = replay(
		&client,
		&endpoint("unused"),
		&target,
		&transactions,
		&mut io::sink(),
	)
	.err()
	.unwrap_or_else(|| panic!("an unreachable profiler aborts the rehearsal"));
	assert!(error.to_string().contains("HTTP 503"), "{error}");
}

#[test]
fn json_rpc_transport_reports_every_failure_class() {
	let call = |reply: fn() -> Reply| {
		let server = FakeRpcServer::start(move |_, _| reply());
		JsonRpc::new(&server.url, REMOTE_TIMEOUT, true).call("getHealth", &json!([]))
	};

	assert_eq!(call(|| Reply::Result(json!("ok"))), Ok(json!("ok")));
	assert_eq!(call(|| Reply::Status(302)), Err(RpcError::Redirect(302)));
	assert_eq!(call(|| Reply::Status(500)), Err(RpcError::Http(500)));
	assert!(matches!(
		call(|| Reply::Body(b"not json".to_vec())),
		Err(RpcError::Body(_))
	));
	assert!(matches!(
		call(|| Reply::Body(vec![0xff, 0xfe])),
		Err(RpcError::Body(_))
	));
	assert_eq!(
		call(|| Reply::Body(b"{}".to_vec())),
		Err(RpcError::Shape("the response has no result".to_owned()))
	);
	assert_eq!(
		call(|| Reply::Error(-32601, "missing".to_owned())),
		Err(RpcError::Rpc {
			code: -32601,
			message: "missing".to_owned(),
		})
	);
	assert_eq!(
		call(|| Reply::Body(br#"{"error":{}}"#.to_vec())),
		Err(RpcError::Rpc {
			code: 0,
			message: "no message".to_owned(),
		})
	);
	assert!(matches!(
		JsonRpc::new("http://127.0.0.1:1", REMOTE_TIMEOUT, false).call("getHealth", &json!([])),
		Err(RpcError::Transport(_))
	));

	let server = FakeRpcServer::start(|_, _| Reply::Result(Value::Null));
	let client = JsonRpc::new(&server.url, REMOTE_TIMEOUT, true);
	let _ = client.call("first", &json!([]));
	let _ = client.call("second", &json!([1]));
	assert_eq!(server.methods(), ["first", "second"]);
}

#[test]
fn rpc_parsers_reject_malformed_responses() {
	let shape = |message: &str| Err::<(), _>(RpcError::Shape(message.to_owned()));

	assert_eq!(
		rpc::parse_signatures(&json!({})).map(|_| ()),
		shape("signatures are not an array")
	);
	assert_eq!(
		rpc::parse_signatures(&json!([{}])).map(|_| ()),
		shape("a signature entry has no signature")
	);
	assert_eq!(
		rpc::parse_transaction(&json!({ "transaction": ["AA==", "base58"] })).map(|_| ()),
		shape("the transaction is not a base64 pair")
	);
	assert_eq!(
		rpc::parse_transaction(&json!({
			"transaction": ["AA==", "base64"],
			"meta": { "loadedAddresses": { "writable": [7], "readonly": [] } }
		}))
		.map(|_| ()),
		shape("a loaded address is not a string")
	);
	assert_eq!(
		rpc::parse_transaction(&json!({
			"transaction": ["AA==", "base64"],
			"meta": { "loadedAddresses": { "writable": ["short"], "readonly": [] } }
		}))
		.map(|_| ()),
		shape("an address is not 32 base58 bytes")
	);
	let loaded = rpc::parse_transaction(&json!({
		"transaction": ["AA==", "base64"],
		"meta": { "loadedAddresses": { "writable": [AUTHORITY], "readonly": [PROGRAM_ID] } }
	}))
	.unwrap_or_else(|error| panic!("parse loaded addresses: {error}"))
	.unwrap_or_else(|| panic!("the transaction exists"));
	assert_eq!(loaded.wire, vec![0]);
	assert_eq!(loaded.loaded_addresses.len(), 2);

	assert_eq!(
		rpc::parse_accounts(&json!({ "value": [] })).map(|_| ()),
		shape("the response has no context slot")
	);
	assert_eq!(
		rpc::parse_accounts(&json!({ "context": { "slot": 1 }, "value": {} })).map(|_| ()),
		shape("the accounts are not an array")
	);
	assert_eq!(
		rpc::parse_accounts(&json!({ "context": { "slot": 1 }, "value": [{ "lamports": 1 }] }))
			.map(|_| ()),
		shape("an account has no owner")
	);

	let account = json!({
		"lamports": 1, "owner": AUTHORITY, "executable": false, "data": ["AQI=", "base64"]
	});
	for (field, message) in [
		("lamports", "an account has no lamports"),
		("owner", "an account has no owner"),
		("executable", "an account has no executable flag"),
		("data", "account data is not a base64 pair"),
	] {
		let mut broken = account.clone();
		broken[field] = Value::Null;
		assert_eq!(
			rpc::parse_account_info(&json!({ "value": broken })).map(|_| ()),
			shape(message)
		);
	}
	assert_eq!(
		rpc::parse_account_info(&json!({ "value": account }))
			.ok()
			.flatten()
			.map(|image| image.data),
		Some(vec![1, 2])
	);
}

#[test]
fn profile_parsing_reads_every_account_change() {
	let account = |lamports: u64| json!({ "lamports": lamports, "owner": AUTHORITY, "executable": false, "rentEpoch": 0, "data": ["", "base64"] });
	let profile = json!({
		"value": {
			"transactionProfile": {
				"computeUnitsConsumed": 9,
				"errorMessage": null,
				"logMessages": null,
				"accountStates": {
					"created": { "type": "writable", "accountChange": { "type": "create", "data": account(1) } },
					"deleted": { "type": "writable", "accountChange": { "type": "delete", "data": account(2) } },
					"same": { "type": "writable", "accountChange": { "type": "unchanged", "data": account(3) } },
					"absent": { "type": "writable", "accountChange": { "type": "unchanged", "data": null } },
					"read": { "type": "readonly" }
				}
			}
		}
	});
	let execution = rpc::parse_execution(&profile)
		.unwrap_or_else(|error| panic!("parse a synthetic profile: {error}"));

	assert_eq!(execution.compute_units, 9);
	assert_eq!(execution.logs, Vec::<String>::new());
	assert_eq!(execution.instruction_units, Vec::<u64>::new());
	assert_eq!(
		execution
			.accounts
			.iter()
			.map(|(address, image)| (address.as_str(), image.as_ref().map(|image| image.lamports)))
			.collect::<Vec<_>>(),
		[
			("absent", None),
			("created", Some(1)),
			("deleted", None),
			("same", Some(3))
		]
	);

	let shape = |message: &str| Err::<(), _>(RpcError::Shape(message.to_owned()));
	let mut broken = profile.clone();
	broken["value"]["transactionProfile"]["accountStates"]["read"] =
		json!({ "type": "writable", "accountChange": { "type": "moved" } });
	assert_eq!(
		rpc::parse_execution(&broken).map(|_| ()),
		shape("an account change has an unknown type")
	);
	assert_eq!(
		rpc::parse_execution(&json!({ "value": {} })).map(|_| ()),
		shape("the profile has no transaction profile")
	);
	let mut broken = profile.clone();
	broken["value"]["instructionProfiles"] = json!({});
	assert_eq!(
		rpc::parse_execution(&broken).map(|_| ()),
		shape("instruction profiles are not an array")
	);
	broken["value"]["instructionProfiles"] = json!([{}]);
	assert_eq!(
		rpc::parse_execution(&broken).map(|_| ()),
		shape("an instruction profile has no compute units")
	);
	let mut broken = profile;
	broken["value"]["transactionProfile"]["computeUnitsConsumed"] = Value::Null;
	assert_eq!(
		rpc::parse_execution(&broken).map(|_| ()),
		shape("the transaction profile has no compute units")
	);
}

fn image(lamports: u64, owner: &str, data: &[u8]) -> AccountImage {
	AccountImage {
		lamports,
		owner: owner.to_owned(),
		executable: false,
		rent_epoch: 0,
		data: data.to_vec(),
	}
}

fn execution(units: u64, accounts: &[(&str, Option<AccountImage>)]) -> Execution {
	Execution {
		compute_units: units,
		instruction_units: vec![units],
		accounts: accounts
			.iter()
			.map(|(address, image)| ((*address).to_owned(), image.clone()))
			.collect(),
		..Execution::default()
	}
}

#[test]
fn classification_covers_compute_unit_and_raw_state_changes() {
	let catalog = target(Binary::Deployed).catalog;
	let instructions = [ProgramInstruction {
		position: 0,
		name: "increment".to_owned(),
	}];
	let program = PROGRAM_ID;
	let compare = |baseline: &Execution, candidate: &Execution| {
		TransactionRehearsal::compare(
			"signature".to_owned(),
			&instructions,
			baseline,
			candidate,
			&catalog,
			program,
		)
	};
	let stable = [("payer", Some(image(5, AUTHORITY, &[])))];

	let faster = compare(&execution(10, &stable), &execution(8, &stable));
	assert_eq!(faster.status, RehearsalStatus::CuChanged);
	assert_eq!(faster.instructions[0].candidate_units, Some(8));
	let slower = compare(&execution(10, &stable), &execution(13, &stable));
	// Without instruction profiles a run still classifies, but contributes no
	// per-instruction samples.
	let unmeasured = compare(&Execution::default(), &Execution::default());
	assert_eq!(unmeasured.status, RehearsalStatus::Unchanged);
	assert_eq!(unmeasured.instructions[0].baseline_units, None);

	let different_errors = compare(
		&Execution {
			error: Some("custom program error: 0x1".to_owned()),
			..Execution::default()
		},
		&Execution {
			error: Some("custom program error: 0x2".to_owned()),
			..Execution::default()
		},
	);
	assert_eq!(different_errors.status, RehearsalStatus::OutcomeChanged);

	let other_owner = "11111111111111111111111111111111";
	let state = compare(
		&execution(
			10,
			&[
				("created", None),
				("gone", Some(image(1, program, &[1, 0, 1]))),
				("moved", Some(image(1, other_owner, &[9, 9]))),
				("both-absent", None),
			],
		),
		&execution(
			10,
			&[
				("created", Some(image(3, program, &[1, 0, 1]))),
				("gone", None),
				("moved", Some(image(2, AUTHORITY, &[9, 8, 7]))),
				("both-absent", None),
			],
		),
	);
	assert_eq!(state.status, RehearsalStatus::StateChanged);
	assert_eq!(state.accounts.len(), 3);
	let moved = &state.accounts[2];
	assert_eq!(moved.account_type, None);
	assert_eq!(moved.byte_ranges, [ByteRange { start: 1, end: 3 }]);

	let report = RehearsalReport::new(
		ReportHeader {
			cluster: "fixture".to_owned(),
			program_id: program.to_owned(),
			deployed_sha256: "a".to_owned(),
			candidate_sha256: "b".to_owned(),
			slot: 7,
		},
		vec![faster, slower, unmeasured, different_errors, state],
	);
	let text = report.render_text();

	for expected in [
		"compute units 10 -> 8 (-2)",
		"compute units 10 -> 13 (+3)",
		"baseline   failed: custom program error: 0x1",
		"candidate  failed: custom program error: 0x2",
		"exists: false -> true",
		"exists: true -> false",
		"lamports: 1 -> 2",
		&format!("owner: {other_owner} -> {AUTHORITY}"),
		"data length: 2 -> 3",
		"bytes differ: 1..3",
		"(CounterState)",
	] {
		assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
	}
	assert_eq!(report.instructions[0].samples, 3);
	assert_eq!(report.instructions[0].median_delta, 0);
}

#[test]
fn reports_render_empty_rehearsals_and_omitted_ranges() {
	let header = || {
		ReportHeader {
			cluster: "fixture".to_owned(),
			program_id: "program".to_owned(),
			deployed_sha256: "a".to_owned(),
			candidate_sha256: "a".to_owned(),
			slot: 1,
		}
	};
	let empty = RehearsalReport::new(header(), Vec::new());
	let text = empty.render_text();

	assert!(text.contains("No transactions were rehearsed"));
	assert!(text.contains("this rehearsal verified nothing about the upgrade"));
	assert!(!empty.behaviour_changed());
	// Nothing compared is never a pass, and accepting changes cannot hide it.
	assert_eq!(empty.exit_code(false), 3);
	assert_eq!(empty.exit_code(true), 3);

	let all_skipped = RehearsalReport::new(
		header(),
		vec![TransactionRehearsal::skipped(
			"stale".to_owned(),
			&[],
			SkipReason::FailedInBoth,
			"instruction requires an uninitialized account".to_owned(),
		)],
	);
	assert_eq!(all_skipped.compared(), 0);
	assert_eq!(all_skipped.exit_code(false), 3);
	assert!(
		all_skipped
			.render_text()
			.contains("No transaction was compared")
	);

	let mut changed = TransactionRehearsal::skipped(
		"signature".to_owned(),
		&[],
		SkipReason::Unavailable,
		String::new(),
	);
	changed.status = RehearsalStatus::StateChanged;
	changed.skip = None;
	changed.accounts.push(AccountChange {
		address: "account".to_owned(),
		account_type: None,
		baseline: None,
		candidate: None,
		fields: Vec::new(),
		byte_ranges: vec![ByteRange { start: 0, end: 1 }],
		omitted_byte_ranges: 4,
	});
	let text = RehearsalReport::new(header(), vec![changed]).render_text();

	assert!(text.contains("bytes differ: 0..1 (+4 more)"), "{text}");
}

#[test]
fn executable_hashes_ignore_trailing_zero_padding() {
	assert_eq!(executable_sha256(&[1, 2, 0, 0]), executable_sha256(&[1, 2]));
	assert_eq!(
		executable_sha256(&[0, 0]),
		"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
	);
	assert_ne!(executable_sha256(&[1, 0, 2]), executable_sha256(&[1, 2]));
	assert_eq!(hex(&[0, 171, 255]), "00abff");
}
