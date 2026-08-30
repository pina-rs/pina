//! Deterministic project and toolchain diagnostics for humans and agents.

use std::fmt::Write;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use serde::Serialize;

use crate::keys::inspect_program_id;
use crate::keys::read_keypair_program_id;
use crate::project::ClientLanguage;
use crate::project::Project;

/// Versioned JSON schema emitted by `pina doctor --json`.
pub const DOCTOR_SCHEMA_VERSION: u8 = 1;

/// Overall diagnostic severity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DoctorStatus {
	/// Required inputs are available and no recommendations remain.
	Ok,
	/// The core toolchain works, but optional capabilities are missing.
	Warning,
	/// A required tool or valid Pina program project is missing.
	Error,
}

/// Status of one stable diagnostic check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
	/// The capability is ready.
	Pass,
	/// The capability is optional or has an actionable recommendation.
	Warn,
	/// A baseline requirement failed.
	Fail,
}

/// One agent-addressable diagnostic result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorCheck {
	/// Stable identifier suitable for automation rules.
	pub id: String,
	/// Typed severity for this check.
	pub status: CheckStatus,
	/// Concise human-readable result without ANSI styling.
	pub message: String,
}

/// A discovered external tool.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDiagnostic {
	/// Executable name searched through `PATH`.
	pub name: &'static str,
	/// Whether the CLI requires the tool for baseline Rust development.
	pub required: bool,
	/// Whether the executable ran successfully.
	pub available: bool,
	/// First line returned by the version command.
	pub version: Option<String>,
}

/// Project paths and identity diagnosed by `pina doctor`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDiagnostic {
	/// Absolute package root.
	pub root: PathBuf,
	/// Cargo package name.
	pub package_name: String,
	/// Rust library entrypoint.
	pub source: PathBuf,
	/// Program ID found in source, when readable.
	pub program_id: Option<String>,
	/// Conventional SBF artifact path.
	pub artifact: PathBuf,
	/// Whether the SBF artifact exists.
	pub artifact_exists: bool,
	/// Conventional local program keypair path.
	pub keypair: PathBuf,
	/// Whether the keypair exists.
	pub keypair_exists: bool,
	/// Whether the keypair identity matches the source declaration.
	pub keypair_matches_source: Option<bool>,
	/// Client ecosystems selected by project configuration.
	pub clients: Vec<ClientLanguage>,
}

/// Stable diagnostic report emitted by `pina doctor`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
	/// Report schema version for agent compatibility.
	pub schema_version: u8,
	/// Pina CLI package version.
	pub cli_version: &'static str,
	/// Aggregate severity.
	pub status: DoctorStatus,
	/// Project information, absent when discovery fails.
	pub project: Option<ProjectDiagnostic>,
	/// External tool availability in stable display order.
	pub tools: Vec<ToolDiagnostic>,
	/// Typed checks with stable IDs for agent decisions.
	pub checks: Vec<DoctorCheck>,
	/// Actionable findings in stable display order.
	pub findings: Vec<String>,
}

impl DoctorReport {
	/// Whether this report should produce a successful process exit.
	pub fn is_usable(&self) -> bool {
		self.status != DoctorStatus::Error
	}

	/// Render stable, color-free terminal output.
	pub fn render_text(&self) -> String {
		let mut output = String::new();
		let _ = writeln!(output, "Pina doctor");
		let _ = writeln!(output, "CLI: {}", escape_controls(self.cli_version));

		if let Some(project) = &self.project {
			let root = project.root.to_string_lossy();
			let artifact = project.artifact.to_string_lossy();
			let keypair = project.keypair.to_string_lossy();
			let _ = writeln!(output, "Project: {}", escape_controls(&root));
			let _ = writeln!(
				output,
				"Package: {}",
				escape_controls(&project.package_name)
			);
			let program_id = project.program_id.as_deref().unwrap_or("unavailable");
			let _ = writeln!(output, "Program ID: {}", escape_controls(program_id));
			let _ = writeln!(
				output,
				"Artifact: {} ({})",
				escape_controls(&artifact),
				presence(project.artifact_exists)
			);
			let _ = writeln!(
				output,
				"Keypair: {} ({})",
				escape_controls(&keypair),
				presence(project.keypair_exists)
			);
		} else {
			let _ = writeln!(output, "Project: unavailable");
		}

		let _ = writeln!(output, "Tools:");

		for tool in &self.tools {
			let requirement = if tool.required {
				"required"
			} else {
				"optional"
			};
			let version = tool.version.as_deref().unwrap_or("missing");
			let _ = writeln!(
				output,
				"  {} ({requirement}): {}",
				escape_controls(tool.name),
				escape_controls(version)
			);
		}

		let _ = writeln!(output, "Checks:");

		for check in &self.checks {
			let status = match check.status {
				CheckStatus::Pass => "pass",
				CheckStatus::Warn => "warn",
				CheckStatus::Fail => "fail",
			};
			let _ = writeln!(
				output,
				"  [{status}] {}: {}",
				escape_controls(&check.id),
				escape_controls(&check.message)
			);
		}

		if !self.findings.is_empty() {
			let _ = writeln!(output, "Findings:");

			for finding in &self.findings {
				let _ = writeln!(output, "  - {}", escape_controls(finding));
			}
		}

		let status = match self.status {
			DoctorStatus::Ok => "ok",
			DoctorStatus::Warning => "warning",
			DoctorStatus::Error => "error",
		};
		let _ = writeln!(output, "Status: {status}");

		output
	}
}

