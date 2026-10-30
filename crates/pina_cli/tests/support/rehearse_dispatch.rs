//! Exploratory real-runtime test for a Surfpool fork without RPC listeners.
//!
//! This module uses the offline remote fixture from `rehearse_surfpool.rs`.
//! It does not yet replace the production rehearsal transport.

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
	let runtime = tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.build()?;
	runtime.block_on(dispatch(&remote, address, absent))?;
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

async fn dispatch(remote: &super::RemoteCluster, address: &str, absent: &str) -> Result<(), Error> {
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
	drop(context);
	drop(handler);
	finished_rx.recv_timeout(Duration::from_secs(5))?;
	finished_rx.recv_timeout(Duration::from_secs(5))?;
	simnet_drain.join().map_err(|_| "simnet drain panicked")?;
	geyser_drain.join().map_err(|_| "geyser drain panicked")?;
	Ok::<(), Error>(())
}
