//! Parent-side transport for the private rehearsal worker.

use std::cell::Cell;
use std::cell::RefCell;
use std::io::BufRead as _;
use std::io::BufReader;
use std::io::Read as _;
use std::io::Write as _;
use std::path::Path;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::sync::mpsc::Sender;
use std::sync::mpsc::{self};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::Value;
use serde_json::json;

use super::rpc::ForkRpc;
use super::rpc::RpcError;

const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;
const MAX_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;
type Exchange = (String, Sender<Result<String, String>>);

pub(crate) struct PipeRpc {
	child: RefCell<Child>,
	requests: Sender<Exchange>,
	io_thread: Option<JoinHandle<()>>,
	next_id: Cell<u64>,
	timeout: Duration,
}

impl PipeRpc {
	pub(crate) fn start(
		executable: &Path,
		url: &str,
		ready_timeout: Duration,
		timeout: Duration,
	) -> Result<Self, RpcError> {
		let mut command = Command::new(executable);
		command.arg("__pina-rehearse-worker");
		Self::start_command(command, url, ready_timeout, timeout)
	}

	fn start_command(
		mut command: Command,
		url: &str,
		ready_timeout: Duration,
		timeout: Duration,
	) -> Result<Self, RpcError> {
		let mut child = command
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.stderr(Stdio::null())
			.spawn()
			.map_err(|error| RpcError::Transport(error.to_string()))?;
		let mut stdin = child
			.stdin
			.take()
			.ok_or_else(|| RpcError::Transport("worker input unavailable".to_owned()))?;
		let stdout = child
			.stdout
			.take()
			.ok_or_else(|| RpcError::Transport("worker output unavailable".to_owned()))?;
		let (requests, receiver) = mpsc::channel::<Exchange>();
		let io_thread = std::thread::spawn(move || {
			let mut output = BufReader::new(stdout);
			for (frame, reply) in receiver {
				let response = (|| -> Result<String, String> {
					stdin
						.write_all(frame.as_bytes())
						.and_then(|_| stdin.write_all(b"\n"))
						.and_then(|_| stdin.flush())
						.map_err(|error| error.to_string())?;
					let mut bytes = Vec::new();
					let count = (&mut output)
						.take(MAX_RESPONSE_BYTES + 2)
						.read_until(b'\n', &mut bytes)
						.map_err(|error| error.to_string())?;
					if count == 0
						|| bytes.last() != Some(&b'\n')
						|| bytes.len() as u64 > MAX_RESPONSE_BYTES + 1
					{
						return Err("worker response is missing or too large".to_owned());
					}
					String::from_utf8(bytes).map_err(|_| "worker response is not UTF-8".to_owned())
				})();
				let failed = response.is_err();
				let _ = reply.send(response);
				if failed {
					break;
				}
			}
		});
		let transport = Self {
			child: RefCell::new(child),
			requests,
			io_thread: Some(io_thread),
			next_id: Cell::new(1),
			timeout,
		};
		let response = transport
			.exchange(json!({"url": url}).to_string(), ready_timeout)
			.map_err(|error| {
				RpcError::Transport(format!(
					"could not start a compatible Pina rehearsal worker: {error}"
				))
			})?;
		let ready: Value =
			serde_json::from_str(&response).map_err(|error| RpcError::Body(error.to_string()))?;
		if let Some(error) = ready["error"].as_str() {
			return Err(RpcError::Transport(error.to_owned()));
		}
		if ready["ready"] != true
			|| ready["protocol"] != 1
			|| ready["pinaVersion"] != env!("CARGO_PKG_VERSION")
		{
			return Err(RpcError::Shape(
				"worker is incompatible or did not become ready".to_owned(),
			));
		}
		Ok(transport)
	}

	fn exchange(&self, request: String, timeout: Duration) -> Result<String, RpcError> {
		if request.len() + 1 > MAX_REQUEST_BYTES {
			return Err(RpcError::Body(
				"worker request exceeds the size limit".to_owned(),
			));
		}
		let (reply, receiver) = mpsc::channel();
		self.requests
			.send((request, reply))
			.map_err(|_| RpcError::Transport("worker has exited".to_owned()))?;
		match receiver.recv_timeout(timeout) {
			Ok(result) => result.map_err(RpcError::Transport),
			Err(mpsc::RecvTimeoutError::Timeout) => {
				let _ = self.child.borrow_mut().kill();
				Err(RpcError::Transport("worker request timed out".to_owned()))
			}
			Err(mpsc::RecvTimeoutError::Disconnected) => {
				Err(RpcError::Transport("worker has exited".to_owned()))
			}
		}
	}