/// Diagnose the current Pina project and relevant ecosystem tools.
pub fn diagnose(start: &Path) -> DoctorReport {
	let mut findings = Vec::new();
	let mut checks = Vec::new();
	let project = diagnose_project(start, &mut findings, &mut checks);
	let needs_node = project.as_ref().is_some_and(|project| {
		project
			.clients
			.iter()
			.any(|client| *client != ClientLanguage::Rust)
	});
	let tools = tool_specs(needs_node)
		.iter()
		.map(|spec| diagnose_tool(*spec))
		.collect::<Vec<_>>();

	for tool in &tools {
		checks.push(DoctorCheck {
			id: format!("tool.{}", tool.name),
			status: if tool.available {
				CheckStatus::Pass
			} else if tool.required {
				CheckStatus::Fail
			} else {
				CheckStatus::Warn
			},
			message: tool_message(tool),
		});
	}

	if needs_node {
		let renderer_available = tools
			.iter()
			.any(|tool| matches!(tool.name, "npx" | "pnpm") && tool.available);
		checks.push(DoctorCheck {
			id: "client.renderer".to_owned(),
			status: if renderer_available {
				CheckStatus::Pass
			} else {
				CheckStatus::Fail
			},
			message: if renderer_available {
				"an npx or pnpm renderer is available".to_owned()
			} else {
				"configured TypeScript or Dart clients require npx or pnpm".to_owned()
			},
		});
	}

	let nightly_available = command_status(Command::new("cargo").args(["-Z", "help"]))
		.is_ok_and(|status| status.success());
	checks.push(DoctorCheck {
		id: "rust.unstable-flags".to_owned(),
		status: if nightly_available {
			CheckStatus::Pass
		} else {
			CheckStatus::Fail
		},
		message: if nightly_available {
			"cargo accepts -Z build-std workflows".to_owned()
		} else {
			"cargo does not accept the nightly -Z flags required for SBF builds".to_owned()
		},
	});
	let rust_src_available = rust_src_available();
	checks.push(DoctorCheck {
		id: "rust.rust-src".to_owned(),
		status: if rust_src_available {
			CheckStatus::Pass
		} else {
			CheckStatus::Fail
		},
		message: if rust_src_available {
			"the active Rust toolchain includes rust-src for -Z build-std".to_owned()
		} else {
			"the active Rust toolchain is missing the rust-src component".to_owned()
		},
	});

	let has_failures = checks.iter().any(|check| check.status == CheckStatus::Fail);
	let missing_required_tools = tools.iter().any(|tool| tool.required && !tool.available)
		|| !nightly_available
		|| !rust_src_available;
	let missing_optional_tools = tools
		.iter()
		.any(|tool| !tool.required && !matches!(tool.name, "npx" | "pnpm") && !tool.available);

	if missing_required_tools {
		findings.push(
			"install the missing Rust/SBF prerequisites and ensure they are on PATH".to_owned(),
		);
	}

	if missing_optional_tools {
		findings.push(
			"optional Solana, Surfpool, and client tools unlock deployment and integration \
			 workflows"
				.to_owned(),
		);
	}

	let status = if project.is_none() || has_failures {
		DoctorStatus::Error
	} else if !findings.is_empty() {
		DoctorStatus::Warning
	} else {
		DoctorStatus::Ok
	};

	DoctorReport {
		schema_version: DOCTOR_SCHEMA_VERSION,
		cli_version: env!("CARGO_PKG_VERSION"),
		status,
		project,
		tools,
		checks,
		findings,
	}
}

#[derive(Clone, Copy)]
struct ToolSpec {
	name: &'static str,
	required: bool,
	version_args: &'static [&'static str],
	accept_nonzero: bool,
}

const BASE_TOOL_SPECS: &[ToolSpec] = &[
	ToolSpec {
		name: "cargo",
		required: true,
		version_args: &["--version"],
		accept_nonzero: false,
	},
	ToolSpec {
		name: "rustc",
		required: true,
		version_args: &["--version"],
		accept_nonzero: false,
	},
	ToolSpec {
		name: "cargo-build-sbf",
		required: true,
		version_args: &["--version"],
		accept_nonzero: true,
	},
	ToolSpec {
		name: "surfpool",
		required: false,
		version_args: &["--version"],
		accept_nonzero: false,
	},
	ToolSpec {
		name: "solana",
		required: false,
		version_args: &["--version"],
		accept_nonzero: false,
	},
];

const CLIENT_TOOL_SPECS: &[ToolSpec] = &[
	ToolSpec {
		name: "node",
		required: true,
		version_args: &["--version"],
		accept_nonzero: false,
	},
	ToolSpec {
		name: "npx",
		required: false,
		version_args: &["--version"],
		accept_nonzero: false,
	},
	ToolSpec {
		name: "pnpm",
		required: false,
		version_args: &["--version"],
		accept_nonzero: false,
	},
];

fn tool_specs(needs_node: bool) -> Vec<ToolSpec> {
	let mut specs = BASE_TOOL_SPECS.to_vec();
	if needs_node {
		specs.extend_from_slice(CLIENT_TOOL_SPECS);
	}
	specs
}

fn rust_src_available() -> bool {
	rust_src_available_from(Command::new("rustc").args(["--print", "sysroot"]))
}

