//! The Surfpool fork a rehearsal owns.
//!
//! Surfpool runs as an external process so `pina` never links the SVM. The
//! fork binds only loopback ports chosen for this run and writes its logs to
//! the rehearsal's temporary directory.
//!
//! No exit path may leave it running. Dropping [`Surfnet`] stops and reaps it,
//! which covers returns, errors, and panics. That is not enough on its own: a
//! Ctrl-C or `kill` terminates `pina` without running destructors, and a
//! Surfpool that is still starting ignores the interrupt the terminal sends
//! to the whole process group. On Unix, Surfpool therefore runs under a small
//! `/bin/sh` supervisor whose standard input is a pipe only `pina` holds. When
//! `pina` exits for any reason, even `SIGKILL`, the operating system closes
//! the pipe and the supervisor kills Surfpool. The supervisor and Surfpool
//! ignore interrupts and hangups, so cleanup always runs through that one
//! path. Elsewhere Surfpool is a direct child stopped by [`Surfnet`]'s
//! destructor.

use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::io::Read as _;
use std::io::Seek as _;
use std::io::SeekFrom;
use std::net::TcpListener;
use std::path::Path;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use serde_json::json;

use super::RehearseError;
use super::rpc::JsonRpc;

/// How long a single readiness probe may take.
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
/// Pause between readiness probes.
const PROBE_INTERVAL: Duration = Duration::from_millis(100);
/// Bytes read from the end of Surfpool's log for a diagnostic.
const LOG_TAIL_BYTES: u64 = 4096;
/// Lines kept from the end of Surfpool's log for a diagnostic.
const LOG_TAIL_LINES: usize = 20;

/// The Unix supervisor, run as `sh -c SUPERVISOR <surfpool> <arguments>...`.
///
/// An asynchronous list in a non-interactive shell reads `/dev/null` unless
/// redirected, so the watcher reads the lifeline through a duplicate of the
/// supervisor's standard input. The supervisor exits with Surfpool's status,
/// so a Surfpool that fails to start is still noticed immediately.
#[cfg(unix)]
const SUPERVISOR: &str = r#"trap '' INT HUP TERM
exec 3<&0
"$0" "$@" </dev/null 3<&- &
surfpool=$!
(read -r lifeline <&3; kill -KILL "$surfpool" 2>/dev/null) &
watcher=$!
wait "$surfpool"
status=$?
kill -KILL "$watcher" 2>/dev/null
exit "$status""#;

/// Loopback ports for one Surfpool instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SurfnetPorts {
	pub(crate) rpc: u16,
	pub(crate) ws: u16,
	pub(crate) studio: u16,
}

impl SurfnetPorts {
	/// Ask the OS for three distinct free loopback ports.
	///
	/// The listeners are held together so the ports differ, then released for
	/// Surfpool to bind. Another process could take one in between; Surfpool
	/// then fails to start and the rehearsal reports it.
	pub(crate) fn allocate() -> io::Result<Self> {
		let listeners = [
			TcpListener::bind("127.0.0.1:0")?,
			TcpListener::bind("127.0.0.1:0")?,
			TcpListener::bind("127.0.0.1:0")?,
		];
		let port = |index: usize| listeners[index].local_addr().map(|address| address.port());

		Ok(Self {
			rpc: port(0)?,
			ws: port(1)?,
			studio: port(2)?,
		})
	}
}

/// Everything needed to start a fork.
pub(crate) struct SurfnetLaunch<'a> {
	pub(crate) executable: &'a OsStr,
	pub(crate) fork_url: &'a str,
	pub(crate) ports: SurfnetPorts,
	/// Private scratch directory: Surfpool's working directory and log home.
	pub(crate) directory: &'a Path,
	pub(crate) ready_timeout: Duration,
	/// Per-request timeout for the rehearsal's own calls.
	pub(crate) request_timeout: Duration,
}

/// A running fork, stopped when dropped.
pub(crate) struct Surfnet {
	child: Child,
	rpc: JsonRpc,
}

impl Surfnet {
	/// Start Surfpool and wait until its RPC answers.
	pub(crate) fn start(launch: &SurfnetLaunch<'_>) -> Result<Self, RehearseError> {
		let log = launch.directory.join("surfpool.log");
		let stdout = File::create(&log).map_err(RehearseError::WorkDirectory)?;
		let stderr = stdout.try_clone().map_err(RehearseError::WorkDirectory)?;
		let child = supervised(launch.executable)
			.current_dir(launch.directory)
			.args(arguments(launch))
			.stdout(stdout)
			.stderr(stderr)
			.spawn()
			.map_err(RehearseError::StartSurfpool)?;
		let url = format!("http://127.0.0.1:{}", launch.ports.rpc);
		let mut surfnet = Self {
			child,
			rpc: JsonRpc::new(&url, launch.request_timeout, true),
		};
		surfnet.wait_until_ready(&url, launch.ready_timeout, &log)?;

		Ok(surfnet)
	}

