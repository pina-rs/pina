use std::fs;
#[cfg(unix)]
use std::path::PathBuf;

use super::*;

fn launch<'a>(
	executable: &'a OsStr,
	directory: &'a Path,
	ready_timeout: Duration,
) -> SurfnetLaunch<'a> {
	SurfnetLaunch {
		executable,
		fork_url: "https://api.devnet.solana.com",
		ports: SurfnetPorts {
			rpc: 1,
			ws: 2,
			studio: 3,
		},
		directory,
		ready_timeout,
		request_timeout: Duration::from_secs(1),
	}
}

/// Write a fake Surfpool without this process opening it, so a concurrent
/// `fork` cannot hold it open and make `exec` fail with "Text file busy".
#[cfg(unix)]
fn script(directory: &Path, name: &str, body: &str) -> PathBuf {
	let path = directory.join(name);
	crate::test_support::write_executable(&path, &format!("#!/bin/sh\n{body}\n"))
		.unwrap_or_else(|error| panic!("write fake surfpool: {error}"));
	path
}

#[test]
fn builds_a_quiet_isolated_fork_command() {
	let directory = Path::new("scratch");
	let arguments = arguments(&launch(OsStr::new("surfpool"), directory, Duration::ZERO))
		.into_iter()
		.map(|argument| argument.to_string_lossy().into_owned())
		.collect::<Vec<_>>();

	assert_eq!(
		arguments,
		[
			"start",
			"--rpc-url",
			"https://api.devnet.solana.com",
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
			"--port",
			"1",
			"--ws-port",
			"2",
			"--studio-port",
			"3",
			"--log-path",
			&Path::new("scratch").join("logs").to_string_lossy(),
		]
	);
}

#[test]
fn allocates_distinct_loopback_ports() {
	let ports =
		SurfnetPorts::allocate().unwrap_or_else(|error| panic!("allocate loopback ports: {error}"));

	assert_ne!(ports.rpc, ports.ws);
	assert_ne!(ports.ws, ports.studio);
	assert_ne!(ports.rpc, ports.studio);
}

#[cfg(unix)]
#[test]
fn reports_an_early_exit_with_the_log_tail() {
	let directory =
		tempfile::tempdir().unwrap_or_else(|error| panic!("create surfpool directory: {error}"));
	let fake = script(
		directory.path(),
		"surfpool",
		"printf '\\033[31mERROR\\033[0m port in use\\a\\n'\nexit 3",
	);
	let error = Surfnet::start(&launch(
		fake.as_os_str(),
		directory.path(),
		Duration::from_secs(30),
	))
	.err()
	.unwrap_or_else(|| panic!("an exiting Surfpool must fail"));

	let RehearseError::SurfpoolExited { status, log } = error else {
		panic!("unexpected error: {error}");
	};
	assert!(status.contains('3'), "{status}");
	assert_eq!(log, "ERROR port in use\\u{7}");
	assert!(directory.path().join("surfpool.log").is_file());
}

#[cfg(unix)]
#[test]
fn times_out_and_kills_a_fork_that_never_answers() {
	let directory =
		tempfile::tempdir().unwrap_or_else(|error| panic!("create surfpool directory: {error}"));
	let pid_file = directory.path().join("pid");
	let fake = script(
		directory.path(),
		"surfpool",
		&format!(
			"echo $$ > '{}'\necho starting\nexec sleep 30",
			pid_file.display()
		),
	);
	// The fake never answers, so the test always waits this long. It must still
	// leave a loaded runner (coverage instrumentation, parallel tests) time to
	// start the fake and let it write its log line and pid before the deadline.
	let ready_timeout = Duration::from_secs(5);
	let error = Surfnet::start(&launch(fake.as_os_str(), directory.path(), ready_timeout))
		.err()
		.unwrap_or_else(|| panic!("a silent Surfpool must time out"));

	let RehearseError::SurfpoolNotReady { timeout, log } = error else {
		panic!("unexpected error: {error}");
	};
	assert_eq!(timeout, ready_timeout);
	assert_eq!(log, "starting");
	let pid = fs::read_to_string(&pid_file)
		.unwrap_or_else(|error| panic!("read fake surfpool pid: {error}"));
	let alive = Command::new("kill")
		.args(["-0", pid.trim()])
		.stderr(Stdio::null())
		.status()
		.unwrap_or_else(|error| panic!("probe fake surfpool: {error}"));
	assert!(!alive.success(), "the timed-out fork must be killed");
}

