//! Portable lifecycle checks for the private rehearsal worker.

use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

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
