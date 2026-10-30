//! Private worker for rehearsal forks.
//!
//! The parent and worker exchange bounded JSON frames over inherited pipes.
//! A separate reader owns stdin so it can terminate the worker on parent death
//! even while the SVM is processing a request.

use std::io::BufRead as _;
use std::io::Read as _;
use std::io::Write as _;
use std::io::{self};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;

use crossbeam_channel::Receiver;
use jsonrpc_core::MetaIoHandler;
use serde_json::Value;
use serde_json::json;
use surfpool_core::rpc::RunloopContext;
use surfpool_core::rpc::accounts_data::AccountsData;
use surfpool_core::rpc::accounts_data::SurfpoolAccountsDataRpc;
use surfpool_core::rpc::surfnet_cheatcodes::SurfnetCheatcodes;
use surfpool_core::rpc::surfnet_cheatcodes::SurfnetCheatcodesRpc;
use surfpool_core::surfnet::PluginCommand;
use surfpool_core::surfnet::locker::SurfnetSvmLocker;
use surfpool_core::surfnet::remote::SurfnetRemoteClient;
use surfpool_core::surfnet::svm::SurfnetSvm;
use surfpool_core::surfnet::svm::SurfnetSvmConfig;
use surfpool_types::CheatcodeConfig;
use surfpool_types::RpcConfig;
use surfpool_types::SimnetCommand;

/// A program chunk is hex encoded, so the one-MiB chunk limit needs two MiB.
const MAX_REQUEST_BYTES: u64 = 4 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;

static RUNNING_AS_PINA: AtomicBool = AtomicBool::new(false);

/// Mark the Pina binary as the owner of its private worker entry point.
#[doc(hidden)]
pub fn register_cli_executable() {
	RUNNING_AS_PINA.store(true, Ordering::Relaxed);
}

pub(crate) fn executable() -> io::Result<PathBuf> {
	if let Some(path) = std::env::var_os("PINA_REHEARSE_WORKER") {
		return Ok(PathBuf::from(path));
	}
	if RUNNING_AS_PINA.load(Ordering::Relaxed) {
		std::env::current_exe()
	} else {
		Ok(PathBuf::from("pina"))
	}
}

type Context = Option<RunloopContext>;
type Handler = MetaIoHandler<Context>;

struct Fork {
	handler: Handler,
	context: Context,
	_commands: Receiver<SimnetCommand>,
	_plugins: Receiver<PluginCommand>,
}

/// A private entry point used by the `pina` binary, not a public CLI command.
pub fn run() -> Result<(), String> {
	let (requests, receiver) = mpsc::sync_channel::<Vec<u8>>(1);
	std::thread::spawn(move || {
		let stdin = io::stdin();
		let mut input = stdin.lock();
		loop {
			let mut frame = Vec::new();
			let count = match (&mut input)
				.take(MAX_REQUEST_BYTES + 1)
				.read_until(b'\n', &mut frame)
			{
				Ok(count) => count,
				Err(_) => std::process::exit(1),
			};
			// EOF means the parent disappeared. Do not wait for a blocked SVM.
			if count == 0 {
				std::process::exit(1);
			}
			if frame.len() as u64 > MAX_REQUEST_BYTES || frame.last() != Some(&b'\n') {
				std::process::exit(1);
			}
			if requests.send(frame).is_err() {
				std::process::exit(1);
			}
		}
	});
	let runtime = tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.build()
		.map_err(|_| "could not start rehearsal runtime".to_owned())?;
	let init = receiver
		.recv()
		.map_err(|_| "worker input closed".to_owned())?;
	let init: Value =
		serde_json::from_slice(&init).map_err(|_| "invalid worker initialization".to_owned())?;
	let url = init
		.get("url")
		.and_then(Value::as_str)
		.ok_or("missing datasource URL")?;
	let fork = match runtime.block_on(initialize(url)) {
		Ok(fork) => fork,
		Err(error) => {
			write_frame(&json!({"error": error}).to_string())?;
			return Err(error);
		}
	};
	write_frame(
		&json!({"ready": true, "protocol": 1, "pinaVersion": env!("CARGO_PKG_VERSION")})
			.to_string(),
	)?;
	for request in receiver {
		let request =
			std::str::from_utf8(&request).map_err(|_| "invalid request encoding".to_owned())?;
		let response = runtime
			.block_on(fork.handler.handle_request(request, fork.context.clone()))
			.ok_or("request returned no response")?;
		write_frame(&response)?;
	}
	Ok(())
}

async fn initialize(url: &str) -> Result<Fork, String> {
	let config = SurfnetSvmConfig {
		skip_blockhash_check: true,
		..Default::default()
	};
	let (svm, events, geyser) =
		SurfnetSvm::new(config).map_err(|_| "could not initialize the rehearsal SVM".to_owned())?;
	std::thread::spawn(move || while events.recv().is_ok() {});
	std::thread::spawn(move || while geyser.recv().is_ok() {});
	let locker = SurfnetSvmLocker::new(svm);
	let remote = Some(
		SurfnetRemoteClient::try_new(url)
			.map_err(|_| "could not initialize the rehearsal datasource".to_owned())?,
	);
	locker.initialize(&remote).await.map_err(|error| {
		let error: jsonrpc_core::Error = error.into();
		format!(
			"could not initialize the rehearsal fork from the datasource (RPC error {})",
			error.code.code()
		)
	})?;
	let (simnet_commands_tx, _commands) = crossbeam_channel::unbounded();
	let (plugin_commands_tx, _plugins) = crossbeam_channel::unbounded();
	let context = Some(RunloopContext {
		id: None,
		svm_locker: locker,
		simnet_commands_tx,
		remote_rpc_client: remote,
		rpc_config: RpcConfig::default(),
		cheatcode_config: CheatcodeConfig::new(),
		plugin_commands_tx,
	});
	let mut handler = Handler::default();
	handler.extend_with(SurfpoolAccountsDataRpc.to_delegate());
	handler.extend_with(SurfnetCheatcodesRpc::empty().to_delegate());
	Ok(Fork {
		handler,
		context,
		_commands,
		_plugins,
	})
}

fn write_frame(frame: &str) -> Result<(), String> {
	if frame.len() > MAX_RESPONSE_BYTES {
		return Err("worker response exceeds the size limit".to_owned());
	}
	let stdout = io::stdout();
	let mut output = stdout.lock();
	output
		.write_all(frame.as_bytes())
		.and_then(|_| output.write_all(b"\n"))
		.and_then(|_| output.flush())
		.map_err(|_| "could not write worker response".to_owned())
}