	/// The fork's RPC client.
	pub(crate) fn rpc(&self) -> &JsonRpc {
		&self.rpc
	}

	fn wait_until_ready(
		&mut self,
		url: &str,
		timeout: Duration,
		log: &Path,
	) -> Result<(), RehearseError> {
		let probe = JsonRpc::new(url, PROBE_TIMEOUT, true);
		let deadline = Instant::now() + timeout;

		loop {
			if let Some(status) = self
				.child
				.try_wait()
				.map_err(RehearseError::StartSurfpool)?
			{
				return Err(RehearseError::SurfpoolExited {
					status: status.to_string(),
					log: log_tail(log),
				});
			}

			if probe.call("getHealth", &json!([])).is_ok() {
				return Ok(());
			}

			if Instant::now() >= deadline {
				return Err(RehearseError::SurfpoolNotReady {
					timeout,
					log: log_tail(log),
				});
			}

			std::thread::sleep(PROBE_INTERVAL);
		}
	}
}

impl Drop for Surfnet {
	fn drop(&mut self) {
		// On Unix, closing the lifeline makes the supervisor kill Surfpool, and
		// the supervisor exits only after reaping it. Waiting therefore
		// guarantees Surfpool is gone and its ports are released.
		drop(self.child.stdin.take());

		#[cfg(not(unix))]
		let _ = self.child.kill();

		let _ = self.child.wait();
	}
}

/// The command that runs Surfpool, under the lifeline supervisor on Unix.
#[cfg(unix)]
fn supervised(executable: &OsStr) -> Command {
	let mut command = Command::new("/bin/sh");
	command
		.arg("-c")
		.arg(SUPERVISOR)
		.arg(executable)
		.stdin(Stdio::piped());
	command
}

#[cfg(not(unix))]
fn supervised(executable: &OsStr) -> Command {
	let mut command = Command::new(executable);
	command.stdin(Stdio::null());
	command
}

/// Surfpool arguments for a quiet, deterministic fork.
///
/// - `--skip-blockhash-check`: replayed transactions carry blockhashes that
///   expired long ago, and profiling validates blockhash age.
/// - `--block-production-mode manual`: the clock never advances, so the
///   baseline and candidate runs observe the same `Clock` sysvar.
/// - `--no-deploy` with a private working directory: no runbook is read,
///   created, or prompted for, and nothing is written into the project.
/// - `--airdrop-amount 0`: no account is funded, so forked state stays exact.
/// - The studio port is set even though the studio is disabled, because
///   Surfpool 1.6 still binds it.
fn arguments(launch: &SurfnetLaunch<'_>) -> Vec<OsString> {
	let mut arguments: Vec<OsString> = [
		"start",
		"--rpc-url",
		launch.fork_url,
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
	]
	.into_iter()
	.map(OsString::from)
	.collect();

	for (flag, port) in [
		("--port", launch.ports.rpc),
		("--ws-port", launch.ports.ws),
		("--studio-port", launch.ports.studio),
	] {
		arguments.push(OsString::from(flag));
		arguments.push(OsString::from(port.to_string()));
	}

	arguments.push(OsString::from("--log-path"));
	arguments.push(launch.directory.join("logs").into_os_string());
	arguments
}

/// The end of Surfpool's log, with terminal escapes removed, for diagnostics.
fn log_tail(path: &Path) -> String {
	let Ok(text) = read_tail(path) else {
		return "(Surfpool wrote no readable log)".to_owned();
	};
	let lines = text.lines().map(clean_log_line).collect::<Vec<_>>();

	lines[lines.len().saturating_sub(LOG_TAIL_LINES)..].join("\n")
}

fn read_tail(path: &Path) -> io::Result<String> {
	let mut file = File::open(path)?;
	let length = file.metadata()?.len();
	file.seek(SeekFrom::Start(length.saturating_sub(LOG_TAIL_BYTES)))?;
	let mut bytes = Vec::new();
	file.read_to_end(&mut bytes)?;

	Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Drop ANSI escape sequences and escape any other control character, so a
/// log line can never drive the user's terminal.
fn clean_log_line(line: &str) -> String {
	let mut cleaned = String::with_capacity(line.len());
	let mut characters = line.chars().peekable();

	while let Some(character) = characters.next() {
		if character == '\u{1b}' && characters.peek() == Some(&'[') {
			characters.next();

			for terminator in characters.by_ref() {
				if ('\u{40}'..='\u{7e}').contains(&terminator) {
					break;
				}
			}

			continue;
		}

		if character.is_control() {
			cleaned.extend(character.escape_default());
		} else {
			cleaned.push(character);
		}
	}

	cleaned
}

#[cfg(test)]
#[path = "surfnet_tests.rs"]
mod tests;
