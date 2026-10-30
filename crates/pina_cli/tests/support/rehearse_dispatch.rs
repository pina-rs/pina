//! Exploratory real-runtime test for a Surfpool fork without RPC listeners.
//!
//! This module uses the offline remote fixture from `rehearse_surfpool.rs`.
//! The production rehearsal transport uses a private worker built on these APIs.

use std::sync::mpsc;
use std::time::Duration;

use jsonrpc_core::MetaIoHandler;
use serde_json::Value;
use serde_json::json;
use surfpool_core::rpc::RunloopContext;
use surfpool_core::rpc::accounts_data::AccountsData;
use surfpool_core::rpc::accounts_data::SurfpoolAccountsDataRpc;
use surfpool_core::rpc::surfnet_cheatcodes::SurfnetCheatcodes;
use surfpool_core::rpc::surfnet_cheatcodes::SurfnetCheatcodesRpc;
use surfpool_core::surfnet::locker::SurfnetSvmLocker;
use surfpool_core::surfnet::remote::SurfnetRemoteClient;
use surfpool_core::surfnet::svm::SurfnetSvm;
use surfpool_core::surfnet::svm::SurfnetSvmConfig;
use surfpool_types::CheatcodeConfig;
use surfpool_types::RpcConfig;

type Error = Box<dyn std::error::Error>;

async fn call(
	handler: &MetaIoHandler<Option<RunloopContext>>,
	context: &Option<RunloopContext>,
	method: &str,
	params: Value,
) -> Result<Value, Error> {
	let request = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
	let response = handler
		.handle_request(&request.to_string(), context.clone())
		.await
		.ok_or("request returned no response")?;
	let mut value: Value = serde_json::from_str(&response)?;

	if let Some(error) = value.get("error") {
		return Err(format!("{method} error: {error}").into());
	}

	value
		.get_mut("result")
		.map(Value::take)
		.ok_or_else(|| format!("{method} returned no result").into())
}

#[test]
#[ignore = "starts a disposable offline Surfpool datasource; run in the Surfpool CI job"]
fn dispatches_fork_operations_without_starting_a_fork_server() -> Result<(), Error> {
	let remote = super::RemoteCluster::start();
	let address = "11111111111111111111111111111112";
	let absent = "11111111111111111111111111111113";
	super::rpc(
		&remote.url,
		"surfnet_setAccount",
		json!([address, {"lamports": 1234567}]),
	);
	let signatures = super::counter_traffic(&remote);
	let transaction = super::rpc(
		&remote.url,
		"getTransaction",
		json!([signatures[3], {"encoding": "base64", "maxSupportedTransactionVersion": 0}]),
	);
	let wire = transaction["transaction"][0]
		.as_str()
		.ok_or("missing encoded transaction")?;
	let candidate = std::fs::read(super::artifact("rehearse-variant/counter_program.so"))?;
	let external = external_fork(&remote)?;
	let profile = json!([wire, null, {"encoding": "base64", "depth": "instruction"}]);
	let external_baseline =
		super::rpc(&external.url, "surfnet_profileTransaction", profile.clone());
	for (index, chunk) in candidate.chunks(256 * 1024).enumerate() {
		super::rpc(
			&external.url,
			"surfnet_writeProgram",
			json!([
				super::COUNTER_PROGRAM_ID,
				super::hex(chunk),
				index * 256 * 1024,
				null
			]),
		);
	}
	let external_changed = super::rpc(&external.url, "surfnet_profileTransaction", profile);
	let runtime = tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.build()?;
	runtime.block_on(dispatch(
		&remote,
		address,
		absent,
		wire,
		&candidate,
		&external_baseline,
		&external_changed,
	))?;
	let original = super::rpc(
		&remote.url,
		"getAccountInfo",
		json!([address, {"encoding": "base64"}]),
	);
	assert_eq!(
		original["value"]["lamports"], 1234567,
		"fork mutation did not affect the datasource"
	);
	Ok(())
}

