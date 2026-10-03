//! Loopback fakes for `pina rehearse` tests.
//!
//! [`FakeRpcServer`] answers JSON-RPC over real HTTP. [`remote_handler`] plays
//! the cluster and [`FakeFork`] plays the Surfpool fork. Every transaction and
//! profile they serve is real Surfpool 1.6 output recorded in
//! `tests/fixtures/rehearse` by replaying counter traffic against the deployed
//! counter (`deployed`), a counter whose increment adds two (`variant`), and
//! an unrelated program (`foreign`). The only edit to the captures zeroes each
//! profile's random per-call `key` UUID, which nothing reads, so the fixtures
//! are deterministic. Program bytes are small synthetic ELF images: the fake
//! only needs to tell the three binaries apart.
//!
//! The module is shared by the library's unit tests and the CLI integration
//! tests, so each side uses a different subset of it.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::io::Read as _;
use std::io::Write as _;
use std::net::Shutdown;
use std::net::TcpListener;
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;

use base64::Engine as _;
use serde_json::Value;
use serde_json::json;

/// The counter example's program id, which every fixture targets.
pub const PROGRAM_ID: &str = "GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS";
/// The upgrade authority the fake program data records.
pub const AUTHORITY: &str = "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin";
const UPGRADEABLE_LOADER: &str = "BPFLoaderUpgradeab1e11111111111111111111111";
const PROGRAMDATA_HEADER_BYTES: usize = 45;

/// The binaries a fixture profile was recorded against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binary {
	Deployed,
	Variant,
	Foreign,
}

impl Binary {
	fn label(self) -> &'static str {
		match self {
			Self::Deployed => "deployed",
			Self::Variant => "variant",
			Self::Foreign => "foreign",
		}
	}

	/// A synthetic ELF image standing in for the binary. The variant is shorter
	/// and the foreign program longer than the deployed one, so installing them
	/// exercises both the zeroed tail and program-data growth.
	pub fn elf(self) -> Vec<u8> {
		let (fill, length) = match self {
			Self::Deployed => (1, 60),
			Self::Variant => (2, 40),
			Self::Foreign => (3, 100),
		};
		let mut elf = b"\x7fELF".to_vec();
		elf.extend(std::iter::repeat_n(fill, length));
		elf
	}
}

/// Captured responses.
pub struct Fixtures {
	pub signatures: Value,
	pub transactions: Value,
	pub profiles: Value,
	pub program_account: Value,
	pub multiple_accounts: Value,
}

impl Fixtures {
	pub fn load() -> Self {
		let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rehearse");
		let read = |name: &str| -> Value {
			let path = directory.join(name);
			let text = std::fs::read_to_string(&path)
				.unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
			serde_json::from_str(&text)
				.unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
		};

		Self {
			signatures: read("signatures.json"),
			transactions: read("transactions.json"),
			profiles: read("profiles.json"),
			program_account: read("program-account.json"),
			multiple_accounts: read("multiple-accounts.json"),
		}
	}

	/// Signatures newest first, as `getSignaturesForAddress` returned them.
	pub fn signatures(&self) -> Vec<String> {
		self.signatures
			.as_array()
			.into_iter()
			.flatten()
			.filter_map(|entry| entry["signature"].as_str().map(str::to_owned))
			.collect()
	}

	/// The `Initialize` transaction, which fails against the current state.
	pub fn initialize_signature(&self) -> String {
		self.signatures()
			.into_iter()
			.find(|signature| {
				self.profiles["deployed"][signature]["value"]["transactionProfile"]["errorMessage"]
					.is_string()
			})
			.unwrap_or_else(|| panic!("fixtures contain the failing initialize transaction"))
	}
}

/// One JSON-RPC response.
pub enum Reply {
	Result(Value),
	Error(i64, String),
	/// An HTTP status with an empty body; 3xx statuses carry a `location`.
	Status(u16),
	/// A raw body served with HTTP 200.
	Body(Vec<u8>),
}

/// A shareable request handler.
pub type Handler = Arc<dyn Fn(&str, &Value) -> Reply + Send + Sync>;

/// A loopback JSON-RPC server that records every request.
pub struct FakeRpcServer {
	pub url: String,
	requests: Arc<Mutex<Vec<(String, Value)>>>,
	stop: Arc<AtomicBool>,
	address: std::net::SocketAddr,
	thread: Option<JoinHandle<()>>,
}