fn rust_src_available_from(command: &mut Command) -> bool {
	let Ok(output) = capture_command(command) else {
		return false;
	};
	if !output.status.success() {
		return false;
	}

	let Ok(sysroot) = std::str::from_utf8(&output.stdout) else {
		return false;
	};
	let sysroot = sysroot.trim();
	if sysroot.is_empty() || sysroot.chars().any(char::is_control) {
		return false;
	}

	Path::new(sysroot)
		.join("lib/rustlib/src/rust/library/core/src/lib.rs")
		.is_file()
}

fn diagnose_project(
	start: &Path,
	findings: &mut Vec<String>,
	checks: &mut Vec<DoctorCheck>,
) -> Option<ProjectDiagnostic> {
	let project = match Project::discover(start) {
		Ok(project) => project,
		Err(error) => {
			findings.push(error.to_string());
			checks.push(DoctorCheck {
				id: "project.discovery".to_owned(),
				status: CheckStatus::Fail,
				message: error.to_string(),
			});

			return None;
		}
	};
	checks.push(DoctorCheck {
		id: "project.discovery".to_owned(),
		status: CheckStatus::Pass,
		message: format!("found {}", project.root.display()),
	});
	let artifact = project.sbf_artifact();
	let keypair = project.keypair();
	let artifact_metadata = inspect_metadata(&artifact, fs::symlink_metadata(&artifact));
	let keypair_metadata = inspect_metadata(&keypair, fs::symlink_metadata(&keypair));
	let artifact_exists = artifact_metadata
		.as_ref()
		.is_ok_and(|value| value.as_ref().is_some_and(fs::Metadata::is_file));
	let keypair_exists = keypair_metadata.as_ref().is_ok_and(Option::is_some);
	append_inspection_error(&artifact_metadata, findings);
	append_inspection_error(&keypair_metadata, findings);
	let program_id = match inspect_program_id(&project.library_source) {
		Ok(declaration) => {
			checks.push(DoctorCheck {
				id: "project.program-id".to_owned(),
				status: CheckStatus::Pass,
				message: format!("declared as {}", declaration.program_id),
			});
			Some(declaration.program_id)
		}
		Err(error) => {
			findings.push(error.to_string());
			checks.push(DoctorCheck {
				id: "project.program-id".to_owned(),
				status: CheckStatus::Fail,
				message: error.to_string(),
			});
			None
		}
	};
	let (keypair_valid, keypair_matches_source) = if keypair_exists {
		match read_keypair_program_id(&keypair) {
			Ok(keypair_program_id) => {
				(
					true,
					program_id
						.as_ref()
						.map(|program_id| program_id == &keypair_program_id),
				)
			}
			Err(error) => {
				findings.push(error.to_string());

				(false, None)
			}
		}
	} else {
		(false, None)
	};
	checks.push(artifact_check(
		&artifact,
		&artifact_metadata,
		artifact_exists,
	));
	checks.push(keypair_check(
		&keypair,
		&keypair_metadata,
		keypair_exists,
		keypair_valid,
	));

	if let Some(matches) = keypair_matches_source {
		checks.push(DoctorCheck {
			id: "project.identity-match".to_owned(),
			status: if matches {
				CheckStatus::Pass
			} else {
				CheckStatus::Warn
			},
			message: if matches {
				"source and keypair program IDs match".to_owned()
			} else {
				"source and keypair program IDs differ".to_owned()
			},
		});
	}

	if artifact_metadata.is_ok() && !artifact_exists {
		findings.push(format!(
			"build the SBF artifact expected at {}",
			artifact.display()
		));
	}

	if keypair_metadata.is_ok() && !keypair_exists {
		findings.push(format!(
			"create a local program keypair at {}",
			keypair.display()
		));
	}

	if keypair_matches_source == Some(false) {
		findings.push("run `pina keys sync` after reviewing the selected keypair".to_owned());
	}

	Some(ProjectDiagnostic {
		root: project.root,
		package_name: project.package_name,
		source: project.library_source,
		program_id,
		artifact,
		artifact_exists,
		keypair,
		keypair_exists,
		keypair_matches_source,
		clients: project.clients,
	})
}

#[allow(clippy::unnecessary_debug_formatting)]
fn inspect_metadata(
	path: &Path,
	result: std::io::Result<fs::Metadata>,
) -> Result<Option<fs::Metadata>, String> {
	match result {
		Ok(metadata) => Ok(Some(metadata)),
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
		Err(error) => Err(format!("failed to inspect {path:?}: {error}")),
	}
}

fn tool_message(tool: &ToolDiagnostic) -> String {
	match (tool.available, tool.version.as_ref()) {
		(true, Some(version)) => format!("available: {version}"),
		(true, None) => "available (version not reported)".to_owned(),
		(false, _) => "not available on PATH".to_owned(),
	}
}

fn append_inspection_error(
	metadata: &Result<Option<fs::Metadata>, String>,
	findings: &mut Vec<String>,
) {
	if let Err(error) = metadata {
		findings.push(error.clone());
	}
}

fn artifact_check(
	path: &Path,
	metadata: &Result<Option<fs::Metadata>, String>,
	exists: bool,
) -> DoctorCheck {
	DoctorCheck {
		id: "project.artifact".to_owned(),
		status: if metadata.is_err() {
			CheckStatus::Fail
		} else if exists {
			CheckStatus::Pass
		} else {
			CheckStatus::Warn
		},
		message: metadata.as_ref().map_or_else(Clone::clone, |_| {
			format!("{} at {}", presence(exists), path.display())
		}),
	}
}

