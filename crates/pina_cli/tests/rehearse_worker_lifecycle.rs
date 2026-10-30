//! Portable lifecycle checks for the private rehearsal worker.

use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

#[path = "support/rehearse.rs"]
mod rehearse;

#[test]
fn worker_reports_datasource_initialization_failure() -> Result<(), Box<dyn std::error::Error>> {
	use std::io::BufRead as _;
	use std::io::Write as _;

	let remote = rehearse::FakeRpcServer::start(|_, _| {
		rehearse::Reply::Error(-32000, "datasource unavailable".to_owned())
	});
	let mut worker = Command::new(env!("CARGO_BIN_EXE_pina"))
		.arg("__pina-rehearse-worker")
		.env("NO_PROXY", "127.0.0.1")
		.env("no_proxy", "127.0.0.1")
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::null())
		.spawn()?;
	let mut input = worker.stdin.take().ok_or("missing worker stdin")?;
	let mut output = std::io::BufReader::new(worker.stdout.take().ok_or("missing worker stdout")?);
	writeln!(input, "{}", serde_json::json!({"url": remote.url}))?;
	let mut response = String::new();
	output.read_line(&mut response)?;
	let response: serde_json::Value = serde_json::from_str(&response)?;
	assert!(
		response["error"]
			.as_str()
			.is_some_and(|error| error.contains("could not initialize the rehearsal fork")),
		"worker response: {response}"
	);
	let status = worker.wait()?;
	drop(input);
	assert!(
		!status.success(),
		"failed initialization must exit unsuccessfully"
	);
	Ok(())
}

#[test]
fn worker_rejects_an_oversized_frame() -> Result<(), Box<dyn std::error::Error>> {
	use std::io::Write as _;

	let mut worker = Command::new(env!("CARGO_BIN_EXE_pina"))
		.arg("__pina-rehearse-worker")
		.stdin(Stdio::piped())
		.stdout(Stdio::null())
		.stderr(Stdio::null())
		.spawn()?;
	let mut input = worker.stdin.take().ok_or("missing worker stdin")?;
	input.write_all(&vec![b'x'; 4 * 1024 * 1024 + 1])?;
	let status = worker.wait()?;
	drop(input);
	assert!(
		!status.success(),
		"oversized frames must terminate the worker"
	);
	Ok(())
}

#[test]
fn worker_initializes_and_handles_private_fork_requests() -> Result<(), Box<dyn std::error::Error>>
{
	use std::io::BufRead as _;
	use std::io::Write as _;

	use serde_json::Value;
	use serde_json::json;

	let remote = rehearse::FakeRpcServer::start(|method, _| {
		let result = match method {
			"getEpochInfo" => {
				json!({
					"epoch": 0, "slotIndex": 1, "slotsInEpoch": 432000,
					"absoluteSlot": 1, "blockHeight": 1, "transactionCount": 0
				})
			}
			"getEpochSchedule" => {
				json!({
					"slotsPerEpoch": 432000, "leaderScheduleSlotOffset": 432000,
					"warmup": false, "firstNormalEpoch": 0, "firstNormalSlot": 0
				})
			}
			"getGenesisHash" => json!("11111111111111111111111111111111"),
			"getAccountInfo" => json!({"context": {"slot": 1}, "value": null}),
			_ => return rehearse::Reply::Error(-32601, format!("unexpected method {method}")),
		};
		rehearse::Reply::Result(result)
	});
	let mut worker = Command::new(env!("CARGO_BIN_EXE_pina"))
		.arg("__pina-rehearse-worker")
		.env("NO_PROXY", "127.0.0.1")
		.env("no_proxy", "127.0.0.1")
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::null())
		.spawn()?;
	let mut input = worker.stdin.take().ok_or("missing worker stdin")?;
	let mut output = std::io::BufReader::new(worker.stdout.take().ok_or("missing worker stdout")?);
	writeln!(input, "{}", json!({"url": remote.url}))?;
	let mut response = String::new();
	output.read_line(&mut response)?;
	assert_eq!(serde_json::from_str::<Value>(&response)?["ready"], true);

	let address = "11111111111111111111111111111112";
	for (id, method, params) in [
		(
			1,
			"surfnet_setAccount",
			json!([address, {"lamports": 1234567}]),
		),
		(
			2,
			"getAccountInfo",
			json!([address, {"encoding": "base64"}]),
		),
	] {
		writeln!(
			input,
			"{}",
			json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
		)?;
		response.clear();
		output.read_line(&mut response)?;
		let value: Value = serde_json::from_str(&response)?;
		assert!(value.get("error").is_none(), "worker response: {value}");
		if method == "getAccountInfo" {
			assert_eq!(value["result"]["value"]["lamports"], 1234567);
		}
	}
	assert!(remote.methods().contains(&"getEpochInfo".to_owned()));
	drop(input);
	let status = worker.wait()?;
	assert!(!status.success(), "worker exits on owner pipe closure");
	Ok(())
}