impl FakeRpcServer {
	pub fn start(handler: impl Fn(&str, &Value) -> Reply + Send + Sync + 'static) -> Self {
		let listener = TcpListener::bind("127.0.0.1:0")
			.unwrap_or_else(|error| panic!("bind fake RPC server: {error}"));
		let address = listener
			.local_addr()
			.unwrap_or_else(|error| panic!("read fake RPC address: {error}"));
		let requests = Arc::new(Mutex::new(Vec::new()));
		let stop = Arc::new(AtomicBool::new(false));
		let thread = {
			let requests = Arc::clone(&requests);
			let stop = Arc::clone(&stop);
			let handler: Handler = Arc::new(handler);
			std::thread::spawn(move || serve(&listener, &handler, &requests, &stop))
		};

		Self {
			url: format!("http://{address}"),
			requests,
			stop,
			address,
			thread: Some(thread),
		}
	}

	/// Methods received so far, in order.
	pub fn methods(&self) -> Vec<String> {
		self.requests()
			.into_iter()
			.map(|(method, _)| method)
			.collect()
	}

	/// Requests received so far, in order.
	pub fn requests(&self) -> Vec<(String, Value)> {
		self.requests
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner)
			.clone()
	}
}

impl Drop for FakeRpcServer {
	fn drop(&mut self) {
		self.stop.store(true, Ordering::SeqCst);
		// Wake the blocking accept so the thread sees the stop flag.
		let _ = TcpStream::connect(self.address);

		if let Some(thread) = self.thread.take() {
			let _ = thread.join();
		}
	}
}

/// Serve on `listener` until the process exits. Used by the fake Surfpool.
pub fn serve_forever(listener: &TcpListener, handler: &Handler) {
	serve(
		listener,
		handler,
		&Arc::new(Mutex::new(Vec::new())),
		&Arc::new(AtomicBool::new(false)),
	);
}

fn serve(
	listener: &TcpListener,
	handler: &Handler,
	requests: &Mutex<Vec<(String, Value)>>,
	stop: &AtomicBool,
) {
	for stream in listener.incoming() {
		if stop.load(Ordering::SeqCst) {
			return;
		}

		let Ok(mut stream) = stream else {
			continue;
		};
		let request = read_request(&mut stream);
		let Some((_, body)) = request.split_once("\r\n\r\n") else {
			continue;
		};
		let Ok(body) = serde_json::from_str::<Value>(body) else {
			continue;
		};
		let method = body["method"].as_str().unwrap_or_default().to_owned();
		let params = body["params"].clone();
		requests
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner)
			.push((method.clone(), params.clone()));
		let response = match handler(&method, &params) {
			Reply::Result(result) => {
				let response = json!({ "jsonrpc": "2.0", "id": body["id"], "result": result });
				http(200, response.to_string().as_bytes())
			}
			Reply::Error(code, message) => {
				let error = json!({ "code": code, "message": message });
				let response = json!({ "jsonrpc": "2.0", "id": body["id"], "error": error });
				http(200, response.to_string().as_bytes())
			}
			Reply::Status(status) => http(status, b""),
			Reply::Body(bytes) => http(200, &bytes),
		};
		let _ = stream.write_all(&response);
		let _ = stream.shutdown(Shutdown::Write);
		let mut sink = [0_u8; 512];
		while matches!(stream.read(&mut sink), Ok(read) if read > 0) {}
	}
}

fn http(status: u16, body: &[u8]) -> Vec<u8> {
	let location = if (300..400).contains(&status) {
		"location: http://127.0.0.1:1/\r\n"
	} else {
		""
	};
	let mut response = format!(
		"HTTP/1.1 {status} Fake\r\ncontent-type: application/json\r\ncontent-length: \
		 {}\r\n{location}connection: close\r\n\r\n",
		body.len()
	)
	.into_bytes();
	response.extend_from_slice(body);
	response
}

fn read_request(stream: &mut TcpStream) -> String {
	let mut request = Vec::new();
	let mut buffer = [0_u8; 4096];

	loop {
		let read = match stream.read(&mut buffer) {
			Ok(0) | Err(_) => break,
			Ok(read) => read,
		};
		request.extend_from_slice(&buffer[..read]);
		let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
			continue;
		};
		let headers = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
		let length = headers
			.lines()
			.find_map(|line| line.strip_prefix("content-length:"))
			.and_then(|value| value.trim().parse::<usize>().ok())
			.unwrap_or(0);

		if request.len() >= end + 4 + length {
			break;
		}
	}

	String::from_utf8_lossy(&request).into_owned()
}