fn keypair_check(
	path: &Path,
	metadata: &Result<Option<fs::Metadata>, String>,
	exists: bool,
	valid: bool,
) -> DoctorCheck {
	DoctorCheck {
		id: "project.keypair".to_owned(),
		status: if metadata.is_err() {
			CheckStatus::Fail
		} else if valid {
			CheckStatus::Pass
		} else if exists {
			CheckStatus::Fail
		} else {
			CheckStatus::Warn
		},
		message: if let Err(error) = metadata {
			error.clone()
		} else if valid {
			format!("valid at {}", path.display())
		} else if exists {
			format!("invalid at {}", path.display())
		} else {
			format!("missing at {}", path.display())
		},
	}
}

fn diagnose_tool(spec: ToolSpec) -> ToolDiagnostic {
	let output = capture_command(Command::new(spec.name).args(spec.version_args));
	let Ok(output) = output else {
		return ToolDiagnostic {
			name: spec.name,
			required: spec.required,
			available: false,
			version: None,
		};
	};

	if !output.status.success() && !spec.accept_nonzero {
		return ToolDiagnostic {
			name: spec.name,
			required: spec.required,
			available: false,
			version: None,
		};
	}

	let bytes = if output.stdout.is_empty() {
		&output.stderr
	} else {
		&output.stdout
	};
	let version = sanitized_first_line(bytes);

	ToolDiagnostic {
		name: spec.name,
		required: spec.required,
		available: true,
		version,
	}
}

const TOOL_OUTPUT_LIMIT: usize = 4 * 1024;
const VERSION_TEXT_LIMIT: usize = 512;
const TOOL_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
struct CapturedOutput {
	status: ExitStatus,
	stdout: Vec<u8>,
	stderr: Vec<u8>,
}

fn capture_command(command: &mut Command) -> std::io::Result<CapturedOutput> {
	capture_command_with_timeout(command, TOOL_TIMEOUT)
}

fn capture_command_with_timeout(
	command: &mut Command,
	timeout: Duration,
) -> std::io::Result<CapturedOutput> {
	let deadline = Instant::now() + timeout;
	let mut child = command
		.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()?;
	let stdout = child
		.stdout
		.take()
		.ok_or_else(|| std::io::Error::other("diagnostic command did not provide stdout"))?;
	let stderr = child
		.stderr
		.take()
		.ok_or_else(|| std::io::Error::other("diagnostic command did not provide stderr"))?;
	let stdout_reader = spawn_reader(stdout);
	let stderr_reader = spawn_reader(stderr);
	let status = wait_with_timeout(
		&mut child,
		deadline.saturating_duration_since(Instant::now()),
	)?;
	let stdout = receive_reader(&stdout_reader, deadline)?;
	let stderr = receive_reader(&stderr_reader, deadline)?;

	Ok(CapturedOutput {
		status,
		stdout,
		stderr,
	})
}

fn command_status(command: &mut Command) -> std::io::Result<ExitStatus> {
	let mut child = command
		.stdin(Stdio::null())
		.stdout(Stdio::null())
		.stderr(Stdio::null())
		.spawn()?;
	wait_with_timeout(&mut child, TOOL_TIMEOUT)
}

trait ManagedChild {
	fn try_wait_managed(&mut self) -> std::io::Result<Option<ExitStatus>>;
	fn kill_managed(&mut self) -> std::io::Result<()>;
	fn wait_managed(&mut self) -> std::io::Result<ExitStatus>;
}

impl ManagedChild for std::process::Child {
	fn try_wait_managed(&mut self) -> std::io::Result<Option<ExitStatus>> {
		self.try_wait()
	}

	fn kill_managed(&mut self) -> std::io::Result<()> {
		self.kill()
	}

	fn wait_managed(&mut self) -> std::io::Result<ExitStatus> {
		self.wait()
	}
}

fn wait_with_timeout(
	child: &mut impl ManagedChild,
	timeout: Duration,
) -> std::io::Result<ExitStatus> {
	let started = Instant::now();

	loop {
		match child.try_wait_managed() {
			Ok(Some(status)) => return Ok(status),
			Ok(None) => {}
			Err(error) => return cleanup_after_error(child, error),
		}

		if started.elapsed() >= timeout {
			let timeout_error = std::io::Error::new(
				std::io::ErrorKind::TimedOut,
				"diagnostic command exceeded the five-second timeout",
			);
			return cleanup_after_error(child, timeout_error);
		}

		std::thread::sleep(Duration::from_millis(10));
	}
}

fn cleanup_after_error<T>(
	child: &mut impl ManagedChild,
	original: std::io::Error,
) -> std::io::Result<T> {
	if let Err(kill) = child.kill_managed() {
		return Err(std::io::Error::other(format!(
			"{original}; child cleanup failed (kill: {kill})"
		)));
	}

	match child.wait_managed() {
		Ok(_) => Err(original),
		Err(wait) => {
			Err(std::io::Error::other(format!(
				"{original}; child cleanup failed (wait: {wait})"
			)))
		}
	}
}