	pub(crate) fn call(&self, method: &str, params: &Value) -> Result<Value, RpcError> {
		let id = self.next_id.get();
		self.next_id.set(id + 1);
		let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
		let response = self.exchange(request.to_string(), self.timeout)?;
		let mut value: Value =
			serde_json::from_str(&response).map_err(|error| RpcError::Body(error.to_string()))?;
		if value["jsonrpc"] != "2.0" || value["id"].as_u64() != Some(id) {
			return Err(RpcError::Shape(
				"worker response has an unexpected ID or protocol version".to_owned(),
			));
		}
		if let Some(error) = value.get("error") {
			return Err(RpcError::Rpc {
				code: error
					.get("code")
					.and_then(Value::as_i64)
					.unwrap_or_default(),
				message: error
					.get("message")
					.and_then(Value::as_str)
					.unwrap_or("no message")
					.to_owned(),
			});
		}
		value
			.get_mut("result")
			.map(Value::take)
			.ok_or_else(|| RpcError::Shape("the response has no result".to_owned()))
	}
}

impl ForkRpc for PipeRpc {
	fn call(&self, method: &str, params: &Value) -> Result<Value, RpcError> {
		PipeRpc::call(self, method, params)
	}
}

impl Drop for PipeRpc {
	fn drop(&mut self) {
		let child = self.child.get_mut();
		let _ = child.kill();
		let _ = child.wait();
		// Closing the channel lets an idle I/O thread exit after the child dies.
		let (replacement, _) = mpsc::channel();
		drop(std::mem::replace(&mut self.requests, replacement));
		if let Some(thread) = self.io_thread.take() {
			let _ = thread.join();
		}
	}
}

#[cfg(all(test, unix))]
mod tests {
	use super::*;

	fn fake(script: &str, timeout: Duration) -> Result<PipeRpc, RpcError> {
		let mut command = Command::new("/bin/sh");
		command
			.arg("-c")
			.arg(script.replace("__PINA_VERSION__", env!("CARGO_PKG_VERSION")));
		PipeRpc::start_command(
			command,
			"http://localhost",
			Duration::from_secs(30),
			timeout,
		)
	}

	#[test]
	fn correlates_response_with_request() {
		let rpc = fake(
			r#"read -r init
printf '%s\n' '{"ready":true,"protocol":1,"pinaVersion":"__PINA_VERSION__"}'
read -r request
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"ok":true}}'
"#,
			Duration::from_secs(5),
		)
		.expect("start fake worker");
		let result = rpc
			.call("getAccountInfo", &json!([]))
			.expect("worker result");
		assert_eq!(result["ok"], true);
	}

	#[test]
	fn rejects_wrong_response_id() {
		let rpc = fake(
			r#"read -r init
printf '%s\n' '{"ready":true,"protocol":1,"pinaVersion":"__PINA_VERSION__"}'
read -r request
printf '%s\n' '{"jsonrpc":"2.0","id":99,"result":null}'
"#,
			Duration::from_secs(5),
		)
		.expect("start fake worker");
		assert!(matches!(
			rpc.call("getAccountInfo", &json!([])),
			Err(RpcError::Shape(_))
		));
	}

	#[test]
	fn times_out_and_reaps_a_stalled_worker() {
		let rpc = fake(
			r#"read -r init
printf '%s\n' '{"ready":true,"protocol":1,"pinaVersion":"__PINA_VERSION__"}'
read -r request
exec sleep 30
"#,
			Duration::from_millis(100),
		)
		.expect("start fake worker");
		let pid = rpc.child.borrow().id();
		assert!(matches!(
			rpc.call("getAccountInfo", &json!([])),
			Err(RpcError::Transport(_))
		));
		drop(rpc);
		let status = Command::new("/bin/sh")
			.args([
				"-c",
				"kill -0 \"$1\" 2>/dev/null",
				"probe",
				&pid.to_string(),
			])
			.status()
			.expect("probe worker");
		assert!(!status.success(), "timed-out worker must be reaped");
	}

	#[test]
	fn rejects_oversized_requests_without_sending_them() {
		let rpc = fake(
			r#"read -r init
printf '%s\n' '{"ready":true,"protocol":1,"pinaVersion":"__PINA_VERSION__"}'
read -r request
"#,
			Duration::from_secs(5),
		)
		.expect("start fake worker");
		assert!(matches!(
			rpc.call("getAccountInfo", &json!(["x".repeat(MAX_REQUEST_BYTES)])),
			Err(RpcError::Body(_))
		));
	}
}