#[test]
fn worker_exits_when_its_owner_closes_the_pipe() {
	let mut worker = Command::new(env!("CARGO_BIN_EXE_pina"))
		.arg("__pina-rehearse-worker")
		.stdin(Stdio::piped())
		.stdout(Stdio::null())
		.stderr(Stdio::null())
		.spawn()
		.expect("start private rehearsal worker");
	let input = worker.stdin.take().expect("worker input is piped");
	drop(input);

	let deadline = Instant::now() + Duration::from_secs(15);
	loop {
		if worker.try_wait().expect("wait for worker").is_some() {
			break;
		}

		if Instant::now() >= deadline {
			let _ = worker.kill();
			let _ = worker.wait();
			panic!("worker did not exit after its input closed");
		}

		std::thread::sleep(Duration::from_millis(20));
	}
}

#[cfg(windows)]
#[test]
fn worker_exits_after_its_owner_is_killed_during_startup() -> Result<(), Box<dyn std::error::Error>>
{
	use std::io::BufRead as _;
	use std::io::Write as _;
	use std::net::TcpListener;
	use std::sync::mpsc;

	const OWNER_URL: &str = "PINA_TEST_WORKER_OWNER_URL";
	const PID_PREFIX: &str = "REHEARSAL_WORKER_PID=";

	if let Some(url) = std::env::var_os(OWNER_URL) {
		let mut worker = Command::new(env!("CARGO_BIN_EXE_pina"))
			.arg("__pina-rehearse-worker")
			.stdin(Stdio::piped())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.spawn()?;
		let mut input = worker.stdin.take().ok_or("worker has no stdin")?;
		writeln!(
			input,
			"{}",
			serde_json::json!({"url": url.to_string_lossy()})
		)?;
		println!("{PID_PREFIX}{}", worker.id());
		std::io::stdout().flush()?;
		let result = worker.wait();
		drop(input);
		result?;
		return Err("worker exited before its owner was killed".into());
	}

	let listener = TcpListener::bind("127.0.0.1:0")?;
	listener.set_nonblocking(true)?;
	let mut owner = Command::new(std::env::current_exe()?)
		.args([
			"--exact",
			"worker_exits_after_its_owner_is_killed_during_startup",
			"--nocapture",
		])
		.env(OWNER_URL, format!("http://{}", listener.local_addr()?))
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
	let mut worker_pid = None;
	let result = (|| -> Result<(), Box<dyn std::error::Error>> {
		let pid = pid_rx.recv_timeout(Duration::from_secs(20))??;
		worker_pid = Some(pid);
		let deadline = Instant::now() + Duration::from_secs(20);
		let _connection = loop {
			match listener.accept() {
				Ok((stream, _)) => break stream,
				Err(error)
					if error.kind() == std::io::ErrorKind::WouldBlock
						&& Instant::now() < deadline =>
				{
					std::thread::sleep(Duration::from_millis(25));
				}
				Err(error) => return Err(error.into()),
			}
		};
		owner.kill()?;
		owner.wait()?;
		let deadline = Instant::now() + Duration::from_secs(15);
		loop {
			let status = Command::new("powershell.exe")
				.args(["-NoProfile", "-NonInteractive", "-Command"])
				.arg(format!(
					"if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ exit 0 }} else {{ exit 11 }}"
				))
				.stdout(Stdio::null())
				.stderr(Stdio::null())
				.status()?;
			match status.code() {
				Some(11) => return Ok(()),
				Some(0) => {}
				_ => return Err("could not inspect worker process".into()),
			}
			if Instant::now() >= deadline {
				return Err("worker survived its owner during a blocked operation".into());
			}
			std::thread::sleep(Duration::from_millis(50));
		}
	})();
	let _ = owner.kill();
	let _ = owner.wait();
	if result.is_err() {
		if let Some(pid) = worker_pid {
			let _ = Command::new("taskkill.exe")
				.args(["/PID", &pid.to_string(), "/T", "/F"])
				.status();
		}
	}
	result
}