fn drain_bounded(mut reader: impl Read) -> std::io::Result<Vec<u8>> {
	let mut captured = Vec::with_capacity(TOOL_OUTPUT_LIMIT);
	let mut chunk = [0u8; 1024];

	loop {
		let read = reader.read(&mut chunk)?;
		if read == 0 {
			return Ok(captured);
		}

		let remaining = TOOL_OUTPUT_LIMIT.saturating_sub(captured.len());
		captured.extend_from_slice(&chunk[..read.min(remaining)]);
	}
}

fn spawn_reader(
	reader: impl Read + Send + 'static,
) -> std::sync::mpsc::Receiver<std::io::Result<Vec<u8>>> {
	let (sender, receiver) = std::sync::mpsc::sync_channel(1);
	std::thread::spawn(move || {
		let _ = sender.send(drain_bounded(reader));
	});
	receiver
}

fn receive_reader(
	receiver: &std::sync::mpsc::Receiver<std::io::Result<Vec<u8>>>,
	deadline: Instant,
) -> std::io::Result<Vec<u8>> {
	let remaining = deadline.saturating_duration_since(Instant::now());

	match receiver.recv_timeout(remaining) {
		Ok(result) => result,
		Err(error) => {
			let kind = match error {
				std::sync::mpsc::RecvTimeoutError::Timeout => std::io::ErrorKind::TimedOut,
				std::sync::mpsc::RecvTimeoutError::Disconnected => std::io::ErrorKind::Other,
			};
			Err(std::io::Error::new(
				kind,
				"diagnostic output remained open after the probe exited",
			))
		}
	}
}

fn sanitized_first_line(bytes: &[u8]) -> Option<String> {
	let text = String::from_utf8_lossy(bytes);
	let line = text
		.lines()
		.next()
		.map(str::trim)
		.filter(|line| !line.is_empty())?;
	Some(escape_controls(
		&line.chars().take(VERSION_TEXT_LIMIT).collect::<String>(),
	))
}

fn escape_controls(value: &str) -> String {
	let mut escaped = String::with_capacity(value.len());

	for character in value.chars() {
		if character.is_control() {
			escaped.extend(character.escape_default());
		} else {
			escaped.push(character);
		}
	}

	escaped
}

fn presence(exists: bool) -> &'static str {
	if exists { "found" } else { "missing" }
}

#[cfg(test)]
mod tests {
	use std::fs;

	use ed25519_dalek::SigningKey;
	use solana_address::Address;
	use tempfile::TempDir;

	use super::*;

	#[test]
	fn renders_stable_human_report() {
		let report = DoctorReport {
			schema_version: 1,
			cli_version: "1.2.3",
			status: DoctorStatus::Warning,
			project: Some(ProjectDiagnostic {
				root: PathBuf::from("/project"),
				package_name: "counter".to_owned(),
				source: PathBuf::from("/project/src/lib.rs"),
				program_id: Some("11111111111111111111111111111111".to_owned()),
				artifact: PathBuf::from("/project/target/deploy/counter.so"),
				artifact_exists: false,
				keypair: PathBuf::from("/project/target/deploy/counter-keypair.json"),
				keypair_exists: true,
				keypair_matches_source: Some(true),
				clients: vec![ClientLanguage::Rust],
			}),
			tools: vec![
				ToolDiagnostic {
					name: "cargo",
					required: true,
					available: true,
					version: Some("cargo 1.89.0".to_owned()),
				},
				ToolDiagnostic {
					name: "surfpool",
					required: false,
					available: false,
					version: None,
				},
			],
			checks: vec![DoctorCheck {
				id: "project.artifact".to_owned(),
				status: CheckStatus::Warn,
				message: "missing".to_owned(),
			}],
			findings: vec!["build the program".to_owned()],
		};

		assert_eq!(
			report.render_text(),
			"Pina doctor\nCLI: 1.2.3\nProject: /project\nPackage: counter\nProgram ID: \
			 11111111111111111111111111111111\nArtifact: /project/target/deploy/counter.so \
			 (missing)\nKeypair: /project/target/deploy/counter-keypair.json (found)\nTools:\n  \
			 cargo (required): cargo 1.89.0\n  surfpool (optional): missing\nChecks:\n  [warn] \
			 project.artifact: missing\nFindings:\n  - build the program\nStatus: warning\n"
		);
	}

	#[test]
	fn human_report_escapes_controls_without_changing_json_values() {
		let report = DoctorReport {
			schema_version: 1,
			cli_version: "1.2.3",
			status: DoctorStatus::Warning,
			project: Some(ProjectDiagnostic {
				root: PathBuf::from("/project\n\u{1b}"),
				package_name: "counter\rname".to_owned(),
				source: PathBuf::from("/source"),
				program_id: Some("program\tidentifier".to_owned()),
				artifact: PathBuf::from("/artifact\npath"),
				artifact_exists: false,
				keypair: PathBuf::from("/keypair\rpath"),
				keypair_exists: true,
				keypair_matches_source: None,
				clients: Vec::new(),
			}),
			tools: vec![ToolDiagnostic {
				name: "tool\nname",
				required: true,
				available: true,
				version: Some("version\u{1b}".to_owned()),
			}],
			checks: vec![DoctorCheck {
				id: "check\nid".to_owned(),
				status: CheckStatus::Warn,
				message: "message\rtext".to_owned(),
			}],
			findings: vec!["finding\ttext".to_owned()],
		};

		let text = report.render_text();
		let json = serde_json::to_value(&report)
			.unwrap_or_else(|error| panic!("serialization failed: {error}"));

		assert!(
			text.chars()
				.all(|character| character == '\n' || !character.is_control())
		);
		assert!(text.contains("/project\\n\\u{1b}"));
		assert!(text.contains("counter\\rname"));
		assert!(text.contains("check\\nid: message\\rtext"));
		assert!(text.contains("finding\\ttext"));
		assert_eq!(json["project"]["packageName"], "counter\rname");
		assert_eq!(json["checks"][0]["message"], "message\rtext");
	}