/// A remote cluster that serves the captured traffic.
pub fn remote_handler(
	fixtures: &Fixtures,
) -> impl Fn(&str, &Value) -> Reply + Send + Sync + 'static {
	let signatures = fixtures.signatures.clone();
	let transactions = fixtures.transactions.clone();

	move |method, params| {
		match method {
			"getSignaturesForAddress" => Reply::Result(signatures.clone()),
			"getTransaction" => {
				let signature = params[0].as_str().unwrap_or_default();
				Reply::Result(transactions.get(signature).cloned().unwrap_or(Value::Null))
			}
			_ => Reply::Error(-32601, format!("method {method} not found")),
		}
	}
}

/// How a [`FakeFork`] misbehaves. Each flag injects one independent fault.
#[derive(Clone, Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct ForkFaults {
	/// The program account does not exist.
	pub missing_program: bool,
	/// The program account is owned by a loader other than the upgradeable one.
	pub foreign_owner: bool,
	/// The program's program-data account does not exist.
	pub missing_programdata: bool,
	/// `surfnet_writeProgram` flips the last byte of every chunk it stores.
	pub corrupt_writes: bool,
	/// Binaries the runtime refuses to load.
	pub unloadable: Vec<Binary>,
	/// Profiles of these signatures are refused with a JSON-RPC error.
	pub refused_profiles: Vec<String>,
	/// Profiles are answered with a body that is not a profile.
	pub malformed_profiles: bool,
}

struct ForkState {
	program_lamports: u64,
	/// Program-data account bytes, header included.
	programdata: Vec<u8>,
	/// The executable the runtime last loaded successfully.
	loaded: Vec<u8>,
}

/// A Surfpool fork with the deployed counter and captured profiles.
pub struct FakeFork {
	fixtures: Fixtures,
	faults: ForkFaults,
	programdata_address: String,
	wires: HashMap<String, String>,
	state: Mutex<ForkState>,
}

impl FakeFork {
	pub fn new(faults: ForkFaults) -> Self {
		let fixtures = Fixtures::load();
		let program_account_data = base64::engine::general_purpose::STANDARD
			.decode(
				fixtures.program_account["value"]["data"][0]
					.as_str()
					.unwrap_or_default(),
			)
			.unwrap_or_else(|error| panic!("decode program account: {error}"));
		let programdata_address = bs58::encode(&program_account_data[4..36]).into_string();
		let wires = fixtures
			.transactions
			.as_object()
			.into_iter()
			.flatten()
			.map(|(signature, transaction)| {
				(
					transaction["transaction"][0]
						.as_str()
						.unwrap_or_default()
						.to_owned(),
					signature.clone(),
				)
			})
			.collect();
		let mut programdata = vec![3, 0, 0, 0];
		programdata.extend(31_u64.to_le_bytes());
		programdata.push(1);
		programdata.extend(
			bs58::decode(AUTHORITY)
				.into_vec()
				.unwrap_or_else(|error| panic!("decode authority: {error}")),
		);
		programdata.extend(Binary::Deployed.elf());
		let lamports = fixtures.program_account["value"]["lamports"]
			.as_u64()
			.unwrap_or_default();

		Self {
			fixtures,
			faults,
			programdata_address,
			wires,
			state: Mutex::new(ForkState {
				program_lamports: lamports,
				programdata,
				loaded: Binary::Deployed.elf(),
			}),
		}
	}

	/// Serve this fork from a [`FakeRpcServer`].
	pub fn serve(self) -> FakeRpcServer {
		FakeRpcServer::start(move |method, params| self.handle(method, params))
	}

	/// The program-data account's address.
	pub fn programdata_address(&self) -> &str {
		&self.programdata_address
	}