async fn dispatch(
	remote: &super::RemoteCluster,
	address: &str,
	absent: &str,
	wire: &str,
	candidate: &[u8],
	external_baseline: &Value,
	external_changed: &Value,
) -> Result<(), Error> {
	let config = SurfnetSvmConfig {
		skip_blockhash_check: true,
		..Default::default()
	};
	let (svm, simnet_events, geyser_events) = SurfnetSvm::new(config)?;
	// The SVM channels are bounded; drain them until the fork is dropped.
	let (finished_tx, finished_rx) = mpsc::channel();
	let simnet_finished = finished_tx.clone();
	let simnet_drain = std::thread::spawn(move || {
		while simnet_events.recv().is_ok() {}
		let _ = simnet_finished.send(());
	});
	let geyser_drain = std::thread::spawn(move || {
		while geyser_events.recv().is_ok() {}
		let _ = finished_tx.send(());
	});
	let locker = SurfnetSvmLocker::new(svm);
	let remote_client = Some(SurfnetRemoteClient::try_new(&remote.url)?);
	locker.initialize(&remote_client).await?;
	let (simnet_commands_tx, _simnet_commands_rx) = crossbeam_channel::unbounded();
	let (plugin_commands_tx, _plugin_commands_rx) = crossbeam_channel::unbounded();
	let context = Some(RunloopContext {
		id: None,
		svm_locker: locker,
		simnet_commands_tx,
		remote_rpc_client: remote_client,
		rpc_config: RpcConfig::default(),
		cheatcode_config: CheatcodeConfig::new(),
		plugin_commands_tx,
	});
	let mut handler = MetaIoHandler::<Option<RunloopContext>>::default();
	handler.extend_with(SurfpoolAccountsDataRpc.to_delegate());
	handler.extend_with(SurfnetCheatcodesRpc::empty().to_delegate());
	let value = call(
		&handler,
		&context,
		"getAccountInfo",
		json!([address, {"encoding": "base64"}]),
	)
	.await?;
	assert_eq!(
		value["value"]["lamports"], 1234567,
		"remote account is hydrated"
	);
	call(&handler, &context, "surfnet_setAccount", json!([address, {"lamports": 7654321, "data": "", "owner": "11111111111111111111111111111111", "executable": false, "rentEpoch": 0}])).await?;
	let value = call(
		&handler,
		&context,
		"getMultipleAccounts",
		json!([[address, absent], {"encoding": "base64", "dataSlice": {"offset": 0, "length": 0}}]),
	)
	.await?;
	assert_eq!(
		value["value"][0]["lamports"], 7654321,
		"fork mutation is visible"
	);
	assert!(
		value["value"][1].is_null(),
		"missing remote account remains absent"
	);
	call(
		&handler,
		&context,
		"surfnet_offlineAccount",
		json!([absent, {"includeOwnedAccounts": false}]),
	)
	.await?;
	let value = call(
		&handler,
		&context,
		"getAccountInfo",
		json!([absent, {"encoding": "base64"}]),
	)
	.await?;
	assert!(
		value["value"].is_null(),
		"offlined missing account remains absent"
	);
	let params = json!([wire, null, {"encoding": "base64", "depth": "instruction"}]);
	let baseline = call(
		&handler,
		&context,
		"surfnet_profileTransaction",
		params.clone(),
	)
	.await?;
	let baseline_tx = &baseline["value"]["transactionProfile"];
	assert_eq!(
		baseline_tx["computeUnitsConsumed"],
		external_baseline["value"]["transactionProfile"]["computeUnitsConsumed"],
		"external and embedded baseline compute units differ"
	);
	assert_eq!(
		baseline_tx["accountStates"],
		external_baseline["value"]["transactionProfile"]["accountStates"],
		"external and embedded baseline account states differ"
	);
	assert!(
		baseline_tx["errorMessage"].is_null(),
		"baseline profile: {baseline}"
	);
	assert!(
		baseline_tx["computeUnitsConsumed"]
			.as_u64()
			.is_some_and(|cu| cu > 0),
		"baseline compute units: {baseline}"
	);
	for (index, chunk) in candidate.chunks(256 * 1024).enumerate() {
		call(
			&handler,
			&context,
			"surfnet_writeProgram",
			json!([
				super::COUNTER_PROGRAM_ID,
				super::hex(chunk),
				index * 256 * 1024,
				null
			]),
		)
		.await?;
	}
	let changed = call(&handler, &context, "surfnet_profileTransaction", params).await?;
	let changed_tx = &changed["value"]["transactionProfile"];
	assert_eq!(
		changed_tx["computeUnitsConsumed"],
		external_changed["value"]["transactionProfile"]["computeUnitsConsumed"],
		"external and embedded candidate compute units differ"
	);
	assert_eq!(
		changed_tx["accountStates"],
		external_changed["value"]["transactionProfile"]["accountStates"],
		"external and embedded candidate account states differ"
	);
	assert!(
		changed_tx["errorMessage"].is_null(),
		"candidate profile: {changed}"
	);
	assert_ne!(
		baseline_tx["accountStates"], changed_tx["accountStates"],
		"candidate changes replayed account state"
	);
	drop(context);
	drop(handler);
	finished_rx.recv_timeout(Duration::from_secs(5))?;
	finished_rx.recv_timeout(Duration::from_secs(5))?;
	simnet_drain.join().map_err(|_| "simnet drain panicked")?;
	geyser_drain.join().map_err(|_| "geyser drain panicked")?;
	Ok::<(), Error>(())
}