	#[test]
	fn renders_error_and_ok_reports_without_a_project_or_findings() {
		let error = DoctorReport {
			schema_version: 1,
			cli_version: "1.2.3",
			status: DoctorStatus::Error,
			project: None,
			tools: Vec::new(),
			checks: vec![DoctorCheck {
				id: "project.discovery".to_owned(),
				status: CheckStatus::Fail,
				message: "missing".to_owned(),
			}],
			findings: Vec::new(),
		};
		let ok = DoctorReport {
			status: DoctorStatus::Ok,
			..error.clone()
		};

		assert!(error.render_text().contains("Project: unavailable"));
		assert!(error.render_text().contains("Status: error"));
		assert!(ok.render_text().contains("Status: ok"));
	}

	#[test]
	fn diagnoses_matching_and_mismatching_project_identity() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
		let root = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonicalize failed: {error}"));
		let source_dir = root.join("src");
		let deploy_dir = root.join("target/deploy");
		fs::create_dir_all(&source_dir)
			.unwrap_or_else(|error| panic!("source create failed: {error}"));
		fs::create_dir_all(&deploy_dir)
			.unwrap_or_else(|error| panic!("deploy create failed: {error}"));
		fs::write(
			root.join("Cargo.toml"),
			"[package]\nname = \"doctor-demo\"\n",
		)
		.unwrap_or_else(|error| panic!("manifest write failed: {error}"));
		fs::write(deploy_dir.join("doctor_demo.so"), [])
			.unwrap_or_else(|error| panic!("artifact write failed: {error}"));
		let signing_key = SigningKey::from_bytes(&[12u8; 32]);
		let public_key = signing_key.verifying_key().to_bytes();
		let program_id = Address::from(public_key).to_string();
		let mut bytes = signing_key.to_bytes().to_vec();
		bytes.extend(public_key);
		fs::write(
			deploy_dir.join("doctor_demo-keypair.json"),
			serde_json::to_vec(&bytes)
				.unwrap_or_else(|error| panic!("keypair serialization failed: {error}")),
		)
		.unwrap_or_else(|error| panic!("keypair write failed: {error}"));
		fs::write(
			source_dir.join("lib.rs"),
			format!("declare_id!(\"{program_id}\");\n"),
		)
		.unwrap_or_else(|error| panic!("source write failed: {error}"));

		let matching = diagnose(&root);
		assert_eq!(
			matching
				.project
				.as_ref()
				.and_then(|project| project.keypair_matches_source),
			Some(true)
		);