	fn state(&self) -> std::sync::MutexGuard<'_, ForkState> {
		self.state
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner)
	}

	/// Answer one request.
	pub fn handle(&self, method: &str, params: &Value) -> Reply {
		match method {
			"getHealth" => Reply::Result(json!("ok")),
			"getMultipleAccounts" => {
				let accounts = params[0]
					.as_array()
					.into_iter()
					.flatten()
					.map(|address| self.account(address.as_str().unwrap_or_default()))
					.collect::<Vec<_>>();
				Reply::Result(json!({
					"context": self.fixtures.multiple_accounts["context"],
					"value": accounts,
				}))
			}
			"getAccountInfo" => {
				Reply::Result(json!({
					"context": self.fixtures.program_account["context"],
					"value": self.account(params[0].as_str().unwrap_or_default()),
				}))
			}
			"surfnet_offlineAccount" => Reply::Result(json!({ "context": {}, "value": null })),
			"surfnet_setAccount" => {
				let mut state = self.state();
				let executable = state.programdata[PROGRAMDATA_HEADER_BYTES..].to_vec();
				let refused = self
					.faults
					.unloadable
					.iter()
					.any(|binary| trim(&binary.elf()) == trim(&executable));

				// Like LiteSVM, store the program account only when its ELF loads.
				if !refused {
					state.program_lamports = params[1]["lamports"].as_u64().unwrap_or_default();
					state.loaded = executable;
				}

				Reply::Result(json!({ "context": {}, "value": null }))
			}
			"surfnet_writeProgram" => {
				let mut bytes = decode_hex(params[1].as_str().unwrap_or_default());
				let offset = params[2].as_u64().unwrap_or_default() as usize;

				if self.faults.corrupt_writes
					&& let Some(last) = bytes.last_mut()
				{
					*last ^= 0xff;
				}

				let mut state = self.state();
				let start = PROGRAMDATA_HEADER_BYTES + offset;
				let end = start + bytes.len();

				if state.programdata.len() < end {
					state.programdata.resize(end, 0);
				}

				state.programdata[start..end].copy_from_slice(&bytes);
				Reply::Result(json!({ "context": {}, "value": null }))
			}
			"surfnet_profileTransaction" => self.profile(params),
			_ => Reply::Error(-32601, format!("method {method} not found")),
		}
	}

	fn profile(&self, params: &Value) -> Reply {
		let wire = params[0].as_str().unwrap_or_default();
		let Some(signature) = self.wires.get(wire) else {
			return Reply::Error(-32602, "unknown transaction".to_owned());
		};

		if self.faults.refused_profiles.contains(signature) {
			return Reply::Error(
				-32002,
				"Transaction signature verification failure".to_owned(),
			);
		}

		if self.faults.malformed_profiles {
			return Reply::Result(json!({ "value": {} }));
		}

		let loaded = trim(&self.state().loaded);
		let Some(binary) = [Binary::Deployed, Binary::Variant, Binary::Foreign]
			.into_iter()
			.find(|binary| trim(&binary.elf()) == loaded)
		else {
			return Reply::Error(-32603, "the fake fork loaded an unknown program".to_owned());
		};

		Reply::Result(self.fixtures.profiles[binary.label()][signature].clone())
	}

	fn account(&self, address: &str) -> Value {
		let state = self.state();

		if address == PROGRAM_ID && !self.faults.missing_program {
			let mut account = self.fixtures.program_account["value"].clone();
			account["lamports"] = json!(state.program_lamports);

			if self.faults.foreign_owner {
				account["owner"] = json!("BPFLoader2111111111111111111111111111111111");
			}

			return account;
		}

		if address == self.programdata_address && !self.faults.missing_programdata {
			return json!({
				"data": [base64::engine::general_purpose::STANDARD.encode(&state.programdata), "base64"],
				"executable": false,
				"lamports": 1_000_000,
				"owner": UPGRADEABLE_LOADER,
				"rentEpoch": 0,
				"space": state.programdata.len(),
			});
		}

		Value::Null
	}
}

fn trim(bytes: &[u8]) -> Vec<u8> {
	let end = bytes
		.iter()
		.rposition(|byte| *byte != 0)
		.map_or(0, |last| last + 1);
	bytes[..end].to_vec()
}

fn decode_hex(value: &str) -> Vec<u8> {
	(0..value.len())
		.step_by(2)
		.map(|index| {
			u8::from_str_radix(&value[index..index + 2], 16)
				.unwrap_or_else(|error| panic!("decode hex: {error}"))
		})
		.collect()
}

/// Group recorded requests by method, for assertions.
pub fn requests_by_method(requests: &[(String, Value)]) -> BTreeMap<String, Vec<Value>> {
	let mut grouped: BTreeMap<String, Vec<Value>> = BTreeMap::new();

	for (method, params) in requests {
		grouped
			.entry(method.clone())
			.or_default()
			.push(params.clone());
	}

	grouped
}