#[test]
#[ignore = "starts a disposable offline datasource and private rehearsal worker"]
fn private_worker_stops_when_its_parent_closes_the_pipe() -> Result<(), Error> {
	use std::io::BufRead as _;
	use std::io::Write as _;
	use std::process::Command;
	use std::process::Stdio;
	use std::time::Duration;
	use std::time::Instant;

	struct Worker(std::process::Child);
	impl Drop for Worker {
		fn drop(&mut self) {
			let _ = self.0.kill();
			let _ = self.0.wait();
		}
	}

	let remote = super::RemoteCluster::start();
	let address = "11111111111111111111111111111112";
	super::rpc(
		&remote.url,
		"surfnet_setAccount",
		json!([address, {"lamports": 1234567}]),
	);
	let mut child = Worker(
		Command::new(env!("CARGO_BIN_EXE_pina"))
			.arg("__pina-rehearse-worker")
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.stderr(Stdio::null())
			.spawn()?,
	);
	let mut input = child.0.stdin.take().ok_or("missing worker stdin")?;
	let output = child.0.stdout.take().ok_or("missing worker stdout")?;
	let mut output = std::io::BufReader::new(output);
	writeln!(input, "{}", json!({"url": remote.url}))?;
	let mut response = String::new();
	output.read_line(&mut response)?;
	assert_eq!(serde_json::from_str::<Value>(&response)?["ready"], true);
	writeln!(
		input,
		"{}",
		json!({"jsonrpc":"2.0", "id":1, "method":"getAccountInfo", "params":[address, {"encoding":"base64"}]})
	)?;
	response.clear();
	output.read_line(&mut response)?;
	let response: Value = serde_json::from_str(&response)?;
	assert_eq!(response["result"]["value"]["lamports"], 1234567);
	#[cfg(target_os = "linux")]
	assert_no_listening_sockets(child.0.id())?;
	drop(input);
	let deadline = Instant::now() + Duration::from_secs(10);
	loop {
		if child.0.try_wait()?.is_some() {
			break;
		}
		if Instant::now() >= deadline {
			return Err("worker survived loss of its input pipe".into());
		}
		std::thread::sleep(Duration::from_millis(25));
	}
	Ok(())
}