#[cfg(unix)]
#[test]
fn reports_a_missing_executable_and_an_unwritable_directory() {
	let directory =
		tempfile::tempdir().unwrap_or_else(|error| panic!("create surfpool directory: {error}"));
	let missing = directory.path().join("missing-surfpool");
	let absent = directory.path().join("absent");
	let error = Surfnet::start(&launch(
		missing.as_os_str(),
		directory.path(),
		Duration::from_secs(30),
	))
	.err()
	.unwrap_or_else(|| panic!("a missing Surfpool must fail"));

	let RehearseError::SurfpoolExited { status, log } = error else {
		panic!("unexpected error: {error}");
	};
	assert!(status.contains("127"), "{status}");
	assert!(log.contains("missing-surfpool"), "{log}");
	assert!(matches!(
		Surfnet::start(&launch(missing.as_os_str(), &absent, Duration::ZERO)),
		Err(RehearseError::WorkDirectory(_))
	));
}

#[cfg(unix)]
#[test]
fn the_lifeline_stops_surfpool_when_its_owner_disappears() {
	let directory =
		tempfile::tempdir().unwrap_or_else(|error| panic!("create surfpool directory: {error}"));
	let pid_file = directory.path().join("pid");
	let fake = script(
		directory.path(),
		"surfpool",
		&format!("echo $$ > '{}'\nexec sleep 30", pid_file.display()),
	);
	let mut supervisor = supervised(fake.as_os_str())
		.stdout(Stdio::null())
		.stderr(Stdio::null())
		.spawn()
		.unwrap_or_else(|error| panic!("start the supervisor: {error}"));
	// Generous, because a loaded runner can be slow to start two shells; the
	// loop returns as soon as the pid appears.
	let start_timeout = Duration::from_secs(30);
	let deadline = Instant::now() + start_timeout;
	let pid = loop {
		let pid = fs::read_to_string(&pid_file).unwrap_or_default();

		if !pid.trim().is_empty() {
			break pid.trim().to_owned();
		}

		assert!(Instant::now() < deadline, "the fake Surfpool never started");
		std::thread::sleep(Duration::from_millis(20));
	};
	let alive = || {
		Command::new("kill")
			.args(["-0", &pid])
			.stderr(Stdio::null())
			.status()
			.is_ok_and(|status| status.success())
	};
	assert!(alive());

	// A terminal interrupt reaches the whole group; neither may die from it.
	let interrupted = Command::new("kill")
		.args(["-INT", &supervisor.id().to_string(), &pid])
		.status()
		.unwrap_or_else(|error| panic!("interrupt the supervisor: {error}"));
	assert!(interrupted.success());
	std::thread::sleep(Duration::from_millis(200));
	assert!(alive(), "Surfpool must not die from an interrupt");

	// Losing the owner, as `pina` dying would, closes the lifeline. The
	// supervisor exits with Surfpool's status, so 137 (128 + SIGKILL) shows the
	// lifeline killed it; an interrupt that had killed it would report 130.
	drop(supervisor.stdin.take());
	let status = supervisor
		.wait()
		.unwrap_or_else(|error| panic!("wait for the supervisor: {error}"));
	assert_eq!(status.code(), Some(137), "{status}");
	assert!(!alive(), "the supervisor must kill Surfpool");
}

#[test]
fn log_tails_are_bounded_and_tolerate_missing_logs() {
	let directory =
		tempfile::tempdir().unwrap_or_else(|error| panic!("create log directory: {error}"));
	let log = directory.path().join("surfpool.log");
	let lines = (0..500)
		.map(|index| format!("line {index}"))
		.collect::<Vec<_>>()
		.join("\n");
	fs::write(&log, lines).unwrap_or_else(|error| panic!("write log: {error}"));
	let tail = log_tail(&log);

	assert_eq!(tail.lines().count(), LOG_TAIL_LINES);
	assert!(tail.ends_with("line 499"));
	assert_eq!(
		log_tail(&directory.path().join("absent.log")),
		"(Surfpool wrote no readable log)"
	);
	assert_eq!(clean_log_line("\u{1b}[1;32mok\u{1b}[0m\t"), "ok\\t");
	assert_eq!(clean_log_line("\u{1b}x"), "\\u{1b}x");
}