		fs::write(
			source_dir.join("lib.rs"),
			"declare_id!(\"11111111111111111111111111111111\");\n",
		)
		.unwrap_or_else(|error| panic!("source rewrite failed: {error}"));
		let mismatch = diagnose(&root);
		assert_eq!(
			mismatch
				.project
				.as_ref()
				.and_then(|project| project.keypair_matches_source),
			Some(false)
		);
		assert!(
			mismatch
				.findings
				.contains(&"run `pina keys sync` after reviewing the selected keypair".to_owned())
		);
	}

	#[test]
	fn tool_diagnostics_cover_launch_failure_nonzero_and_stderr_versions() {
		let missing = diagnose_tool(ToolSpec {
			name: "pina-definitely-missing-tool",
			required: true,
			version_args: &[],
			accept_nonzero: false,
		});
		let rejected = diagnose_tool(ToolSpec {
			name: "sh",
			required: false,
			version_args: &["--pina-invalid-option"],
			accept_nonzero: false,
		});
		let accepted = diagnose_tool(ToolSpec {
			name: "sh",
			required: false,
			version_args: &["--pina-invalid-option"],
			accept_nonzero: true,
		});

		assert!(!missing.available);
		assert!(!rejected.available);
		assert!(accepted.available);
		assert!(accepted.version.is_some());
	}

	#[cfg(unix)]
	#[test]
	fn command_capture_drains_both_streams_with_bounded_storage() {
		let output = capture_command(Command::new("sh").args([
			"-c",
			"i=0; while [ $i -lt 6000 ]; do printf x; printf y >&2; i=$((i+1)); done",
		]))
		.unwrap_or_else(|error| panic!("capture failed: {error}"));

		assert!(output.status.success());
		assert_eq!(output.stdout.len(), TOOL_OUTPUT_LIMIT);
		assert_eq!(output.stderr.len(), TOOL_OUTPUT_LIMIT);
	}

	#[test]
	fn command_capture_closes_child_stdin() {
		const NESTED: &str = "PINA_DOCTOR_STDIN_NESTED";

		let executable = std::env::current_exe()
			.unwrap_or_else(|error| panic!("test executable discovery failed: {error}"));

		if std::env::var_os(NESTED).is_some() {
			let output = capture_command(
				Command::new(executable)
					.args(["--exact", "doctor::tests::stdin_probe_process"])
					.env("PINA_DOCTOR_STDIN_PROBE", "1"),
			)
			.unwrap_or_else(|error| panic!("capture failed: {error}"));
			assert!(output.status.success());

			return;
		}

		let mut nested = Command::new(executable)
			.args([
				"--exact",
				"doctor::tests::command_capture_closes_child_stdin",
			])
			.env(NESTED, "1")
			.stdin(Stdio::piped())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.spawn()
			.unwrap_or_else(|error| panic!("nested test launch failed: {error}"));
		let _held_open = nested
			.stdin
			.take()
			.unwrap_or_else(|| panic!("nested stdin pipe should be present"));

		let status = wait_for_test_child(&mut nested, 200)
			.unwrap_or_else(|| panic!("diagnostic child inherited operator input"));
		assert!(status.success());
	}

	#[test]
	fn command_capture_and_status_kill_hanging_probes() {
		let mut capture = hanging_command();
		let capture_error = capture_command_with_timeout(&mut capture, Duration::from_millis(50))
			.expect_err("capture must time out");
		assert_eq!(capture_error.kind(), std::io::ErrorKind::TimedOut);

		let mut status = hanging_command();
		let mut child = status
			.stdin(Stdio::null())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.spawn()
			.unwrap_or_else(|error| panic!("status probe launch failed: {error}"));
		let status_error = wait_with_timeout(&mut child, Duration::from_millis(50))
			.expect_err("status must time out");
		assert_eq!(status_error.kind(), std::io::ErrorKind::TimedOut);

		let mut preserved = FakeManagedChild::new(true, false, false);
		let error = wait_with_timeout(&mut preserved, Duration::ZERO)
			.expect_err("try-wait error must be preserved");
		assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
		let mut timed_out = FakeManagedChild::new(false, false, false);
		let error = wait_with_timeout(&mut timed_out, Duration::ZERO)
			.expect_err("running child must time out");
		assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
		assert!(timed_out.wait_called);

		for (kill_error, wait_error) in [(true, false), (false, true)] {
			let mut cleanup_failure = FakeManagedChild::new(true, kill_error, wait_error);
			let error = wait_with_timeout(&mut cleanup_failure, Duration::ZERO)
				.expect_err("cleanup failure must be reported");
			assert!(error.to_string().contains("child cleanup failed"));
			assert_eq!(cleanup_failure.wait_called, !kill_error);
		}
	}

	#[test]
	fn disconnected_output_reader_is_reported() {
		let (sender, receiver) = std::sync::mpsc::channel();
		drop(sender);

		let error = receive_reader(&receiver, Instant::now() + Duration::from_millis(10))
			.expect_err("disconnected reader must fail");
		assert_eq!(error.kind(), std::io::ErrorKind::Other);
	}

	#[test]
	fn command_capture_bounds_pipes_held_by_descendants() {
		const PROBE: &str = "PINA_DOCTOR_OUTPUT_PROBE";
		const HOLDER: &str = "PINA_DOCTOR_OUTPUT_HOLDER";
		let executable = std::env::current_exe()
			.unwrap_or_else(|error| panic!("test executable discovery failed: {error}"));

		if std::env::var_os(HOLDER).is_some() {
			std::thread::sleep(Duration::from_millis(500));
			return;
		}

		if std::env::var_os(PROBE).is_some() {
			let mut holder = Command::new(executable)
				.args([
					"--exact",
					"doctor::tests::command_capture_bounds_pipes_held_by_descendants",
				])
				.env(HOLDER, "1")
				.stdin(Stdio::null())
				.spawn()
				.unwrap_or_else(|error| panic!("output holder launch failed: {error}"));
			std::thread::spawn(move || holder.wait());
			return;
		}

		let mut command = Command::new(executable);
		command
			.args([
				"--exact",
				"doctor::tests::command_capture_bounds_pipes_held_by_descendants",
			])
			.env(PROBE, "1");
		let started = Instant::now();
		let error = capture_command_with_timeout(&mut command, Duration::from_millis(100))
			.expect_err("inherited output pipe must hit the capture deadline");

		assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
		assert!(started.elapsed() < Duration::from_millis(400));
	}

	struct FakeManagedChild {
		failures: [bool; 3],
		wait_called: bool,
	}

	impl FakeManagedChild {
		fn new(try_error: bool, kill_error: bool, wait_error: bool) -> Self {
			Self {
				failures: [try_error, kill_error, wait_error],
				wait_called: false,
			}
		}
	}

	impl ManagedChild for FakeManagedChild {
		fn try_wait_managed(&mut self) -> std::io::Result<Option<ExitStatus>> {
			if self.failures[0] {
				return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
			}
			Ok(None)
		}

		fn kill_managed(&mut self) -> std::io::Result<()> {
			if self.failures[1] {
				return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
			}
			Ok(())
		}

		fn wait_managed(&mut self) -> std::io::Result<ExitStatus> {
			self.wait_called = true;
			if self.failures[2] {
				return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
			}
			Ok(success_status())
		}
	}

	#[cfg(unix)]
	fn success_status() -> ExitStatus {
		use std::os::unix::process::ExitStatusExt;
		ExitStatus::from_raw(0)
	}

	#[cfg(windows)]
	fn success_status() -> ExitStatus {
		use std::os::windows::process::ExitStatusExt;
		ExitStatus::from_raw(0)
	}

	#[cfg(unix)]
	fn hanging_command() -> Command {
		let mut command = Command::new("sleep");
		command.arg("60");
		command
	}

	#[cfg(windows)]
	fn hanging_command() -> Command {
		let mut command = Command::new("ping");
		command.args(["-n", "60", "127.0.0.1"]);
		command
	}

	#[test]
	fn stdin_probe_process() {
		if std::env::var_os("PINA_DOCTOR_STDIN_PROBE").is_none() {
			return;
		}

		let mut input = Vec::new();
		std::io::stdin()
			.read_to_end(&mut input)
			.unwrap_or_else(|error| panic!("stdin probe failed: {error}"));
		assert!(input.is_empty());
	}

	#[test]
	fn nested_test_wait_terminates_a_blocked_probe() {
		let executable = std::env::current_exe()
			.unwrap_or_else(|error| panic!("test executable discovery failed: {error}"));
		let mut child = Command::new(executable)
			.args(["--exact", "doctor::tests::stdin_probe_process"])
			.env("PINA_DOCTOR_STDIN_PROBE", "1")
			.stdin(Stdio::piped())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.spawn()
			.unwrap_or_else(|error| panic!("probe launch failed: {error}"));
		let _held_open = child
			.stdin
			.take()
			.unwrap_or_else(|| panic!("probe stdin pipe should be present"));

		assert!(wait_for_test_child(&mut child, 1).is_none());
	}

	fn wait_for_test_child(child: &mut std::process::Child, attempts: usize) -> Option<ExitStatus> {
		for _ in 0..attempts {
			if let Some(status) = child
				.try_wait()
				.unwrap_or_else(|error| panic!("nested test wait failed: {error}"))
			{
				return Some(status);
			}

			std::thread::sleep(Duration::from_millis(10));
		}

		let _ = child.kill();
		let _ = child.wait();

		None
	}

	#[test]
	fn version_text_escapes_controls_and_replaces_invalid_utf8() {
		let version = sanitized_first_line(b"\x1b[31mtool\xff\t1.0\nignored")
			.unwrap_or_else(|| panic!("version should be present"));

		assert!(!version.contains('\x1b'));
		assert!(!version.contains('\t'));
		assert!(version.contains("\\u{1b}"));
		assert!(version.contains(char::REPLACEMENT_CHARACTER));
		assert!(version.contains("\\t"));
	}

	#[test]
	fn version_text_is_bounded_and_empty_input_stays_absent() {
		let oversized = vec![b'x'; VERSION_TEXT_LIMIT * 4];
		let version =
			sanitized_first_line(&oversized).unwrap_or_else(|| panic!("version should be present"));

		assert_eq!(version.len(), VERSION_TEXT_LIMIT);
		assert_eq!(sanitized_first_line(b"\n"), None);
	}

	#[test]
	fn metadata_inspection_and_versionless_tools_remain_unambiguous() {
		let path = Path::new("keypair\n\u{1b}[31m.json");
		assert!(
			inspect_metadata(
				path,
				Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
			)
			.unwrap_or_else(|error| panic!("missing metadata rejected: {error}"))
			.is_none()
		);
		let denied = inspect_metadata(
			path,
			Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
		)
		.expect_err("inspection error must be preserved");
		assert!(!denied.contains('\u{1b}'));
		assert!(!denied.contains('\n'));
		assert!(denied.contains("\\n"));

		assert_eq!(
			tool_message(&ToolDiagnostic {
				name: "cargo-build-sbf",
				required: true,
				available: true,
				version: None,
			}),
			"available (version not reported)"
		);

		let mut findings = Vec::new();
		let inspection_error = Err("inspection failed".to_owned());
		append_inspection_error(&inspection_error, &mut findings);
		append_inspection_error(&Ok(None), &mut findings);
		assert_eq!(findings, ["inspection failed"]);
		assert_eq!(
			artifact_check(path, &inspection_error, false).status,
			CheckStatus::Fail
		);
		assert_eq!(
			artifact_check(path, &Ok(None), false).status,
			CheckStatus::Warn
		);
		assert_eq!(
			artifact_check(path, &Ok(None), true).status,
			CheckStatus::Pass
		);
		assert_eq!(
			keypair_check(path, &inspection_error, false, false).status,
			CheckStatus::Fail
		);
		assert_eq!(
			keypair_check(path, &Ok(None), true, false).status,
			CheckStatus::Fail
		);
		assert_eq!(
			keypair_check(path, &Ok(None), false, false).status,
			CheckStatus::Warn
		);
		assert_eq!(
			keypair_check(path, &Ok(None), true, true).status,
			CheckStatus::Pass
		);
	}

	#[cfg(unix)]
	#[test]
	fn rust_source_probe_rejects_launch_nonzero_non_utf8_and_control_failures() {
		assert!(!rust_src_available_from(&mut Command::new(
			"pina-definitely-missing-rustc"
		)));
		assert!(!rust_src_available_from(
			Command::new("sh").args(["-c", "exit 1"])
		));
		assert!(!rust_src_available_from(
			Command::new("sh").args(["-c", "printf '\\377'",])
		));
		assert!(!rust_src_available_from(
			Command::new("sh").args(["-c", "printf '/tmp/invalid\\tpath'",])
		));
	}
}