#[cfg(target_os = "linux")]
fn assert_no_listening_sockets(pid: u32) -> Result<(), Error> {
	use std::collections::HashSet;
	use std::io::ErrorKind;
	let root = format!("/proc/{pid}");
	let mut sockets = HashSet::new();
	for entry in std::fs::read_dir(format!("{root}/fd"))? {
		let entry = entry?;
		let target = match std::fs::read_link(entry.path()) {
			Ok(target) => target,
			Err(error) if error.kind() == ErrorKind::NotFound => continue,
			Err(error) => return Err(error.into()),
		};
		if let Some(inode) = target
			.to_str()
			.and_then(|value| value.strip_prefix("socket:["))
			.and_then(|value| value.strip_suffix(']'))
		{
			sockets.insert(inode.to_owned());
		}
	}
	for table in ["tcp", "tcp6"] {
		let entries = std::fs::read_to_string(format!("{root}/net/{table}"))?;
		for row in entries.lines().skip(1) {
			let fields = row.split_whitespace().collect::<Vec<_>>();
			if fields.len() < 10 {
				return Err("malformed Linux socket table".into());
			}
			if fields[3] == "0A" && sockets.contains(fields[9]) {
				return Err(format!("private worker owns a TCP listener: {row}").into());
			}
		}
	}
	Ok(())
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "kills a worker owner while the datasource is stalled"]
fn private_worker_exits_if_its_owner_is_killed_during_startup() -> Result<(), Error> {
	use std::io::BufRead as _;
	use std::io::Write as _;
	use std::net::TcpListener;
	use std::process::Command;
	use std::process::Stdio;
	use std::time::Duration;
	use std::time::Instant;

	const OWNER_ENV: &str = "PINA_TEST_REHEARSAL_WORKER_OWNER_URL";
	const PID_PREFIX: &str = "REHEARSAL_WORKER_PID=";
	if let Some(url) = std::env::var_os(OWNER_ENV) {
		let mut worker = Command::new(env!("CARGO_BIN_EXE_pina"))
			.arg("__pina-rehearse-worker")
			.stdin(Stdio::piped())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.spawn()?;
		let mut input = worker.stdin.take().ok_or("worker has no stdin")?;
		writeln!(input, "{}", json!({"url": url.to_string_lossy()}))?;
		println!("{PID_PREFIX}{}", worker.id());
		std::io::stdout().flush()?;
		// Child::wait closes an attached stdin. Keep this separate handle alive
		// so the owner only loses it when the test kills the owner process.
		let result = worker.wait();
		drop(input);
		let _ = result?;
		return Err("worker exited before its owner was killed".into());
	}

	let listener = TcpListener::bind("127.0.0.1:0")?;
	listener.set_nonblocking(true)?;
	let url = format!("http://{}", listener.local_addr()?);
	let mut owner = Command::new(std::env::current_exe()?)
		.args([
			"--exact",
			"rehearse_dispatch::private_worker_exits_if_its_owner_is_killed_during_startup",
			"--ignored",
			"--nocapture",
		])
		.env(OWNER_ENV, url)
		.env("NO_PROXY", "127.0.0.1")
		.env("no_proxy", "127.0.0.1")
		.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::null())
		.spawn()?;
	let output = owner.stdout.take().ok_or("owner has no stdout")?;
	let (pid_tx, pid_rx) = mpsc::channel();
	std::thread::spawn(move || {
		for line in std::io::BufReader::new(output).lines() {
			let Ok(line) = line else { break };
			if let Some((_, pid)) = line.split_once(PID_PREFIX) {
				let _ = pid_tx.send(pid.trim().parse::<u32>());
				break;
			}
		}
	});
	let result = (|| -> Result<(), Error> {
		let worker_pid = pid_rx.recv_timeout(Duration::from_secs(20))??;
		let deadline = Instant::now() + Duration::from_secs(20);
		let _stalled_connection = loop {
			match listener.accept() {
				Ok((stream, _)) => break stream,
				Err(error)
					if error.kind() == std::io::ErrorKind::WouldBlock
						&& Instant::now() < deadline =>
				{
					std::thread::sleep(Duration::from_millis(25))
				}
				Err(error) => return Err(error.into()),
			}
		};
		owner.kill()?;
		owner.wait()?;
		let deadline = Instant::now() + Duration::from_secs(10);
		loop {
			let stat = match std::fs::read_to_string(format!("/proc/{worker_pid}/stat")) {
				Ok(stat) => stat,
				Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
				Err(error) => return Err(error.into()),
			};
			let state = stat
				.rsplit_once(')')
				.and_then(|(_, tail)| tail.split_whitespace().next());
			if state == Some("Z") {
				break;
			}
			if Instant::now() >= deadline {
				return Err("worker survived owner SIGKILL during a blocked operation".into());
			}
			std::thread::sleep(Duration::from_millis(25));
		}
		Ok(())
	})();
	let _ = owner.kill();
	let _ = owner.wait();
	result
}

/// The current external Surfpool implementation, used only as a parity oracle.
fn external_fork(remote: &super::RemoteCluster) -> Result<super::RemoteCluster, Error> {
	use std::process::Command;
	use std::process::Stdio;
	use std::time::Duration;
	use std::time::Instant;
	let directory = tempfile::tempdir()?;
	let port = super::free_port();
	let child = Command::new(super::surfpool())
		.current_dir(directory.path())
		.args([
			"start",
			"--rpc-url",
			&remote.url,
			"--host",
			"127.0.0.1",
			"--no-tui",
			"--no-studio",
			"--no-deploy",
			"--skip-blockhash-check",
			"--block-production-mode",
			"manual",
			"--airdrop-amount",
			"0",
		])
		.args([
			"--port",
			&port.to_string(),
			"--ws-port",
			&super::free_port().to_string(),
			"--studio-port",
			&super::free_port().to_string(),
		])
		.arg("--log-path")
		.arg(directory.path().join("logs"))
		.stdin(Stdio::null())
		.stdout(Stdio::null())
		.stderr(Stdio::null())
		.spawn()?;
	let fork = super::RemoteCluster {
		child,
		url: format!("http://127.0.0.1:{port}"),
		_directory: directory,
	};
	let deadline = Instant::now() + Duration::from_secs(60);
	while super::try_rpc(&fork.url, "getHealth", json!([])).is_err() {
		if Instant::now() >= deadline {
			return Err("external parity fork did not start".into());
		}
		std::thread::sleep(Duration::from_millis(100));
	}
	Ok(fork)
}
